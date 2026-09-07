//! The writer's strict-decrease obligation, exercised as an actual byte read.

use super::*;

mod rejections;

const SOURCE: &str = r#"
    boundary trait Output { machine write(value: u8) reaches Output; }
    machine relay(bytes: &[u8]) reaches Output {
        transition bytes.len > 0 {
            true -> tail(bytes[1..])
            false -> empty()
        }
        state tail(bytes: &[u8]) { Output::write(1u8); }
        state empty() {}
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80AB"); relay(""); relay("Z"); Output::write(7u8);
    }
"#;

/// Use the source-produced acyclic call graph as fixture scaffolding. The
/// added Terminal read tests a proof obligation, not source correspondence.
fn fixture() -> LoweredPsi {
    let mut lowered = lower_machine(&checked_source(SOURCE), "Root::enter").unwrap();
    let machine = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| {
            machine.blocks.iter().any(|block| {
                block.operations.iter().any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                })
            })
        })
        .unwrap();
    let slice = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
        .unwrap()
        .clone();
    let OperationKind::ByteSequenceSubslice { source, length, .. } = slice.kind else {
        panic!("subslice fixture");
    };
    let tail = slice.result.structural().unwrap().place;
    let next_operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .map(|operation| operation.id.get())
        .max()
        .unwrap()
        + 1;
    let next_value = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.scalar().map(|value| value.id.get()))
        .max()
        .unwrap()
        + 1;
    let extent = value_id(next_value);
    let byte = value_id(next_value + 1);
    let obligation = obligation_id(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .map(|evidence| evidence.obligation.get())
            .max()
            .unwrap()
            + 1,
    );
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| operation.id == slice.id)
        })
        .unwrap();
    block.operations.extend([
        Operation {
            id: operation_id(next_operation),
            result: OperationResult::Scalar(ValueDeclaration {
                id: extent,
                scalar_type: terminal_scalar_type(PrimitiveType::U64).unwrap(),
            }),
            kind: OperationKind::ByteSequenceLength { source: tail },
        },
        Operation {
            id: operation_id(next_operation + 1),
            result: OperationResult::Scalar(ValueDeclaration {
                id: byte,
                scalar_type: terminal_scalar_type(PrimitiveType::U8).unwrap(),
            }),
            // Its canonical bounds goal is tail.len < original.len: exactly
            // strict descent, not a fresh guard asserting the desired result.
            kind: OperationKind::ByteSequenceRead {
                source,
                index: extent,
                length,
                obligation,
            },
        },
    ]);
    for operation in machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::BoundaryCall { arguments, .. } = &mut operation.kind {
            *arguments = vec![byte];
        }
    }
    lowered.proof_bundle.evidence.clear();
    lowered
}

#[test]
fn guarded_subslice_extent_proves_strict_length_descent_and_executes() {
    let mut lowered = fixture();
    crate::operation_emission::finalize_operation_proofs(&mut lowered)
        .expect("a guarded tail's exact extent proves strict descent");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let execution = terminal_interpreter::interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[],
    )
    .expect("independent proof verification and byte execution");
    assert_eq!(
        execution.value(),
        terminal_interpreter::TerminalExecutionResult::Unit
    );
    let effects = execution
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
                panic!("one byte")
            };
            *value
        })
        .collect::<Vec<_>>();
    assert_eq!(effects, vec![66, 90, 7]);
}
