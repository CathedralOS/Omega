//! Scalar call results cross two ordinary machine boundaries and the final return.

use crate::tests::*;

fn artifact(value: u64) -> (Vec<u8>, Vec<u8>) {
    let mut entry = conditional_u64_integer_equal_parameters_machine(28_000, [1, 0]);
    let mut middle = conditional_u64_integer_equal_parameters_machine(28_100, [1, 0]);
    let mut leaf = conditional_u64_integer_equal_parameters_machine(28_200, [1, 0]);
    for machine in [&mut entry, &mut middle, &mut leaf] {
        machine.parameters.truncate(1);
        machine.blocks.truncate(1);
        machine.blocks[0].operations.clear();
    }
    let scalar_type = leaf.parameters[0].scalar_type;
    leaf.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(28_220).unwrap(),
        value: leaf.parameters[0].id,
        cleanup_actions: Vec::new(),
    };
    let call = |operation, result, callee, argument| Operation {
        id: OperationId::new(operation).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(result).unwrap(),
            scalar_type,
        }),
        kind: OperationKind::Call {
            callee,
            arguments: vec![argument],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    middle.blocks[0]
        .operations
        .push(call(28_120, 28_121, leaf.id, middle.parameters[0].id));
    middle.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(28_122).unwrap(),
        value: ValueId::new(28_121).unwrap(),
        cleanup_actions: Vec::new(),
    };
    entry.parameters.clear();
    let constant = ValueId::new(28_020).unwrap();
    entry.blocks[0].operations.push(Operation {
        id: OperationId::new(28_021).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: constant,
            scalar_type,
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(u128::from(value)),
        },
    });
    entry.blocks[0]
        .operations
        .push(call(28_022, 28_023, middle.id, constant));
    entry.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(28_024).unwrap(),
        value: ValueId::new(28_023).unwrap(),
        cleanup_actions: Vec::new(),
    };
    let module = conditional_immediate_module(entry.id, vec![entry, middle, leaf]);
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}

fn preserving_artifact(value: u64) -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = artifact(value);
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    // Both inputs affect the result. A lost original parameter or a lost
    // earlier call result makes the second equality return zero.
    let leaf = conditional_u64_integer_equal_parameters_machine(28_200, [u128::from(value), 0]);
    let middle = &mut module.machines[1];
    let parameter = middle.parameters[0].id;
    let mut second = middle.blocks[0].operations[0].clone();
    let OperationKind::Call { arguments, .. } = &mut middle.blocks[0].operations[0].kind else {
        unreachable!()
    };
    arguments.push(parameter);
    second.id = OperationId::new(28_130).unwrap();
    let OperationResult::Scalar(result) = &mut second.result else {
        unreachable!()
    };
    result.id = ValueId::new(28_131).unwrap();
    let OperationKind::Call { arguments, .. } = &mut second.kind else {
        unreachable!()
    };
    *arguments = vec![ValueId::new(28_121).unwrap(), parameter];
    middle.blocks[0].operations.push(second);
    let Terminator::Return { value, .. } = &mut middle.blocks[0].terminator else {
        unreachable!()
    };
    *value = ValueId::new(28_131).unwrap();
    module.machines[2] = leaf;
    (terminal_codec::encode_module(&module).unwrap(), proof)
}

fn discarded_call_result_artifact(value: u64) -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = artifact(value);
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let middle = &mut module.machines[1];
    let mut second = middle.blocks[0].operations[0].clone();
    second.id = OperationId::new(28_130).unwrap();
    let OperationResult::Scalar(result) = &mut second.result else {
        unreachable!()
    };
    result.id = ValueId::new(28_131).unwrap();
    let OperationKind::Call { arguments, .. } = &mut second.kind else {
        unreachable!()
    };
    *arguments = vec![ValueId::new(28_121).unwrap()];
    middle.blocks[0].operations.push(second);
    // Return the first call's result. The second call must still execute:
    // ordered calls are not reconstructed from the returned value's ancestry.
    (terminal_codec::encode_module(&module).unwrap(), proof)
}

#[test]
fn scalar_returning_multihop_calls_reach_common_native_stages() {
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    eprintln!(
        "SKIP: Windows native execution requires a Windows x86-64 host; cross-target replay still runs"
    );
    let value = 37_u64;
    for (semantic, proof) in [
        artifact(value),
        preserving_artifact(value),
        discarded_call_result_artifact(value),
    ] {
        let source = terminal_codec::decode_module(&semantic).unwrap();
        let expected_calls = source
            .machines
            .iter()
            .flat_map(|machine| {
                machine.blocks.iter().flat_map(move |block| {
                    block
                        .operations
                        .iter()
                        .filter_map(move |operation| match operation.kind {
                            OperationKind::Call { callee, .. } => {
                                Some((machine.id, operation.id, callee))
                            }
                            _ => None,
                        })
                })
            })
            .collect::<std::collections::BTreeSet<_>>();
        let interpreted = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
        )
        .expect("ordinary scalar call semantics must independently verify and execute");
        assert!(matches!(
            interpreted.value(),
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(actual), ..
                }
            ) if actual == u128::from(value)
        ));
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::windows_x64(),
            NativeTarget::macos_arm64(),
        ] {
            let materialization = match target.architecture {
                target::Architecture::X86_64 => {
                    Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
                }
                target::Architecture::Aarch64 => {
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                }
            };
            for choices in [
                Vec::new(),
                vec![Optimization::CopyPropagation],
                vec![materialization],
            ] {
                let selections = OptimizationSelections::new(choices).unwrap();
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    compiler_baseline_request_v1(&selections),
                )
                .unwrap();
                let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized, target, &[],
            ).unwrap_or_else(|error| panic!(
                "scalar-returning calls must use common physical stages: {target:?}, {selections:?}: {error:?}"
            ));
                let emitted = stage_optimized_function_fragment_emission(
                    physical.into_function_fragment_emission_source(),
                )
                .unwrap();
                let framed = stage_function_fragment_frame_application(emitted).unwrap();
                let text = stage_optimized_fixed_frame_text_section(framed).unwrap();
                assert_eq!(
                    text.text_section()
                        .resolved_internal_machine_calls
                        .iter()
                        .map(|call| (call.caller, call.operation, call.callee))
                        .collect::<std::collections::BTreeSet<_>>(),
                    expected_calls,
                );
                let object = stage_optimized_relocation_free_object_container(text).unwrap();
                validate_optimized_relocation_free_object_container(&object).unwrap();
                let published =
                    image_emission::build_function_fragment_object_artifact(&object).unwrap();
                image_emission::validate_function_fragment_object_artifact(&object, &published)
                    .unwrap();
                let image = image_emission::emit_executable_image(&published, 3).unwrap();
                image_emission::validate_executable_image(&published, &image).unwrap();
                let record = image_emission::build_installation_record(
                    &image,
                    semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
                )
                .unwrap();
                let encoded = image_emission::encode_installation_record(&record).unwrap();
                let decoded = image_emission::decode_installation_record(&encoded).unwrap();
                image_emission::validate_installation_record(&decoded, &image).unwrap();
                assert_eq!(
                    image_emission::derive_installation_stack_demand(
                        &decoded,
                        &image,
                        published.entry()
                    )
                    .unwrap(),
                    image_emission::derive_stack_demand(&published, published.entry()).unwrap(),
                );
                #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
                if target == NativeTarget::windows_x64() {
                    let text = object.source().text_section();
                    let code = super::native_execution::Code::new(&text.bytes);
                    assert_eq!(
                        code.call_scalar(
                            usize::try_from(text.semantic_entry_offset).unwrap(),
                            [0; 4]
                        ),
                        value,
                        "compiled parameter/result flow must agree with Terminal interpretation"
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_returning_calls_reject_changed_result_callee_and_provenance() {
    let (semantic, proof) = preserving_artifact(37);
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let staged = stage_optimized_instruction_selection(
        lower_optimized_to_target_operations(optimized, NativeTarget::windows_x64()).unwrap(),
    )
    .unwrap();
    let original = staged.selected().plan();
    let middle = original
        .functions
        .iter()
        .position(|function| function.machine.get() == 28_101)
        .unwrap();
    let first_call = original.functions[middle].blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. }))
        .unwrap();

    let mut changed = original.clone();
    changed.functions[middle].blocks[0].instructions[first_call].kind =
        SelectedInstructionKind::CallI64 {
            callee: MachineId::new(28_001).unwrap(),
        };
    assert!(validate_raw_selection(&staged, changed).is_err());

    let mut changed = original.clone();
    changed.functions[middle].blocks[0].instructions[first_call]
        .provenance
        .operations = vec![OperationId::new(28_022).unwrap()];
    assert!(validate_raw_selection(&staged, changed).is_err());

    let mut changed = original.clone();
    let original_parameter = changed.functions[middle]
        .virtual_registers
        .iter()
        .find(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::EntryParameter { .. }
            )
        })
        .unwrap()
        .id;
    let SelectedTerminator::Return { instruction, .. } =
        &mut changed.functions[middle].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_ne!(instruction.operands[0].virtual_register, original_parameter);
    instruction.operands[0].virtual_register = original_parameter;
    assert!(validate_raw_selection(&staged, changed).is_err());
}
