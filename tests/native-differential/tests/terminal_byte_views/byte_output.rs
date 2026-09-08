//! A generated Unit function invokes the real returning Linux byte-output leaf.
use super::*;
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{BoundaryMachineId, EdgeId, IntegerSign, IntegerType, ScalarType};
use target_operations::{CompilerBuiltinExecution, LinuxWriteByteI32Realization};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, Operation, OperationResult,
    TerminalMachineResult, Terminator, ValueDeclaration,
};
#[path = "byte_output/unit_calls.rs"]
mod unit_calls;
#[path = "byte_output/widening.rs"]
mod widening;

fn byte_output_module() -> TerminalModule {
    let mut module = fixtures::byte_view_length_module();
    module.structural_types.clear();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let boundary = BoundaryMachineId::new(1).unwrap();
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: "Console::write_byte(i32)->Unit".into(),
        attachment: None,
        scalar_parameters: vec![scalar_type],
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.structural_places.clear();
    let value = ValueId::new(5).unwrap();
    machine.parameters = vec![ValueDeclaration {
        id: value,
        scalar_type,
    }];
    machine.result = TerminalMachineResult::Unit;
    machine.blocks[0].operations = vec![Operation {
        id: OperationId::new(7).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary,
            arguments: vec![value],
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    }];
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(8).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

fn stage_byte_output(
    target: NativeTarget,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    let module = byte_output_module();
    stage_byte_output_module(target, &module)
}

fn stage_byte_output_module(
    target: NativeTarget,
    module: &TerminalModule,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    let proof = ProofBundle::default();
    terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default()).unwrap();
    calls::stage_call_text_with_settlements(
        target,
        module,
        &proof,
        &[AdmittedBoundarySettlement {
            boundary: module.boundary_machines[0].id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(
                CompilerBuiltinExecution::LinuxWriteByteI32,
            ),
            realization: LinuxWriteByteI32Realization.into(),
        }],
    )
}

#[test]
fn returning_byte_output_cross_lowers_on_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let placed = stage_byte_output(target);
        assert!(!placed.text_section().bytes.is_empty());
        assert_eq!(placed.text_section().functions.len(), 1);
    }
}

#[test]
fn returning_byte_output_publishes_objects_and_images_with_exact_source_custody() {
    use machine_code::{BoundaryExecutionRecord, InternalUnitScalarArgumentSourceRecord};
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let text = stage_byte_output(target);
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        assert_eq!(object.text_bytes(), source.source().text_section().bytes);
        assert_eq!(object.boundary_settlements().len(), 1);
        let settlement = &object.boundary_settlements()[0].settlement;
        assert_eq!(settlement.psi_operation, OperationId::new(7).unwrap());
        assert_eq!(
            settlement.execution,
            BoundaryExecutionRecord::CompilerBuiltin(CompilerBuiltinExecution::LinuxWriteByteI32)
        );
        assert!(matches!(settlement.runtime_scalar_arguments[0].source,
            InternalUnitScalarArgumentSourceRecord::SelectedBoundary { source_value, .. }
                if source_value == ValueId::new(5).unwrap()));
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
        for mutation in 0..6 {
            let mut changed = object.clone();
            if mutation == 5 {
                changed.clear_fragment_replay_for_test();
            } else {
                let mut settlement = changed.boundary_settlements()[0].settlement.clone();
                match mutation {
                    0 | 1 => {
                        let InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                            source_value,
                            scratch_byte_offset,
                            ..
                        } = &mut settlement.runtime_scalar_arguments[0].source
                        else {
                            panic!("selected boundary input")
                        };
                        if mutation == 0 {
                            *source_value = ValueId::new(99).unwrap();
                        } else {
                            *scratch_byte_offset += 1;
                        }
                    }
                    2 => {
                        settlement.code_offset += 1;
                        settlement.runtime_scalar_arguments[0].code_offset += 1;
                    }
                    3 => {
                        settlement.byte_count -= 1;
                        settlement.runtime_scalar_arguments[0].byte_count -= 1;
                    }
                    4 => {
                        settlement.execution = BoundaryExecutionRecord::CompilerBuiltin(
                            CompilerBuiltinExecution::LinuxExitGroupI32,
                        )
                    }
                    _ => unreachable!(),
                }
                changed.boundary_settlements_mut_for_test()[0].settlement = settlement;
            }
            if mutation == 5 {
                // An explicit source can still validate these bytes. Image
                // publication has no such argument and requires retained replay.
                image_emission::validate_function_fragment_object_artifact(&source, &changed)
                    .unwrap();
            } else {
                assert!(
                    image_emission::validate_function_fragment_object_artifact(&source, &changed)
                        .is_err(),
                    "publication mutation {mutation} on {target:?}"
                );
            }
            assert!(
                image_emission::emit_executable_image(&changed, 3).is_err(),
                "image mutation {mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn returning_byte_output_executes_exact_raw_bytes() {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        let placed = stage_byte_output(NativeTarget::host());
        let text = placed.text_section();
        native_function::assert_c_text(
            &text.bytes,
            text.functions[0].section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            #include <unistd.h>
            extern void omega_entry(int32_t byte);
            int main(void) {
                alarm(10);
                int channel[2];
                if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                const uint8_t expected[] = { 0, 0x80, 0xff, 0, 0x80, 0xff };
                for (unsigned position = 0; position < sizeof(expected); ++position)
                    omega_entry(expected[position]);
                if (dup2(saved, STDOUT_FILENO) < 0) return 3;
                close(saved);
                for (unsigned position = 0; position < sizeof(expected); ++position) {
                    uint8_t actual;
                    if (read(channel[0], &actual, 1) != 1 || actual != expected[position]) return 4;
                }
                uint8_t extra;
                if (read(channel[0], &extra, 1) != 0) return 5;
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
        "SKIP: Linux byte-output builtin runtime requires a Linux x86-64 or AArch64 host; macOS/Windows providers are not substituted"
    );
}
