//! Evaluated content predicates cross a state edge only for their exact view.
use super::lower_machine;
use crate::TerminalMachineSelection;
use proof_admission::AdmissionProfile;

const SOURCE: &str = r#"
    machine validate(unrelated: &[u8], input: &[u8]) {
        transition valid_utf8(input) {
            true -> consume(0, unrelated, input)
            _ -> reject()
        }
        state consume(tag: u64, other: &[u8], selected: &[u8])
            requires valid_utf8(selected)
        {}
        state reject() {}
    }
    data Main {}
    machine Main::run() { validate("other", "valid"); }
"#;

#[test]
fn evaluated_byte_predicate_satisfies_exact_successor_requirement() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("the evaluated byte predicate reaches its exact successor formal");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verification reconstructs the current-content predicate");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequencePredicate { .. }
            ))
    );

    // An unchanged proof cannot turn another primitive predicate's Boolean
    // result into the UTF-8 requirement, even with the same view and length.
    let mut corrupted = lowered.semantic_module.clone();
    let machine = corrupted
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::ByteSequencePredicate { .. }
                    )
                })
        })
        .unwrap();
    let operation = machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequencePredicate { .. }
            )
        })
        .unwrap();
    let terminal_psi::OperationKind::ByteSequencePredicate { predicate, .. } = &mut operation.kind
    else {
        unreachable!()
    };
    *predicate = terminal_psi::ByteSequencePredicate::NoNul;
    assert!(
        terminal_verifier::verify_module(
            &corrupted,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "another predicate's evaluated result cannot certify UTF-8"
    );
}

fn verify_byte_contract(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("validate"))
        .expect("Boolean composition retains the exact content subject");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("polarity and projected content requirements independently verify");
    lowered
}

#[test]
fn byte_predicate_contract_preserves_compared_boolean() {
    verify_byte_contract(
        r#"machine validate(bytes: &[u8], expected: bool) {
        transition valid_utf8(bytes) == expected {
            true -> checked(13, expected, bytes)
            _ -> done()
        }
        state checked(tag: u64, expected: bool, bytes: &[u8])
            requires valid_utf8(bytes) == expected
        {}
        state done() {}
    }"#,
    );
}

#[test]
fn byte_predicate_contract_preserves_false_polarity() {
    verify_byte_contract(
        r#"machine validate(bytes: &[u8]) {
        transition valid_utf8(bytes) {
            true -> done()
            _ -> invalid(bytes)
        }
        state invalid(bytes: &[u8]) requires !valid_utf8(bytes) {}
        state done() {}
    }"#,
    );
}

#[test]
fn byte_predicate_contract_preserves_projected_subjects() {
    verify_byte_contract(
        r#"data Packet { tag: u64; bytes: &[u8]; }
    machine validate(unrelated: &Packet, input: &Packet) {
        transition valid_utf8(input.bytes) && no_nul(input.bytes) {
            true -> consume(7, unrelated, input)
            _ -> reject()
        }
        state consume(tag: u64, other: &Packet, selected: &Packet)
            requires valid_utf8(selected.bytes) && no_nul(selected.bytes)
        {}
        state reject() {}
    }"#,
    );
}

#[test]
fn byte_predicate_contract_substitutes_stored_view_actual() {
    let mut lowered = verify_byte_contract(
        r#"data Packet { bytes: &[u8]; other: &[u8]; }
    machine require_utf8(bytes: &[u8]) requires valid_utf8(bytes) {}
    machine validate(input: &Packet) {
        transition valid_utf8(input.bytes) {
            true -> consume(input)
            _ -> done()
        }
        state consume(input: &Packet) requires valid_utf8(input.bytes) {
            require_utf8(input.bytes);
        }
        state done() {}
    }"#,
    );
    let sibling = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| {
            let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
                return None;
            };
            (fields.len() == 2
                && fields
                    .iter()
                    .all(|field| field.field_type.byte_sequence_carrier().is_some()))
            .then(|| fields[1].identity.clone())
        })
        .expect("two byte-field endpoints");
    let argument = lowered
        .semantic_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| {
            let terminal_psi::OperationKind::CallUnit {
                structural_arguments,
                ..
            } = &mut operation.kind
            else {
                return None;
            };
            structural_arguments
                .iter_mut()
                .find(|argument| !argument.path.is_empty())
        })
        .expect("the helper receives the exact stored descriptor");
    *argument.path.last_mut().unwrap() = terminal_psi::StructuralPathSegment::Field(sibling);
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "the first field's verdict cannot prove the sibling actual's requirement"
    );
}
