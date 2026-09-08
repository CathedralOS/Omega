//! Scalar arrivals select the byte observed by a real Unit output call.
use super::*;
use semantic_vocabulary::BlockId;

fn scalar_transfer_output_module() -> TerminalModule {
    let mut module = unit_control::conditional_unit_byte_output_module();
    let caller = module.machines.last_mut().unwrap();
    let byte_type = caller.parameters[0].scalar_type;
    let mut output = caller.blocks[1].operations.remove(0);
    caller.blocks[2].operations.pop().unwrap();
    caller.blocks[1].parameters.push(ValueDeclaration {
        id: ValueId::new(151).unwrap(),
        scalar_type: byte_type,
    });
    let Terminator::Conditional { when_true, .. } = &mut caller.blocks[0].terminator else {
        panic!("entry selector")
    };
    when_true.arguments.push(ValueId::new(103).unwrap());
    for (block, source) in [(1, 151), (2, 132)] {
        let Terminator::Jump { arguments, .. } = &mut caller.blocks[block].terminator else {
            panic!("join arrival")
        };
        arguments.push(ValueId::new(source).unwrap());
    }
    let joined = ValueId::new(150).unwrap();
    caller.blocks[3].parameters.push(ValueDeclaration {
        id: joined,
        scalar_type: byte_type,
    });
    let OperationKind::CallUnit { arguments, .. } = &mut output.kind else {
        panic!("output call")
    };
    arguments[0] = joined;
    caller.blocks[3].operations.insert(0, output);
    module
}

fn scalar_guard_transfer_module(boolean: bool) -> TerminalModule {
    let mut module = scalar_transfer_output_module();
    let caller = module.machines.last_mut().unwrap();
    let scalar_type = if boolean {
        ScalarType::Boolean
    } else {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    };
    caller.parameters[1].scalar_type = scalar_type;
    let transferred = ValueId::new(152).unwrap();
    caller.blocks[0].parameters.push(ValueDeclaration {
        id: transferred,
        scalar_type,
    });
    let mut bridge = caller.blocks[0].clone();
    bridge.id = BlockId::new(160).unwrap();
    bridge.parameters.clear();
    bridge.terminator = Terminator::Jump {
        edge: EdgeId::new(160).unwrap(),
        target: caller.entry,
        arguments: vec![ValueId::new(104).unwrap()],
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let condition_value = if boolean {
        transferred
    } else {
        caller.blocks[0].operations.extend([
            Operation {
                id: OperationId::new(161).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(161).unwrap(),
                    scalar_type,
                }),
                kind: OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Unsigned(0),
                },
            },
            Operation {
                id: OperationId::new(162).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(162).unwrap(),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::IntegerEqual {
                    left: transferred,
                    right: ValueId::new(161).unwrap(),
                },
            },
        ]);
        ValueId::new(162).unwrap()
    };
    let Terminator::Conditional { condition, .. } = &mut caller.blocks[0].terminator else {
        panic!("transferred guard")
    };
    *condition = condition_value;
    caller.entry = bridge.id;
    caller.blocks.push(bridge);
    caller.blocks.sort_unstable_by_key(|block| block.id);
    module
}

fn paired_scalar_transfer_module() -> TerminalModule {
    let mut module = scalar_transfer_output_module();
    let caller = module.machines.last_mut().unwrap();
    let byte_type = caller.parameters[0].scalar_type;
    let mut constant = caller.blocks[2].operations[0].clone();
    constant.id = OperationId::new(156).unwrap();
    constant.result = OperationResult::Scalar(ValueDeclaration {
        id: ValueId::new(156).unwrap(),
        scalar_type: byte_type,
    });
    caller.blocks[1].operations.push(constant);
    for (block, value) in [(1, 156), (2, 103)] {
        let Terminator::Jump { arguments, .. } = &mut caller.blocks[block].terminator else {
            panic!("paired arrival")
        };
        arguments.push(ValueId::new(value).unwrap());
    }
    caller.blocks[3].parameters.push(ValueDeclaration {
        id: ValueId::new(155).unwrap(),
        scalar_type: byte_type,
    });
    let mut output = caller.blocks[3].operations[0].clone();
    output.id = OperationId::new(157).unwrap();
    let OperationKind::CallUnit { arguments, .. } = &mut output.kind else {
        panic!("paired output")
    };
    arguments[0] = ValueId::new(155).unwrap();
    caller.blocks[3].operations.insert(1, output);
    module
}

fn same_target_transfer_module() -> TerminalModule {
    let mut module = scalar_transfer_output_module();
    let caller = module.machines.last_mut().unwrap();
    let constant = caller.blocks[2].operations.remove(0);
    caller.blocks[0].operations.push(constant);
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut caller.blocks[0].terminator
    else {
        panic!("same-target selector")
    };
    when_false.target = when_true.target;
    when_false.arguments = vec![ValueId::new(132).unwrap()];
    caller.blocks.remove(2);
    module
}

fn literal_boolean_transfer_module() -> TerminalModule {
    let mut module = scalar_guard_transfer_module(true);
    let caller = module.machines.last_mut().unwrap();
    let entry = caller
        .blocks
        .iter_mut()
        .find(|block| block.id == caller.entry)
        .unwrap();
    entry.operations.push(Operation {
        id: OperationId::new(161).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(161).unwrap(),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: false },
    });
    let Terminator::Jump { arguments, .. } = &mut entry.terminator else {
        panic!("literal arrival")
    };
    arguments[0] = ValueId::new(161).unwrap();
    module
}

#[test]
fn scalar_transfer_unit_output_rejects_target_binding_substitution() {
    let module = same_target_transfer_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let semantic = terminal_codec::encode_module(&module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap();
        let selections = OptimizationSelections::new([]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled = abstract_operations_to_target_operations::lower_optimized_to_target_operations_with_provider_executions(
            optimized, target, &[AdmittedBoundarySettlement {
                boundary: module.boundary_machines[0].id,
                execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::LinuxWriteByteI32),
                realization: LinuxWriteByteI32Realization.into(),
            }],
        ).unwrap();
        let validate = |raw: &target_operations::TargetOperationPlan| {
            target_operations_to_selected_instructions::legalize_target_operations(
                raw,
                compiled.optimized().plan(),
                compiled.optimized().unit(),
            )
        };
        validate(compiled.target_operations()).unwrap();
        for mutation in ["argument", "parameter", "type", "missing", "edge"] {
            let mut raw = compiled.target_operations().clone();
            let caller = raw
                .functions
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let target_operations::TargetOperation::UnitGraph(graph) = &mut caller.operation else {
                panic!("ordinary Unit graph")
            };
            let entry = graph
                .blocks
                .iter_mut()
                .find(|block| block.block == graph.entry)
                .unwrap();
            let target_operations::TargetUnitTerminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut entry.terminator
            else {
                panic!("conditional arrival")
            };
            assert_eq!(when_true.target, when_false.target);
            assert_ne!(
                when_true.bindings[0].argument,
                when_false.bindings[0].argument
            );
            match mutation {
                "argument" => when_true.bindings[0].argument = when_false.bindings[0].argument,
                "parameter" => when_true.bindings[0].parameter = ValueId::new(150).unwrap(),
                "type" => when_true.bindings[0].scalar_type = ScalarType::Boolean,
                "missing" => when_true.bindings.clear(),
                "edge" => when_true.psi_edge = when_false.psi_edge,
                _ => unreachable!(),
            }
            assert!(validate(&raw).is_err(), "{mutation} on {target:?}");
        }
    }
}

fn publish_transfer(
    target: NativeTarget,
    module: &TerminalModule,
) -> (image_emission::ExecutableImage, usize) {
    let text = stage_byte_output_module(target, module);
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let encoded = image_emission::encode_installation_record(&record).unwrap();
    let decoded = image_emission::decode_installation_record(&encoded).unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    (image, object.entry_function().text_offset)
}

fn assert_transfer_publication(module: &TerminalModule) {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (image, _) = publish_transfer(target, module);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

#[test]
fn scalar_transfer_unit_output_publishes_boolean_arrival() {
    assert_transfer_publication(&scalar_guard_transfer_module(true));
}

#[test]
fn scalar_transfer_unit_output_publishes_u64_arrival() {
    assert_transfer_publication(&scalar_guard_transfer_module(false));
}

#[test]
fn scalar_transfer_unit_output_paired_arrivals_require_physical_edge_copies() {
    assert_edge_copy_fence(&paired_scalar_transfer_module());
}

#[test]
fn scalar_transfer_unit_output_same_target_arrivals_require_physical_edge_copies() {
    assert_edge_copy_fence(&same_target_transfer_module());
}

#[test]
fn scalar_transfer_unit_output_publishes_literal_boolean_arrival() {
    assert_transfer_publication(&literal_boolean_transfer_module());
}

fn assert_edge_copy_fence(module: &TerminalModule) {
    use selected_instructions_to_register_homes::{
        OptimizedRegisterHomeCustodyError, RegisterAllocationError, RegisterHomeError,
    };
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let semantic = terminal_codec::encode_module(module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap();
        let selections = OptimizationSelections::new([]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled = abstract_operations_to_target_operations::lower_optimized_to_target_operations_with_provider_executions(
            optimized, target, &[AdmittedBoundarySettlement {
                boundary: module.boundary_machines[0].id,
                execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::LinuxWriteByteI32),
                realization: LinuxWriteByteI32Realization.into(),
            }],
        ).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let selected =
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                compiled,
                environment,
            )
            .unwrap();
        // Selection independently accepts these semantic bindings. Allocation
        // rejects their unavailable physical edge-copy realization.
        let optimized =
            selected_instructions_to_selected_instructions::optimize_selected_instructions(
                selected,
            )
            .unwrap();
        let error =
            match selected_instructions_to_register_homes::stage_register_allocation(optimized) {
                Ok(_) => panic!(
                    "edge-copy support now available; promote fixture to publication/runtime"
                ),
                Err(error) => error,
            };
        assert_eq!(
            error,
            RegisterAllocationError::Homes(OptimizedRegisterHomeCustodyError::Assignment(
                RegisterHomeError::UnsupportedEdgeTransfer {
                    function: 1,
                    edge: 111,
                }
            ),),
            "{target:?}"
        );
    }
}

#[test]
fn scalar_transfer_unit_output_executes_coalescible_arrivals_and_continues() {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    for (mode, module) in [
        (0, scalar_transfer_output_module()),
        (1, scalar_guard_transfer_module(true)),
        (2, scalar_guard_transfer_module(false)),
        (5, literal_boolean_transfer_module()),
    ] {
        let (image, offset) = publish_transfer(NativeTarget::host(), &module);
        let signature = if mode == 2 { "uint64_t" } else { "bool" };
        let oracle = r#"
            #include <stdint.h>
            #include <stdbool.h>
            #include <unistd.h>
            extern void omega_entry(uint8_t byte, SELECTOR_TYPE selector);
            int main(void) {
                alarm(10);
                int channel[2]; if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                uint8_t expected[3072]; unsigned count = 0;
                const uint64_t selectors[] = {0, 1, UINT64_MAX};
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned byte = 0; byte < 256; ++byte)
                        for (unsigned choice = 0; choice < (MODE == 2 ? 3 : 2); ++choice) {
                            uint64_t selector = selectors[choice];
                            omega_entry((uint8_t)byte, selector);
                            bool selected = MODE == 5 ? false : MODE == 2 ? selector == 0 : selector != 0;
                            expected[count++] = selected ? byte : '?';
                            expected[count++] = '!';
                        }
                if (dup2(saved, STDOUT_FILENO) < 0) return 3;
                close(saved);
                for (unsigned position = 0; position < count; ++position) {
                    uint8_t actual;
                    if (read(channel[0], &actual, 1) != 1 || actual != expected[position]) return 4;
                }
                uint8_t extra; if (read(channel[0], &extra, 1) != 0) return 5;
                close(channel[0]); return 0;
            }
        "#.replace("SELECTOR_TYPE", signature).replace("MODE", &mode.to_string());
        native_function::assert_c_text(&image.output().final_text_bytes, offset, &oracle);
    }
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: scalar-transfer byte output requires a Linux host; no macOS provider substitute"
    );
}

#[test]
fn scalar_transfer_unit_output_publishes_on_linux_targets() {
    let module = scalar_transfer_output_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let text = stage_byte_output_module(target, &module);
        let calls = &text.text_section().resolved_internal_machine_calls;
        assert_eq!(calls.len(), 2);
        for identity in [105, 107] {
            assert!(
                calls
                    .iter()
                    .any(|call| call.operation == OperationId::new(identity).unwrap()
                        && call.caller == module.entry
                        && call.callee == module.machines[0].id)
            );
        }
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        let encoded = image_emission::encode_installation_record(&record).unwrap();
        let decoded = image_emission::decode_installation_record(&encoded).unwrap();
        image_emission::validate_installation_record(&decoded, &image).unwrap();
    }
}
