use std::sync::Arc;

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use target_operations::{TargetControlTerminator, TargetOperation, TargetUnitOperation};
use terminal_codec::CanonicalTerminalArtifact;

fn targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ]
}

fn publish(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
) -> (image_emission::ExecutableImage, usize) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap_or_else(|error| panic!("admit primitive store-return for {target:?}: {error:#?}"));
    let post_terminal = optimized.selections().project_post_terminal();
    let target_operations =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized, target,
        )
        .unwrap_or_else(|error| panic!("lower primitive store-return for {target:?}: {error:#?}"));
    let entry = target_operations
        .target_operations()
        .functions
        .iter()
        .find(|function| function.machine == module.entry)
        .unwrap();
    let TargetOperation::ControlGraph(graph) = &entry.operation else {
        panic!("effectful scalar return must use the ordinary target graph on {target:?}");
    };
    assert_eq!(
        graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation,
                TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
            ))
            .count(),
        1
    );
    assert!(graph.blocks.iter().any(|block| matches!(
        block.terminator,
        TargetControlTerminator::ReturnScalar { .. }
    )));
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target_operations,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| {
        panic!("select and allocate primitive store-return for {target:?}: {error:#?}")
    });
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&text)
        .expect("independent physical replay");
    assert_eq!(text.text_section().functions.len(), 1);
    assert!(
        text.text_section()
            .resolved_internal_machine_calls
            .is_empty()
    );
    let source =
        Arc::new(object_file::stage_optimized_relocation_free_object_container(text).unwrap());
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    assert_eq!(object.text_bytes(), source.source().text_section().bytes);
    let function = object.entry_function();
    assert!(function.scalar_abi.is_none());
    assert!(function.unit_scalar_abi.is_none());
    let mixed = function
        .mixed_structural_scalar_abi
        .as_ref()
        .expect("mixed scalar/reference ABI");
    assert_eq!(
        mixed.call_plan.result.as_ref(),
        Some(&mixed.result.placement)
    );
    assert!(function.unit_parameters.is_empty());
    assert!(function.unit_parameter_homes.is_empty());
    assert_eq!(function.scalar_structural_parameters.len(), 1);
    assert_eq!(function.scalar_structural_parameter_homes.len(), 1);
    let home = &function.scalar_structural_parameter_homes[0];
    assert!(home.indirect);
    assert_eq!(home.access, terminal_psi::StructuralAccess::MutableBorrow);
    assert_eq!(
        home.shape,
        calling_conventions::ValueShape::borrowed_reference(8, 8)
    );
    assert!(matches!(
        home.location,
        machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
    ));
    if mixed.scalar_parameters.len() == 9 {
        assert!(
            matches!(
                home.location,
                machine_code::StructuralSourceLocation::IncomingBorrowedPointer {
                    location: calling_conventions::IndirectPointerLocation::Stack { .. }
                }
            ),
            "stack companion must actually receive its reference on the stack"
        );
        assert!(
            matches!(
                mixed.scalar_parameters[8].placement.locations.as_slice(),
                [calling_conventions::ValueLocation::Stack { .. }]
            ),
            "replacement must be a runtime stack input"
        );
    }
    assert!(function.unit_write_only_primitive_stores.is_empty());
    assert!(function.unit_structural_scalar_field_stores.is_empty());
    assert!(function.scalar_structural_scalar_field_stores.is_empty());
    let mut stripped = object.clone();
    stripped.clear_fragment_replay_for_test();
    assert!(
        image_emission::emit_executable_image(&stripped, 3).is_err(),
        "mixed incoming borrow requires fragment replay on {target:?}"
    );
    let mut erased = object.clone();
    let erased_function = &mut erased.functions_mut_for_test()[0];
    erased_function.scalar_structural_parameters.clear();
    erased_function.scalar_structural_parameter_homes.clear();
    erased_function.mixed_structural_scalar_abi = None;
    assert!(
        image_emission::validate_function_fragment_object_artifact(&source, &erased).is_err(),
        "retained replay rejects coherently erased ABI on {target:?}"
    );
    assert!(
        image_emission::emit_executable_image(&erased, 3).is_err(),
        "erasing ABI cannot bypass exact source on {target:?}"
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
    let [installed] = decoded.functions() else {
        panic!("one installed callee")
    };
    assert_eq!(
        installed.mixed_structural_scalar_abi,
        function.mixed_structural_scalar_abi
    );
    assert_eq!(
        installed.scalar_structural_parameters,
        function.scalar_structural_parameters
    );
    assert_eq!(
        installed.scalar_structural_parameter_homes,
        function.scalar_structural_parameter_homes
    );
    assert!(installed.scalar_abi.is_none());
    assert!(installed.unit_scalar_abi.is_none());
    assert!(installed.unit_parameters.is_empty());
    assert!(installed.unit_parameter_homes.is_empty());
    assert!(installed.unit_write_only_primitive_stores.is_empty());
    assert!(installed.unit_structural_scalar_field_stores.is_empty());
    assert!(installed.scalar_structural_scalar_field_stores.is_empty());
    assert_eq!(
        image_emission::derive_stack_demand(&object, module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(&decoded, &image, module.entry).unwrap()
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
    .expect("exact empty authority closure for the source-produced primitive callee");
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
            // Ordinary fragment replay above retains physical translation evidence;
            // this fixture does not claim provider/operator occurrence evidence.
            physical_evidence_scope: native_artifact::NativePhysicalEvidenceScope::Unavailable,
        },
    )
    .unwrap_or_else(|error| {
        panic!("canonical mixed-ABI native artifact join on {target:?}: {error}")
    });
    native
        .validate()
        .expect("independent native artifact canonical-source replay");
    (native.into_parts().image, entry_offset)
}

pub(super) fn assert_four_targets(artifact: &CanonicalTerminalArtifact) {
    for target in targets() {
        let (image, _) = publish(artifact, target);
        assert!(!image.output().final_text_bytes.is_empty(), "{target:?}");
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
            "SKIP: primitive store-return C runtime requires Linux x86-64/AArch64 or macOS AArch64; Windows has publication coverage only"
        );
    }
}

pub(super) fn assert_corruptions(artifact: &CanonicalTerminalArtifact) {
    for target in targets() {
        let (image, _) = publish(artifact, target);
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        for corruption in [
            "missing referent",
            "access",
            "width",
            "fake stack",
            "return placement",
            "return type",
            "erased ABI",
        ] {
            let mut changed = record.clone();
            let [function] = changed.functions_mut_for_test().as_mut_slice() else {
                panic!("one installed callee")
            };
            assert_eq!(
                function.scalar_structural_parameter_homes.len(),
                1,
                "retain original reference home on {target:?}"
            );
            match corruption {
                "missing referent" => function.scalar_structural_parameter_homes.clear(),
                "access" => {
                    function.scalar_structural_parameter_homes[0].access =
                        terminal_psi::StructuralAccess::SharedBorrow
                }
                "width" => {
                    function.scalar_structural_parameter_homes[0]
                        .source
                        .shape
                        .byte_size = 4
                }
                "fake stack" => {
                    function.scalar_structural_parameter_homes[0].location =
                        machine_code::StructuralSourceLocation::Stack { byte_offset: 0 }
                }
                "return placement" => {
                    let mixed = function.mixed_structural_scalar_abi.as_mut().unwrap();
                    mixed.result.placement.locations.clear();
                    mixed.call_plan.result = Some(mixed.result.placement.clone());
                }
                "return type" => {
                    function
                        .mixed_structural_scalar_abi
                        .as_mut()
                        .unwrap()
                        .result
                        .scalar_type = semantic_vocabulary::ScalarType::Boolean
                }
                "erased ABI" => {
                    function.scalar_structural_parameters.clear();
                    function.scalar_structural_parameter_homes.clear();
                    function.mixed_structural_scalar_abi = None;
                }
                _ => unreachable!(),
            }
            assert!(
                image_emission::validate_installation_record(&changed, &image).is_err(),
                "reject {corruption} on {target:?}"
            );
            if let Ok(encoded) = image_emission::encode_installation_record(&changed)
                && let Ok(decoded) = image_emission::decode_installation_record(&encoded)
            {
                assert!(
                    image_emission::validate_installation_record(&decoded, &image).is_err(),
                    "codec cannot authorize {corruption} on {target:?}"
                );
            }
        }
    }
}
