//! Fixed-array literals of scalar-leaf records have no element-assembling
//! Terminal operation. Their whole value is a roster of authored leaves, so
//! lowering establishes the array's zero and stores each leaf: record
//! scalars (bounded ones carrying their range obligation), nested record
//! members and closed primitive-array members, while omitted members keep the
//! zero. Shapes without a universal zero keep the establishment refusal.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::{OperationKind, StructuralPathSegment};

fn lower(source: &str) -> Result<lowered_psi::LoweredPsi, String> {
    let checked = crate::front_end::checked_program_result(source)
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("make"))
        .map_err(|error| format!("{error:?}"))
}

/// Lower `make`, verify the reloaded module independently, and list its leaf
/// stores in emission order as (path, carries a range obligation).
fn verified_stores(source: &str) -> Vec<(Vec<StructuralPathSegment>, bool)> {
    let lowered = lower(source).unwrap_or_else(|error| panic!("{error}"));
    let module = terminal_codec::decode_module(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
    )
    .unwrap();
    let proof = terminal_codec::decode_proof_bundle(
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
    )
    .unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("zeroed storage and its leaf stores verify independently");
    let operations = || {
        module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
    };
    assert_eq!(
        operations()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::EstablishZeroedStructural {}
            ))
            .count(),
        1,
        "the whole array establishes its zero once"
    );
    operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::StructuralScalarFieldStore {
                path,
                range_obligation,
                ..
            } => Some((path.clone(), range_obligation.is_some())),
            OperationKind::WriteOnlyPrimitiveStore { path, .. } => Some((path.clone(), false)),
            _ => None,
        })
        .collect()
}

fn field(name: &str) -> StructuralPathSegment {
    StructuralPathSegment::Field(name.to_owned())
}

#[test]
fn record_leaves_store_in_authored_order_with_their_range_obligations() {
    let stores = verified_stores(
        "data Cell { tag: u8 [0..=100]; low: u8; }
         data Pack { items: [Cell; 2]; }
         machine make() -> Pack { Pack { items: [Cell { tag: 7, low: 1 }, Cell { tag: 9 }] } }",
    );
    let index = |index| vec![StructuralPathSegment::FixedIndex(index)];
    // Only the bounded `tag` carries an obligation; the omitted `low` of the
    // second element keeps the established zero and stores nothing.
    assert_eq!(
        stores,
        [(index(0), true), (index(0), false), (index(1), true)]
    );
}

#[test]
fn nested_members_store_at_their_leaf_paths() {
    let stores = verified_stores(
        "data Span { low: u8; high: u8; }
         data Cell { span: Span; bytes: [u8; 2]; }
         data Pack { items: [Cell; 2]; }
         machine make() -> Pack {
             Pack { items: [Cell { span: Span { low: 1, high: 2 }, bytes: [3, 4] },
                            Cell { bytes: [8, 10] }] }
         }",
    );
    let index = StructuralPathSegment::FixedIndex;
    assert_eq!(
        stores,
        [
            (vec![index(0), field("span")], false),
            (vec![index(0), field("span")], false),
            (vec![index(0), field("bytes"), index(0)], false),
            (vec![index(0), field("bytes"), index(1)], false),
            (vec![index(1), field("bytes"), index(0)], false),
            (vec![index(1), field("bytes"), index(1)], false),
        ]
    );
}

#[test]
fn zeroless_shapes_keep_the_refusal() {
    assert_eq!(
        lower(
            "data Cell { tag: u8 [1..=8]; }
             data Pack { items: [Cell; 2]; }
             machine make() -> Pack { Pack { items: [Cell { tag: 7 }, Cell { tag: 8 }] } }"
        )
        .unwrap_err(),
        r#"Unsupported("fixed array literal has no Terminal establishment")"#
    );
}
