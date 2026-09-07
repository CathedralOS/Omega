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

#[test]
fn direct_boolean_target_parity_requires_exact_source_and_abi_custody() {
    for inverted in [false, true] {
        let (semantic, proof) = boolean_call_artifact(inverted);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                compiler_baseline_request_v1(&OptimizationSelections::default()),
            )
            .unwrap();
            let staged = lower_optimized_to_target_operations(optimized, target).unwrap();
            let original = staged.target_operations();
            let source = staged.optimized().plan();
            let unit = staged.optimized().unit();
            let legalized = legalize_target_operations(original, source, unit).unwrap();
            let entry_index = original
                .functions
                .iter()
                .position(|function| function.machine == original.entry)
                .unwrap();
            for mutation in 0..6 {
                let mut changed = original.clone();
                let function = &mut changed.functions[entry_index];
                let TargetOperation::ReturnIntegerConditionalControl {
                    condition_source,
                    condition_parameter_index,
                    when_true,
                    when_false,
                    ..
                } = &mut function.operation
                else {
                    panic!("canonical direct Boolean target")
                };
                match mutation {
                    0 => std::mem::swap(when_true, when_false),
                    1 => *condition_parameter_index = 1,
                    2 => *condition_source = ValueId::new(999_999).unwrap(),
                    // A real source value is not interchangeable with the Not result.
                    3 => {
                        *condition_source =
                            ValueId::new(if inverted { 29_000 } else { 29_001 }).unwrap()
                    }
                    4 => when_true.psi_edge = when_false.psi_edge,
                    _ => {
                        if inverted {
                            let operation = function
                                .provenance
                                .operations
                                .iter_mut()
                                .find(|operation| operation.get() == 29_001)
                                .unwrap();
                            *operation = OperationId::new(999_999).unwrap();
                        } else {
                            function
                                .provenance
                                .operations
                                .push(OperationId::new(29_001).unwrap());
                        }
                    }
                }
                assert!(
                    legalize_target_operations(&changed, source, unit).is_err(),
                    "inverted {inverted}, target {target:?}, mutation {mutation}"
                );
                assert!(
                    validate_legalized_operations(&changed, source, unit, legalized.plan().clone())
                        .is_err(),
                    "independent replay: inverted {inverted}, mutation {mutation}"
                );
            }
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
            id: OperationId::new(29_001).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
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
