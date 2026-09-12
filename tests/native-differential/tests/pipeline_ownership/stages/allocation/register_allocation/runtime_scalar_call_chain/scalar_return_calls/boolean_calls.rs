//! Boolean entry values survive ordinary calls before selecting an arm.
use super::*;

#[test]
fn boolean_entry_survives_calls_and_controls_ordinary_graph_publication() {
    for inverted in [false, true] {
        let artifact = boolean_call_artifact(inverted);
        // The ABI defines only the low byte of a Boolean argument register.
        // Pass raw integers through the native trampoline, never invalid Rust
        // bool values, to verify that unrelated upper bits cannot select an arm.
        for (condition, native_argument) in [(false, 0), (true, 1), (false, 0x100), (true, 0x101)] {
            publish_scalar_artifacts_with_arguments(
                if condition != inverted { 37 } else { 41 },
                [artifact.clone()],
                &[terminal_interpreter::TerminalScalarValue::Boolean(
                    condition,
                )],
                [native_argument, 0, 0, 0],
            );
        }
    }
}

fn boolean_call_artifact(inverted: bool) -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = control_flow::branch_call_artifact(true);
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let branch = module.machines.remove(1);
    let callee = module.machines[1].id;
    let entry = &mut module.machines[0];
    let condition = ValueId::new(29_000).unwrap();
    entry.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: condition,
        scalar_type: ScalarType::Boolean,
    }];
    // Retain a real U64 call before the Boolean is consumed. Its result is
    // deliberately discarded, but the call remains part of execution order.
    let mut prefix = vec![
        entry.blocks[0].operations[0].clone(),
        entry.blocks[0].operations[2].clone(),
    ];
    let OperationKind::Call {
        callee: target,
        arguments,
        ..
    } = &mut prefix[1].kind
    else {
        unreachable!()
    };
    *target = callee;
    arguments.truncate(1);
    let branch_condition = if inverted {
        let result = ValueId::new(29_001).unwrap();
        prefix.push(Operation {
            static_reach_binding: None,
            id: OperationId::new(29_001).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanNot { operand: condition },
        });
        result
    } else {
        condition
    };
    entry.entry = branch.entry;
    entry.blocks = branch.blocks;
    entry.blocks[0].operations = prefix;
    let Terminator::Conditional { condition, .. } = &mut entry.blocks[0].terminator else {
        unreachable!()
    };
    *condition = branch_condition;
    (terminal_codec::encode_module(&module).unwrap(), proof)
}
