//! A bounded byte local built from a literal owns fresh backing and lends it
//! to a static machine that forwards the qualified shared loan across states.
//!
//! `let text: [u8; 8] in Utf8 = "150";` is a bounded owner with live length
//! 3. Lowering copies the literal into owned backing through the ordinary
//! concatenation materialization, lends the owner whole to the callee's
//! `&[u8; 8] in Utf8` formal, and the callee forwards that loan to a
//! successor block with the same qualifications. Terminal verification
//! replays the local's predicates from its operands and the successor's from
//! its exact forwarded source, so either link rejects when corrupted.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::{ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralTypeShape};

const SOURCE: &str = r#"
domain [u8]::Utf8 requires valid_utf8(self);
domain [u8; 8]::Utf8 requires valid_utf8(self);

machine forwarded_length(text: &[u8; 8] in Utf8) -> u64 {
    transition { _ -> measure(text) }

    state measure(text: &[u8; 8] in Utf8) -> u64 {
        transition { _ -> (text.len) }
    }
}

machine main() -> u64 {
    let text: [u8; 8] in Utf8 = "150";
    let length: u64 = forwarded_length(&text);
    transition { _ -> (length) }
}
"#;

fn lower() -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("main"))
        .expect("a lent bounded owner local lowers to Terminal Psi")
}

fn verify(lowered: &lowered_psi::LoweredPsi) -> Result<(), String> {
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

fn bounded_owner(
    module: &terminal_psi::TerminalModule,
    id: semantic_vocabulary::StructuralTypeId,
) -> bool {
    module.structural_types.iter().any(|row| {
        row.id == id
            && row.shape
                == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 8,
                })
    })
}

#[test]
fn literal_local_owns_its_backing_and_its_loan_is_forwarded_qualified() {
    let lowered = lower();
    verify(&lowered).expect("the owned local and its forwarded loan verify");
    let module = &lowered.semantic_module;
    let owned = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(
            |operation| match (&operation.kind, operation.result.structural()) {
                (OperationKind::EstablishByteSequenceConcatenation { .. }, Some(result)) => {
                    Some(result.structural_type)
                }
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    assert!(
        matches!(owned.as_slice(), [owner] if bounded_owner(module, *owner)),
        "the local is one owned bounded concatenation of its declared carrier: {owned:?}"
    );
    let forwarded = module
        .machines
        .iter()
        .flat_map(|machine| machine.blocks.iter().skip(1))
        .flat_map(|block| &block.structural_parameters)
        .filter(|parameter| {
            parameter.access == StructuralAccess::SharedBorrow
                && !parameter.qualifications.is_empty()
                && bounded_owner(module, parameter.structural_type)
        })
        .count();
    assert_eq!(
        forwarded, 1,
        "the successor state receives the qualified owner loan"
    );
}

#[test]
fn a_successor_loan_cannot_drop_or_change_its_qualifications() {
    let mut lowered = lower();
    let module = &mut lowered.semantic_module;
    let parameter = module
        .machines
        .iter_mut()
        .flat_map(|machine| machine.blocks.iter_mut().skip(1))
        .flat_map(|block| block.structural_parameters.iter_mut())
        .find(|parameter| !parameter.qualifications.is_empty())
        .expect("qualified successor loan");
    parameter.qualifications.clear();
    let error = verify(&lowered).expect_err("a successor loan cannot forget its qualifications");
    assert!(
        error.contains("InvalidStructuralSuccessorArgument"),
        "the forwarding edge rejects: {error}"
    );
}

/// The local declares `in Utf8`, so its owned backing must establish that
/// predicate from its operands before any loan of it reaches the formal. A
/// same-length replacement keeps every length fact intact: valid UTF-8 still
/// verifies, and invalid UTF-8 rejects at the owner's establishment.
#[test]
fn an_owned_local_must_establish_every_predicate_its_formal_requires() {
    for (replacement, valid) in [(b"151".to_vec(), true), (vec![0x80, b'5', b'0'], false)] {
        let mut lowered = lower();
        let literal = lowered
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| machine.blocks.iter_mut())
            .flat_map(|block| block.operations.iter_mut())
            .find_map(|operation| match &mut operation.kind {
                OperationKind::EstablishByteSequenceLiteral { bytes, .. } if bytes == b"150" => {
                    Some(bytes)
                }
                _ => None,
            })
            .expect("the local's literal operand");
        *literal = replacement;
        if valid {
            verify(&lowered).expect("another valid UTF-8 literal establishes the owner");
            continue;
        }
        let error = verify(&lowered).expect_err("invalid UTF-8 cannot establish a Utf8 owner");
        assert!(
            error.contains("InvalidByteSequenceConcatenation"),
            "the owner establishment rejects: {error}"
        );
    }
}
