//! Conditional Unit effects converge before the caller's final output.
use super::*;
use semantic_vocabulary::BlockId;
use terminal_psi::SuccessorEdge;

pub(super) fn conditional_unit_byte_output_module() -> TerminalModule {
    let mut module = unit_calls::unit_byte_output_calls_module();
    let caller = module.machines.last_mut().unwrap();
    let selector = ValueId::new(104).unwrap();
    caller.parameters.push(ValueDeclaration {
        id: selector,
        scalar_type: ScalarType::Boolean,
    });
    let mut selected = caller.blocks[0].clone();
    selected.id = BlockId::new(120).unwrap();
    selected.operations.truncate(1);
    let jump = |edge| Terminator::Jump {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(140).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    selected.terminator = jump(121);
    let mut fallback = selected.clone();
    fallback.id = BlockId::new(130).unwrap();
    fallback.terminator = jump(131);
    fallback.operations[0].id = OperationId::new(133).unwrap();
    let OperationKind::CallUnit { arguments, .. } = &mut fallback.operations[0].kind else {
        panic!("Unit byte helper call")
    };
    arguments[0] = ValueId::new(132).unwrap();
    fallback.operations.insert(
        0,
        Operation {
            id: OperationId::new(132).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(132).unwrap(),
                scalar_type: caller.parameters[0].scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(63),
            },
        },
    );
    let mut continuation = caller.blocks[0].clone();
    continuation.id = BlockId::new(140).unwrap();
    continuation.operations.remove(0);
    let successor = |edge, target| SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks[0].operations.clear();
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: selector,
        when_true: successor(111, 120),
        when_false: successor(112, 130),
    };
    caller.blocks.extend([selected, fallback, continuation]);
    module
}

#[test]
fn conditional_unit_byte_calls_publish_on_linux_targets() {
    let module = conditional_unit_byte_output_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let text = stage_byte_output_module(target, &module);
        let calls = &text.text_section().resolved_internal_machine_calls;
        assert_eq!(calls.len(), 3);
        for identity in [105, 133, 107] {
            let call = calls
                .iter()
                .find(|call| call.operation == OperationId::new(identity).unwrap())
                .unwrap();
            assert_eq!(call.caller, module.entry);
            assert_eq!(call.callee, module.machines[0].id);
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
        for mutation in ["edge", "source", "callee"] {
            let mut changed = object.clone();
            let caller = changed
                .functions_mut_for_test()
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            match mutation {
                "edge" => caller.provenance.edges[0] = EdgeId::new(999).unwrap(),
                "source" => {
                    let call = caller
                        .internal_unit_calls
                        .iter_mut()
                        .find(|call| {
                            call.scalar_arguments[0].source.source_value()
                                == ValueId::new(103).unwrap()
                        })
                        .unwrap();
                    let machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        source_value,
                        ..
                    } = &mut call.scalar_arguments[0].source
                    else {
                        panic!("selected Unit call source")
                    };
                    *source_value = ValueId::new(132).unwrap();
                }
                "callee" => caller.internal_unit_calls[0].target = module.entry,
                _ => unreachable!(),
            }
            assert!(
                image_emission::validate_function_fragment_object_artifact(&source, &changed)
                    .is_err(),
                "{mutation} on {target:?}"
            );
            assert!(
                image_emission::emit_executable_image(&changed, 3).is_err(),
                "{mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn conditional_unit_byte_calls_reject_changed_selected_control() {
    use selected_instructions::SelectedTerminator;
    use target_operations_to_selected_instructions::{
        selection_constraints, stage_optimized_instruction_selection,
        validate_selected_instructions,
    };
    let module = conditional_unit_byte_output_module();
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
                execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::HostedWriteByteI32),
                realization: HostedWriteByteI32Realization.into(),
            }],
        ).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let staged = stage_optimized_instruction_selection(compiled, environment).unwrap();
        let environment = staged.register_environment();
        let constraints = selection_constraints(staged.legalized(), environment);
        let validate = |raw| {
            validate_selected_instructions(
                staged.legalized(),
                &constraints,
                environment.physical(),
                environment.constraints(),
                raw,
            )
        };
        validate(staged.selected().plan().clone()).unwrap();
        let caller = staged
            .selected()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        let entry = caller
            .blocks
            .iter()
            .find(|block| {
                block.origin
                    == selected_instructions::SelectedBlockOrigin::Source(
                        BlockId::new(101).unwrap(),
                    )
            })
            .unwrap();
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &entry.terminator
        else {
            panic!("runtime Boolean retains ordinary conditional control")
        };
        assert_eq!(
            (when_nonzero.psi_edge, when_nonzero.source_target),
            (EdgeId::new(111).unwrap(), BlockId::new(120).unwrap())
        );
        assert_eq!(
            (when_zero.psi_edge, when_zero.source_target),
            (EdgeId::new(112).unwrap(), BlockId::new(130).unwrap())
        );
        for mutation in ["polarity", "target", "edge", "condition"] {
            let mut raw = staged.selected().plan().clone();
            let caller = raw
                .functions
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let selector_register = caller.virtual_registers.iter().find(|register| matches!(
                register.origin,
                selected_instructions::VirtualRegisterOrigin::EntryParameter { source_value, .. }
                    if source_value == ValueId::new(104).unwrap()
            )).unwrap().id;
            let entry = caller
                .blocks
                .iter_mut()
                .find(|block| {
                    block.origin
                        == selected_instructions::SelectedBlockOrigin::Source(
                            BlockId::new(101).unwrap(),
                        )
                })
                .unwrap();
            let SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } = &mut entry.terminator
            else {
                panic!("runtime Boolean retains ordinary conditional control")
            };
            match mutation {
                "polarity" => std::mem::swap(when_nonzero, when_zero),
                "target" => when_nonzero.block = when_zero.block,
                "edge" => when_nonzero.psi_edge = when_zero.psi_edge,
                "condition" => {
                    let normalization = entry
                        .instructions
                        .iter()
                        .find(|instruction| {
                            instruction.kind
                                == selected_instructions::SelectedInstructionKind::ZeroExtendU8
                                && instruction.provenance.values == [ValueId::new(104).unwrap()]
                        })
                        .unwrap();
                    assert_eq!(
                        normalization.operands[0].virtual_register,
                        selector_register
                    );
                    let normalized_selector = normalization.operands[1].virtual_register;
                    let comparison = entry
                        .instructions
                        .iter_mut()
                        .find(|instruction| {
                            instruction.kind
                                == selected_instructions::SelectedInstructionKind::CompareI64Zero
                        })
                        .unwrap();
                    assert_eq!(comparison.provenance.values, [ValueId::new(104).unwrap()]);
                    assert_eq!(comparison.operands[0].virtual_register, normalized_selector);
                    comparison.operands[0].virtual_register =
                        selected_instructions::VirtualRegisterId(u32::MAX);
                }
                _ => unreachable!(),
            }
            assert!(validate(raw).is_err(), "{mutation} on {target:?}");
        }
    }
}

#[test]
fn conditional_unit_byte_calls_execute_both_choices_and_continue_on_linux() {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        let module = conditional_unit_byte_output_module();
        let text = stage_byte_output_module(NativeTarget::host(), &module);
        let placed = text.text_section();
        let entry = placed
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        native_function::assert_c_text(
            &placed.bytes,
            entry.section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            #include <stdbool.h>
            #include <unistd.h>
            extern void omega_entry(uint8_t byte, bool selector);
            int main(void) {
                alarm(10);
                int channel[2];
                if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned byte = 0; byte < 256; ++byte)
                        for (unsigned selector = 0; selector < 2; ++selector)
                            omega_entry((uint8_t)byte, selector != 0);
                if (dup2(saved, STDOUT_FILENO) < 0) return 3;
                close(saved);
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned byte = 0; byte < 256; ++byte)
                        for (unsigned selector = 0; selector < 2; ++selector) {
                            uint8_t actual;
                            if (read(channel[0], &actual, 1) != 1 || actual != (selector ? byte : '?')) return 4;
                            if (read(channel[0], &actual, 1) != 1 || actual != '!') return 5;
                        }
                uint8_t extra;
                if (read(channel[0], &extra, 1) != 0) return 6;
                close(channel[0]);
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: conditional Linux byte-output runtime requires a Linux host; no macOS provider substitute"
    );
}
