use std::sync::Arc;

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use target_operations::{TargetControlTerminator, TargetOperation};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::OperationKind;

fn publish(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
    expected_calls: usize,
) -> (image_emission::ExecutableImage, usize) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let authored_calls = module
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
        .collect::<Vec<_>>();
    assert_eq!(authored_calls.len(), expected_calls);
    assert_eq!(
        module.machines.len(),
        expected_calls + 1,
        "only the authored closure"
    );
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap_or_else(|error| panic!("admit scalar cycle for {target:?}: {error:#?}"));
    let post_terminal = optimized.selections().project_post_terminal();
    let target_operations =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized, target,
        )
        .unwrap_or_else(|error| panic!("lower scalar cycle for {target:?}: {error:#?}"));
    let entry = target_operations
        .target_operations()
        .functions
        .iter()
        .find(|function| function.machine == module.entry)
        .unwrap();
    let TargetOperation::ControlGraph(graph) = &entry.operation else {
        panic!("ordinary scalar cycle requires TargetControlGraph on {target:?}");
    };
    assert!(
        graph.blocks.iter().any(|block| matches!(
            block.terminator,
            TargetControlTerminator::ReturnScalar { .. }
        )),
        "scalar exit remains explicit on {target:?}"
    );
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target_operations,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("select and allocate scalar cycle for {target:?}: {error:#?}"));
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&text)
        .expect("independently replay complete scalar-cycle text");
    let resolved_calls = text
        .text_section()
        .resolved_internal_machine_calls
        .iter()
        .map(|call| (call.caller, call.operation, call.callee))
        .collect::<Vec<_>>();
    assert_eq!(
        resolved_calls, authored_calls,
        "exact selected scalar call survives native placement on {target:?}"
    );
    assert_eq!(text.text_section().functions.len(), module.machines.len());
    let source =
        Arc::new(object_file::stage_optimized_relocation_free_object_container(text).unwrap());
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    assert_eq!(object.text_bytes(), source.source().text_section().bytes);
    let entry_offset = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let decoded = image_emission::decode_installation_record(
        &image_emission::encode_installation_record(&record).unwrap(),
    )
    .unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    assert_eq!(
        image_emission::derive_stack_demand(&object, module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(&decoded, &image, module.entry).unwrap(),
    );
    (image, entry_offset)
}

pub(super) fn assert_four_targets(artifact: &CanonicalTerminalArtifact, expected_calls: usize) {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, _) = publish(artifact, target, expected_calls);
        assert!(!image.output().final_text_bytes.is_empty(), "{target:?}");
    }
}

pub(super) fn assert_host_execution(
    artifact: &CanonicalTerminalArtifact,
    expected_calls: usize,
    driver: &str,
) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, entry_offset) = publish(artifact, NativeTarget::host(), expected_calls);
        super::native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            driver,
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (artifact, expected_calls, driver);
        eprintln!(
            "SKIP: scalar cycle C runtime requires Linux x86-64/AArch64 or macOS AArch64; Windows has publication coverage only"
        );
    }
}
