//! Two generated Unit calls retain observable byte output and caller continuation.
use super::*;
use semantic_vocabulary::{BlockId, ContractId, MachineId};

fn unit_byte_output_calls_module() -> TerminalModule {
    let mut module = widening::widened_byte_output_module();
    let mut caller = module.machines[0].clone();
    caller.id = MachineId::new(100).unwrap();
    caller.entry = BlockId::new(101).unwrap();
    caller.contract.id = ContractId::new(110).unwrap();
    caller.parameters[0].id = ValueId::new(103).unwrap();
    caller.blocks[0].id = caller.entry;
    let call = |identity, value| Operation {
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: module.entry,
            arguments: vec![value],
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    caller.blocks[0].operations = vec![
        call(105, caller.parameters[0].id),
        Operation {
            id: OperationId::new(106).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(106).unwrap(),
                scalar_type: caller.parameters[0].scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(33),
            },
        },
        call(107, ValueId::new(106).unwrap()),
    ];
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(109).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    module.entry = caller.id;
    module.machines.push(caller);
    module
}

#[test]
fn scalar_only_unit_byte_calls_publish_on_linux_targets() {
    let module = unit_byte_output_calls_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let placed = stage_byte_output_module(target, &module);
        let calls = &placed.text_section().resolved_internal_machine_calls;
        assert_eq!(
            calls
                .iter()
                .map(|call| (call.caller, call.operation, call.callee))
                .collect::<Vec<_>>(),
            [105, 107].map(|operation| (
                module.entry,
                OperationId::new(operation).unwrap(),
                MachineId::new(1).unwrap()
            ))
        );
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let caller = object.entry_function();
        assert_eq!(caller.internal_unit_calls.len(), 2);
        for (call, source_value) in caller.internal_unit_calls.iter().zip([103, 106]) {
            assert!(call.result.is_none());
            assert!(call.semantic_result.is_none());
            assert!(call.structural_result.is_none());
            assert!(call.arguments.is_empty());
            assert_eq!(call.scalar_arguments.len(), 1);
            assert!(matches!(
                call.scalar_arguments[0].source,
                machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { .. }
            ));
            assert_eq!(
                call.scalar_arguments[0].source.source_value(),
                ValueId::new(source_value).unwrap()
            );
        }
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

#[test]
fn scalar_only_unit_byte_calls_reject_changed_publication_custody() {
    let module = unit_byte_output_calls_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(
                stage_byte_output_module(target, &module),
            )
            .unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        for mutation in [
            "order",
            "callee",
            "source",
            "type",
            "instruction",
            "destination",
            "parameter_index",
            "argument_span",
            "result",
            "span",
        ] {
            let mut changed = object.clone();
            let caller = changed
                .functions_mut_for_test()
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let calls = &mut caller.internal_unit_calls;
            match mutation {
                "order" => calls.swap(0, 1),
                "callee" => calls[0].target = module.entry,
                "result" => calls[0].result = Some(module.machines[0].parameters[0].scalar_type),
                "span" => calls[0].code_offset += 1,
                _ => mutate_selected_argument(&mut calls[0], mutation),
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
        let mut missing_replay = object.clone();
        missing_replay.clear_fragment_replay_for_test();
        // Isolate selected-call replay: the earlier selected-boundary guard
        // must not be what rejects this artifact.
        missing_replay.boundary_settlements_mut_for_test().clear();
        let error = image_emission::emit_executable_image(&missing_replay, 3).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("selected Unit call requires common-pipeline replay evidence")
        );
    }
}

fn mutate_selected_argument(call: &mut machine_code::InternalUnitCallRecord, mutation: &str) {
    let argument = &mut call.scalar_arguments[0];
    let machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
        source_value,
        scalar_type,
        instruction,
    } = &mut argument.source
    else {
        panic!("ordinary Unit call retains selected transport")
    };
    match mutation {
        "source" => *source_value = ValueId::new(106).unwrap(),
        "type" => {
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
        }
        "instruction" => instruction.0 += 1,
        "destination" => {
            let calling_conventions::ValueLocation::Register {
                value_byte_offset, ..
            } = &mut argument.destination.locations[0]
            else {
                panic!("first byte argument uses the native register ABI")
            };
            *value_byte_offset += 1;
        }
        "parameter_index" => argument.parameter_index += 1,
        "argument_span" => argument.code_offset += 1,
        _ => panic!("unknown selected argument mutation: {mutation}"),
    }
}

#[test]
fn scalar_only_unit_byte_calls_installation_requires_the_admitted_image() {
    let module = unit_byte_output_calls_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(
                stage_byte_output_module(target, &module),
            )
            .unwrap(),
        );
        let object = image_emission::build_function_fragment_object_artifact(source).unwrap();
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        image_emission::validate_installation_record(&record, &image).unwrap();
        for mutation in ["source", "instruction"] {
            let mut changed = record.clone();
            let call = &mut changed.internal_unit_calls_mut_for_test()[0].custody;
            assert_eq!(
                call.scalar_arguments[0].source.source_value(),
                ValueId::new(103).unwrap()
            );
            mutate_selected_argument(call, mutation);
            // A nonzero source/instruction with the right ABI shape is a
            // well-formed record, not authority to replace admitted custody.
            let encoded = image_emission::encode_installation_record(&changed).unwrap();
            let decoded = image_emission::decode_installation_record(&encoded).unwrap();
            assert!(
                image_emission::validate_installation_record(&decoded, &image).is_err(),
                "{mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn scalar_only_unit_byte_calls_execute_in_order_and_continue_on_linux() {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        let module = unit_byte_output_calls_module();
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
            #include <unistd.h>
            extern void omega_entry(uint8_t byte);
            int main(void) {
                alarm(10);
                int channel[2];
                if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned byte = 0; byte < 256; ++byte) omega_entry((uint8_t)byte);
                if (dup2(saved, STDOUT_FILENO) < 0) return 3;
                close(saved);
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned byte = 0; byte < 256; ++byte) {
                        uint8_t actual;
                        if (read(channel[0], &actual, 1) != 1 || actual != byte) return 4;
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
        "SKIP: observable Linux byte-output calls require a Linux host; no macOS provider substitute"
    );
}
