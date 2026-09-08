use std::sync::Arc;

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use terminal_codec::CanonicalTerminalArtifact;

fn publish(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
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
                        terminal_psi::OperationKind::Call { callee, .. }
                        | terminal_psi::OperationKind::CallStructuralScalar { callee, .. }
                        | terminal_psi::OperationKind::CallUnit { callee, .. } => {
                            Some((machine.id, operation.id, callee))
                        }
                        _ => None,
                    })
            })
        })
        .collect::<Vec<_>>();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap_or_else(|error| panic!("admit primitive locals for {target:?}: {error:#?}"));
    let post_terminal = optimized.selections().project_post_terminal();
    let target_operations =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized, target,
        )
        .unwrap_or_else(|error| panic!("lower primitive locals for {target:?}: {error:#?}"));
    let entry = target_operations
        .target_operations()
        .functions
        .iter()
        .find(|function| function.machine == module.entry)
        .unwrap();
    let target_operations::TargetOperation::ControlGraph(graph) = &entry.operation else {
        panic!("primitive locals require the ordinary control graph");
    };
    assert!(
        graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation,
                target_operations::TargetUnitOperation::EstablishPrimitiveLocal { .. }
            ))
    );
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target_operations,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("select primitive locals for {target:?}: {error:#?}"));
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&text).unwrap();
    assert_eq!(text.text_section().functions.len(), module.machines.len());
    let resolved_calls = text
        .text_section()
        .resolved_internal_machine_calls
        .iter()
        .map(|call| (call.caller, call.operation, call.callee))
        .collect::<Vec<_>>();
    assert_eq!(
        resolved_calls, authored_calls,
        "every authored call survives exactly once"
    );
    let source =
        Arc::new(object_file::stage_optimized_relocation_free_object_container(text).unwrap());
    let object = image_emission::build_function_fragment_object_artifact(source.clone())
        .unwrap_or_else(|error| panic!("publish primitive locals for {target:?}: {error:#?}"));
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let mut stripped = object.clone();
    stripped.clear_fragment_replay_for_test();
    assert!(
        image_emission::emit_executable_image(&stripped, 3).is_err(),
        "local storage needs retained physical replay"
    );
    let entry_offset = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let encoded = image_emission::encode_installation_record(&record).unwrap();
    let decoded = image_emission::decode_installation_record(&encoded).unwrap();
    assert_eq!(
        image_emission::encode_installation_record(&decoded).unwrap(),
        encoded
    );
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    assert_eq!(
        image_emission::derive_stack_demand(&object, module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(&decoded, &image, module.entry).unwrap(),
    );
    let canonical = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let selected_digest = effects::SelectedProviderPlanFacts::default().identity_digest();
    let physical_policy =
        native_realization::current_compiler_intrinsic_terminal_authority_policy();
    let permission_policy = native_realization::current_terminal_authority_permission_policy();
    let closure_review = effects::TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        *canonical.manifest().identity().as_bytes(),
        target,
        selected_digest,
        physical_policy.identity(),
        permission_policy.identity(),
        Vec::new(),
    )
    .unwrap();
    let native = native_artifact::NativeArtifact::from_emitted_parts(
        native_artifact::NativeArtifactEmissionParts {
            target,
            psi_artifact: canonical,
            object,
            image,
            selected_provider_closure_report_identity: 1,
            selected_provider_closure_digest:
                native_artifact::NativeSelectedProviderClosureDigest::from_digest(
                    *selected_digest.as_bytes(),
                ),
            selected_provider_plans: Vec::new(),
            provider_executions: Vec::new(),
            terminal_authority_policy_identity: physical_policy.identity(),
            terminal_authority_permission_policy_identity: permission_policy.identity(),
            terminal_authority_closure_review: closure_review,
            boundary_application_coverage: None,
            physical_evidence_scope: native_artifact::NativePhysicalEvidenceScope::Unavailable,
        },
    )
    .unwrap_or_else(|error| panic!("native primitive-local join for {target:?}: {error}"));
    native.validate().unwrap();
    (native.into_parts().image, entry_offset)
}

pub(super) fn assert_four_targets(artifact: &CanonicalTerminalArtifact) {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, _) = publish(artifact, target);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

pub(super) fn assert_host_execution(artifact: &CanonicalTerminalArtifact, driver: &str) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, entry_offset) = publish(artifact, NativeTarget::host());
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
        let _ = (artifact, driver);
        eprintln!(
            "SKIP: primitive-local C runtime requires Linux x86-64/AArch64 or macOS AArch64; Windows has publication coverage only"
        );
    }
}
