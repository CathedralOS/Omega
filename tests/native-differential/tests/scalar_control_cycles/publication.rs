use std::sync::Arc;

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use target_operations::TargetControlTerminator;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::OperationKind;

fn publish(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
    expected_calls: usize,
) -> (image_emission::ExecutableImage, usize) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let mut authored_calls = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block
                    .operations
                    .iter()
                    .filter_map(move |operation| match operation.kind {
                        OperationKind::Call { callee, .. }
                        | OperationKind::CallUnit { callee, .. } => {
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
    let graph = &entry.graph;
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
    let mut resolved_calls = text
        .text_section()
        .resolved_internal_machine_calls
        .iter()
        .map(|call| (call.caller, call.operation, call.callee))
        .collect::<Vec<_>>();
    // Physical block layout need not follow the serialized source block roster.
    // Compare complete occurrence multisets; graph replay checks control order.
    authored_calls.sort_unstable();
    resolved_calls.sort_unstable();
    assert_eq!(
        resolved_calls, authored_calls,
        "exact selected call roster survives native placement on {target:?}"
    );
    assert_eq!(text.text_section().functions.len(), module.machines.len());
    let source =
        Arc::new(object_file::stage_optimized_relocation_free_object_container(text).unwrap());
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    assert_eq!(object.text_bytes(), source.source().text_section().bytes);
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    if !entry.structural_parameters.is_empty() {
        let function = object.entry_function();
        assert_eq!(
            function.scalar_structural_parameters.len(),
            entry.structural_parameters.len()
        );
        for (record, parameter) in function
            .scalar_structural_parameters
            .iter()
            .zip(&entry.structural_parameters)
        {
            assert_eq!(record.place, parameter.place);
            assert_eq!(record.access, parameter.access);
            assert_eq!(record.multiplicity, parameter.multiplicity);
            assert_eq!(record.structural_type, parameter.structural_type);
        }
        assert!(
            function.scalar_structural_parameter_homes.is_empty(),
            "unobserved payload has no home"
        );
        assert!(
            function.mixed_structural_scalar_abi.is_some(),
            "owned value ABI is retained"
        );
        let mut stripped = object.clone();
        stripped.clear_fragment_replay_for_test();
        assert!(
            image_emission::emit_executable_image(&stripped, 3).is_err(),
            "stripped owned replay"
        );
        let mut erased = object.clone();
        let function = erased
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == module.entry)
            .unwrap();
        function.scalar_structural_parameters.clear();
        function.mixed_structural_scalar_abi = None;
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &erased).is_err(),
            "erased owned ABI and roster"
        );
        let mut forged = object.clone();
        let function = forged
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == module.entry)
            .unwrap();
        let parameter = function.scalar_structural_parameters[0];
        let placement = function
            .mixed_structural_scalar_abi
            .as_ref()
            .unwrap()
            .structural_parameters[0]
            .placement
            .clone();
        function
            .scalar_structural_parameter_homes
            .push(machine_code::UnitParameterHomeRecord {
                place: parameter.place,
                structural_type: parameter.structural_type,
                access: parameter.access,
                multiplicity: parameter.multiplicity,
                shape: parameter.shape,
                source: placement,
                location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
                indirect: false,
            });
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &forged).is_err(),
            "invented payload home cannot replace unused transport"
        );
    }
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
    if decoded.internal_unit_calls().len() >= 2 {
        let mut reordered = decoded.clone();
        reordered.internal_unit_calls_mut_for_test().swap(0, 1);
        assert!(
            image_emission::encode_installation_record(&reordered).is_err(),
            "installation call rows must retain physical image order"
        );
        assert!(image_emission::validate_installation_record(&reordered, &image).is_err());

        let mut stale_ordinal = decoded.clone();
        let first = &mut stale_ordinal.internal_unit_calls_mut_for_test()[0];
        first.custody.operation_ordinal = usize::MAX;
        assert!(
            image_emission::encode_installation_record(&stale_ordinal).is_err(),
            "physical ordering cannot replace the exact semantic operation ordinal"
        );
        assert!(image_emission::validate_installation_record(&stale_ordinal, &image).is_err());
    }
    assert_eq!(
        image_emission::derive_stack_demand(&object, module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(&decoded, &image, module.entry).unwrap(),
    );
    let canonical = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let selected = effects::SelectedProviderPlanFacts::default();
    let selected_digest = selected.identity_digest();
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
    .expect("exact empty authority closure for ordinary source cycles");
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
            // Ordinary physical replay is retained; no provider/operator
            // occurrence evidence is claimed by this fixture.
            physical_evidence_scope: native_artifact::NativePhysicalEvidenceScope::Unavailable,
        },
    )
    .unwrap_or_else(|error| panic!("native cycle artifact on {target:?}: {error}"));
    native
        .validate()
        .expect("independent native artifact replay");
    (native.into_parts().image, entry_offset)
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
