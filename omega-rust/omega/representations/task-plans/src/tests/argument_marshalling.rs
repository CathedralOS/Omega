//! Moved-argument marshalling: the start bundle carries a packed byte image
//! bound to the plan's exact argument layout, rejects malformed bundles, and
//! conserves the image byte-exact on rejection while retaining it as
//! provider-domain custody on acceptance.

use super::{
    argument_layout, candidate, id, invocation_receipt, marshal_arguments, moved_arguments,
    runtime, stack_lease, wcsu_plan,
};
use crate::{
    ActivationInstanceId, MovedTaskArguments, TaskArgumentCustodyId, TaskArgumentExtent,
    TaskArgumentLayout, TaskLifecycleLedger, TaskRuntimeInstanceId, TaskSettlementOutcome,
    TaskStartStorage, ValueLayoutId, validate_activation_plan,
};

fn layout(identity: u64, arguments: &[(u64, u64)]) -> TaskArgumentLayout {
    TaskArgumentLayout::new(
        id(identity, ValueLayoutId::from_normalized_identity),
        arguments,
    )
    .expect("canonical test layout")
}

#[test]
fn marshal_packs_arguments_at_canonical_offsets() {
    // (3 bytes at align 1) then (8 bytes at align 8) then (2 bytes at
    // align 2): offsets 0, 8, 16; image extent 24 at alignment 8.
    let layout = layout(700, &[(3, 1), (8, 8), (2, 2)]);
    assert_eq!(
        layout.fields,
        vec![
            TaskArgumentExtent {
                offset: 0,
                bytes: 3,
                alignment: 1
            },
            TaskArgumentExtent {
                offset: 8,
                bytes: 8,
                alignment: 8
            },
            TaskArgumentExtent {
                offset: 16,
                bytes: 2,
                alignment: 2
            },
        ]
    );
    assert_eq!(layout.bytes, 24);
    assert_eq!(layout.alignment, 8);

    let first = [0x11u8; 3];
    let second = [0x22u8; 8];
    let third = [0x33u8; 2];
    let bundle = MovedTaskArguments::marshal(
        &layout,
        &[&first, &second, &third],
        id(701, TaskArgumentCustodyId::from_normalized_identity),
    )
    .expect("three arguments marshal");

    let expected_image: Vec<u8> = first
        .into_iter()
        .chain([0u8; 5])
        .chain(second)
        .chain(third)
        .chain([0u8; 6])
        .collect();
    assert_eq!(bundle.image(), expected_image.as_slice());
    assert_eq!(bundle.argument_count(), 3);
    assert_eq!(bundle.argument(0), Some(&first[..]));
    assert_eq!(bundle.argument(1), Some(&second[..]));
    assert_eq!(bundle.argument(2), Some(&third[..]));
    assert_eq!(bundle.argument(3), None);
    assert_eq!(*bundle.layout(), layout);
}

#[test]
fn marshal_rejects_partial_and_ill_fitting_bundles() {
    let layout = argument_layout();
    let custody = id(710, TaskArgumentCustodyId::from_normalized_identity);
    let exact: Vec<Vec<u8>> = layout
        .fields
        .iter()
        .map(|field| vec![0x55u8; field.bytes as usize])
        .collect();
    let exact_views: Vec<&[u8]> = exact.iter().map(Vec::as_slice).collect();

    // A missing argument cannot cross the boundary.
    assert!(
        MovedTaskArguments::marshal(&layout, &exact_views[..1], custody)
            .expect_err("a missing argument rejects")
            .0
            .contains("supply 1 value(s) under a 2-field")
    );

    // An argument that under- or over-fills its field extent rejects:
    // neither truncation nor padding-by-omission crosses.
    let mut short = exact_views.clone();
    short[1] = &exact[1][..3];
    assert!(
        MovedTaskArguments::marshal(&layout, &short, custody)
            .expect_err("an under-filled argument rejects")
            .0
            .contains("supplies 3 byte(s) under a 4-byte")
    );
    let mut long = exact_views.clone();
    let oversized = vec![0x55u8; 5];
    long[1] = &oversized;
    assert!(
        MovedTaskArguments::marshal(&layout, &long, custody)
            .expect_err("an over-filled argument rejects")
            .0
            .contains("supplies 5 byte(s) under a 4-byte")
    );

    // A non-canonical layout — a field offset that is not the canonical
    // packed offset — cannot marshal at all.
    let mut drifted = layout.clone();
    drifted.fields[1].offset += 4;
    assert!(!drifted.is_canonical());
    assert!(
        MovedTaskArguments::marshal(&drifted, &exact_views, custody)
            .expect_err("a non-canonical layout rejects")
            .0
            .contains("non-canonical")
    );

    // Malformed field alignments reject at layout construction.
    assert!(
        TaskArgumentLayout::new(id(711, ValueLayoutId::from_normalized_identity), &[(4, 3)],)
            .expect_err("non-power-of-two alignment")
            .0
            .contains("power of two")
    );
    assert!(
        TaskArgumentLayout::new(id(712, ValueLayoutId::from_normalized_identity), &[(4, 0)],)
            .expect_err("zero alignment")
            .0
            .contains("power of two")
    );
    assert!(
        TaskArgumentLayout::new(
            id(713, ValueLayoutId::from_normalized_identity),
            &[(4, 1 << 20)],
        )
        .expect_err("alignment beyond the marshalling bound")
        .0
        .contains("exceeds the marshalling bound")
    );
}

#[test]
fn activation_validation_rejects_a_noncanonical_argument_layout() {
    let mut drifted = candidate();
    drifted.argument_layout.fields[0].offset = 8;
    assert!(
        validate_activation_plan(drifted)
            .expect_err("a non-canonical argument layout fails the plan seal")
            .0
            .contains("canonical packed marshalling layout")
    );

    let mut bad_alignment = candidate();
    bad_alignment.argument_layout.fields[0].alignment = 3;
    assert!(
        validate_activation_plan(bad_alignment)
            .expect_err("a malformed field alignment fails the plan seal")
            .0
            .contains("power of two")
    );
}

#[test]
fn accepted_start_retains_the_marshalled_image_under_the_claim() {
    let plan = wcsu_plan(72);
    let instance = id(720, TaskRuntimeInstanceId::from_normalized_identity);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let receipt = invocation_receipt(&plan, instance, 721, 722);

    let claim = ledger
        .accept_invocation(
            &receipt,
            id(723, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 724),
            TaskStartStorage::Persistent(stack_lease(&plan, 725, 726)),
        )
        .expect("accepted activation");

    // The activation holds the marshalled image as provider-domain custody:
    // the exact bytes a real runtime writes into the argument area. The
    // compact record reports the custody coordinate and byte extent.
    let claim_identity = claim.identity();
    {
        let record = ledger.records().next().expect("one live dependency");
        assert_eq!(
            record.argument_custody,
            id(724, TaskArgumentCustodyId::from_normalized_identity)
        );
        assert_eq!(record.argument_bytes, 16);
    }
    let retained = ledger
        .activation_arguments(claim_identity)
        .expect("a live claim retains its argument bundle");
    let expected_image: Vec<u8> = [0xA0u8; 8]
        .into_iter()
        .chain([0xA1u8; 4])
        .chain([0u8; 4])
        .collect();
    assert_eq!(retained.image(), expected_image.as_slice());
    assert_eq!(retained.argument(0), Some(&[0xA0u8; 8][..]));
    assert_eq!(retained.argument(1), Some(&[0xA1u8; 4][..]));

    // Settlement consumes the bundle with the activation: the moved
    // arguments do not return — only rejection conserves them.
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle");
    assert_eq!(ledger.activation_arguments(claim_identity), None);
    assert_eq!(ledger.records().count(), 0);
}
