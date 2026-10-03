use super::*;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::emit_direct_expression;
use crate::machine_lowering::lower_machine;
use crate::proofs::operation_proofs::finalize_operation_proofs;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::IntegerValue;
use terminal_psi::{OperationKind, Terminator};

#[test]
fn record_overlay_coordinate_requires_its_dimension_bound() {
    // Reuse a scalar module's ordinary return signature, then exercise only
    // the shared coordinate emitter and independent operation proof owner.
    let checked = crate::front_end::checked_program("machine coordinate() -> u64 { 0 }");
    let mut lowered = lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("coordinate"),
    )
    .unwrap();
    let coordinate = bounded_coordinate(coordinate_literal(1), 2).unwrap();
    let mut operations = OperationBuffer::new(100);
    let mut next_value = 100;
    let result = emit_direct_expression(&coordinate, &[], &mut next_value, &mut operations);
    let machine = &mut lowered.semantic_module.machines[0];
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| block.id == machine.entry)
        .unwrap();
    block.operations = operations.operations;
    let Terminator::Return { value, .. } = &mut block.terminator else {
        panic!("coordinate has an ordinary scalar return")
    };
    *value = result;
    lowered.proof_bundle.evidence.clear();
    finalize_operation_proofs(&mut lowered).unwrap();
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    assert!(matches!(
        terminal_interpreter::interpret_terminal_artifact(
            &semantic_bytes,
            &proof_bytes,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap(),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(1),
                ..
            }
        )
    ));

    let block = &mut lowered.semantic_module.machines[0].blocks[0];
    let (selector, dimension_bound) = block
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                right, obligation, ..
            } => Some((right, obligation)),
            _ => None,
        })
        .expect("the coordinate carries its dimension's formation obligation");
    let selected = block
        .operations
        .iter_mut()
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|value| value.id == selector)
        })
        .unwrap();
    assert!(matches!(
        selected.kind,
        OperationKind::IntegerConstant { .. }
    ));
    // Index 2 could still fit a larger byte backing after flattening, but it
    // cannot select an element of this declared two-element dimension.
    selected.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(2),
    };
    terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err()
    );
    lowered.proof_bundle.evidence.clear();
    assert!(matches!(
        finalize_operation_proofs(&mut lowered),
        Err(LoweringError::OperationProofUnavailable(obligation)) if obligation == dimension_bound
    ));
}

#[test]
fn record_overlay_coordinate_rejects_an_empty_dimension() {
    assert!(bounded_coordinate(coordinate_literal(0), 0).is_err());
}
