//! Canonical proof production for a measured source-produced literal.

use super::*;

/// The literal and call are source-produced. The added Terminal read exercises
/// proof closure, not source correspondence for literal indexing.
fn fixture(literal: &str, byte_index: u128) -> LoweredPsi {
    let source = format!(
        r#"
        boundary trait Output {{ machine write(value: u8) reaches Output; }}
        machine relay(bytes: &[u8]) reaches Output {{ Output::write(1u8); }}
        data Root {{}}
        machine Root::enter() reaches Output {{
            relay("{literal}"); Output::write(0u8);
        }}
        "#
    );
    let mut lowered = lower_machine(&checked_source(&source), "Root::enter").unwrap();
    let obligation = obligation_id(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .map(|evidence| evidence.obligation.get())
            .max()
            .unwrap_or(0)
            + 1,
    );
    let next_operation = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| operation.id.get())
        .max()
        .unwrap()
        + 1;
    let next_value = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .parameters
                .iter()
                .copied()
                .chain(machine.blocks.iter().flat_map(|block| {
                    block.parameters.iter().copied().chain(
                        block
                            .operations
                            .iter()
                            .filter_map(|operation| operation.result.scalar()),
                    )
                }))
        })
        .map(|value| value.id.get())
        .max()
        .unwrap_or(0)
        + 1;
    let machine = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishByteSequenceLiteral { .. }
                )
            })
        })
        .unwrap();
    let literal_place = block
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::EstablishByteSequenceLiteral { destination, .. } => Some(destination),
            _ => None,
        })
        .unwrap();
    let mut output = block
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
        .unwrap()
        .clone();
    let length = value_id(next_value);
    let index = value_id(next_value + 1);
    let byte = value_id(next_value + 2);
    let scalar_result = |id, primitive| {
        OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id,
            scalar_type: terminal_scalar_type(primitive).unwrap(),
        })
    };
    block.operations.extend([
        Operation {
            id: operation_id(next_operation),
            result: scalar_result(length, PrimitiveType::U64),
            kind: OperationKind::ByteSequenceLength {
                source: literal_place,
            },
        },
        Operation {
            id: operation_id(next_operation + 1),
            result: scalar_result(index, PrimitiveType::U64),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(byte_index),
            },
        },
        Operation {
            id: operation_id(next_operation + 2),
            result: scalar_result(byte, PrimitiveType::U8),
            kind: OperationKind::ByteSequenceRead {
                source: literal_place,
                index,
                length,
                obligation,
            },
        },
    ]);
    output.id = operation_id(next_operation + 3);
    let OperationKind::BoundaryCall { arguments, .. } = &mut output.kind else {
        panic!("scalar output boundary");
    };
    *arguments = vec![byte];
    block.operations.push(output);
    lowered.proof_bundle.evidence.clear();
    lowered
}

#[test]
fn measured_literal_last_byte_proves_bounds_and_executes_without_guard() {
    // The second literal has three bytes but only two Unicode scalar values.
    for (literal, expected_byte) in [("XXX", 88), ("éZ", 90)] {
        let mut lowered = fixture(literal, 2);
        crate::operation_emission::finalize_operation_proofs(&mut lowered)
            .expect("exact literal extent proves the last byte is in bounds");
        let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
        let execution = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("independent reload, proof verification, and byte execution");
        assert_eq!(
            execution.value(),
            terminal_interpreter::TerminalExecutionResult::Unit
        );
        let output = execution
            .effects()
            .iter()
            .map(|effect| {
                let terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. } = effect
                else {
                    panic!("byte output");
                };
                let [
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(value),
                        ..
                    },
                ] = arguments.as_slice()
                else {
                    panic!("one byte");
                };
                *value
            })
            .collect::<Vec<_>>();
        assert_eq!(output, vec![1, 0, expected_byte]);
    }
}

#[test]
fn measured_literal_extent_does_not_prove_one_past_end_or_empty_read() {
    for (literal, byte_index) in [("XXX", 3), ("éZ", 3), ("", 0)] {
        let mut lowered = fixture(literal, byte_index);
        terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
        assert!(
            matches!(
                crate::operation_emission::finalize_operation_proofs(&mut lowered),
                Err(LoweringError::OperationProofUnavailable(_))
            ),
            "literal {literal:?}, byte index {byte_index} must remain out of bounds"
        );
    }
}
