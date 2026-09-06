//! Hosted publication matrix for exact selected-lowering rules without a layout rule.

use crate::FunctionFragmentReplayInputs;
use crate::tests::*;

#[derive(Debug, Clone, Copy)]
struct HostedTargetCase {
    target: NativeTarget,
    object_format: target::ObjectFormat,
    text_section_name: &'static str,
    calling_policy: calling_conventions::CallingPolicy,
    exit_policy: WholeFunctionExitPolicy,
}

#[derive(Debug, PartialEq, Eq)]
struct PublishedSelectedLoweringSnapshot {
    encoding: [u8; 32],
    realization_manifest: Vec<u8>,
    fragment_manifest: Vec<u8>,
    text_manifest: Vec<u8>,
    text_bytes: Vec<u8>,
    object_manifest: Vec<u8>,
    object_container_bytes: Vec<u8>,
    artifact_record: Vec<u8>,
    artifact_manifest: Vec<u8>,
    callable_record: Vec<u8>,
    callable_manifest: Vec<u8>,
}

fn hosted_target_cases() -> [HostedTargetCase; 4] {
    use calling_conventions::CallingPolicy;
    use target::ObjectFormat;

    [
        HostedTargetCase {
            target: NativeTarget::linux_x64(),
            object_format: ObjectFormat::Elf,
            text_section_name: ".text",
            calling_policy: CallingPolicy::SystemVAMD64,
            exit_policy: WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
        },
        HostedTargetCase {
            target: NativeTarget::windows_x64(),
            object_format: ObjectFormat::Coff,
            text_section_name: ".text",
            calling_policy: CallingPolicy::MicrosoftX64,
            exit_policy: WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1,
        },
        HostedTargetCase {
            target: NativeTarget::linux_arm64(),
            object_format: ObjectFormat::Elf,
            text_section_name: ".text",
            calling_policy: CallingPolicy::Aapcs64,
            exit_policy: WholeFunctionExitPolicy::Aapcs64FramelessLeafV1,
        },
        HostedTargetCase {
            target: NativeTarget::macos_arm64(),
            object_format: ObjectFormat::MachO,
            text_section_name: "__TEXT,__text",
            calling_policy: CallingPolicy::Aapcs64,
            exit_policy: WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1,
        },
    ]
}

fn publish_exact_selected_lowering(
    subtract: bool,
    optimization: Optimization,
    case: HostedTargetCase,
) -> PublishedSelectedLoweringSnapshot {
    let (semantic, proof) = conditional_exact_binary_artifact(subtract);
    let selections = OptimizationSelections::new([optimization]).unwrap();
    let selection_identity = selections.identity();
    let selected_lowering_identity = selections
        .for_phase(optimization_core::OptimizationExecutionPhase::SelectedLowering)
        .identity();
    let selected = if subtract {
        staged_exact_subtract_conditional_with_selections(
            case.target,
            selections,
            selected_lowering_budget(),
        )
    } else {
        staged_exact_add_conditional_with_selections(
            case.target,
            selections,
            selected_lowering_budget(),
        )
    };
    let ranges = stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let sole_view_name = match case.target.architecture {
        target::Architecture::X86_64 => "rax",
        target::Architecture::Aarch64 => "x0",
    };
    let sole_view = environment
        .physical()
        .model()
        .view_named(sole_view_name)
        .unwrap()
        .id;
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
            views: vec![sole_view],
        },
    )
    .unwrap();
    let legality =
        stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap();
    let run = run_selected_lowering_optimizations(legality).unwrap();
    assert_eq!(run.custody().action_count(), 2);
    assert_eq!(run.steps().len(), 2);
    let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
    let realization = crate::tests::with_allocated_machine(
        homes.try_into().unwrap(),
        stage_selected_lowering_function_relative_realization,
    )
    .unwrap();

    assert_eq!(
        validate_selected_lowering_function_relative_realization_custody(&realization).unwrap(),
        *realization.custody()
    );
    assert!(realization.relaxation().is_none());
    let transformations = &realization
        .allocation()
        .current()
        .post_allocation_manifest()
        .record()
        .selected_transformations;
    assert_eq!(transformations.len(), 2);
    assert!(transformations.iter().all(|transformation| matches!(
        transformation,
        PostAllocationSelectedTransformation::LiteralFold(_)
    )));
    let realization_record = realization.manifest().record();
    assert_eq!(realization_record.target, case.target);
    assert_eq!(realization_record.selections, selection_identity);
    assert_eq!(
        realization_record.selected_lowering_selections,
        selected_lowering_identity
    );
    assert!(realization_record.selected_lowering_completion.is_some());
    assert_eq!(
        realization_record.pre_layout,
        realization.encoding().identity()
    );
    assert_eq!(
        realization.exit_contract().contract().policy,
        case.exit_policy
    );
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&realization_record.encode()),
        Ok(realization_record.clone())
    );
    let encoding = realization.encoding().identity().bytes();
    let realization_manifest = realization_record.encode();

    let fragments = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::SelectedLowering(Box::new(realization)).into(),
    )
    .unwrap();
    let fragment_record = fragments.manifest().record();
    assert_eq!(
        fragment_record.source_kind,
        FunctionFragmentEmissionSourceKind::SelectedLoweringV1
    );
    assert_eq!(fragment_record.target, case.target);
    assert_eq!(fragment_record.selections, selection_identity);
    assert_eq!(fragment_record.final_pre_layout.bytes(), encoding);
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&fragment_record.encode()),
        Ok(fragment_record.clone())
    );
    assert_eq!(
        validate_optimized_function_fragment_emission(&fragments).unwrap(),
        fragments.custody()
    );
    let fragment_manifest = fragment_record.encode();

    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let text_record = text.manifest().record();
    assert_eq!(
        text_record.source_kind,
        FunctionFragmentEmissionSourceKind::SelectedLoweringV1
    );
    assert_eq!(text_record.target, case.target);
    assert_eq!(text_record.selections, selection_identity);
    assert_eq!(text.text_section().target, case.target);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&text_record.encode()),
        Ok(text_record.clone())
    );
    assert_eq!(
        validate_optimized_relocation_free_text_section(&text).unwrap(),
        text.custody()
    );
    let text_manifest = text_record.encode();
    let text_bytes = text.text_section().bytes.clone();
    assert!(!text_bytes.is_empty());

    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let object_record = object.manifest().record();
    assert_eq!(object_record.target, case.target);
    assert_eq!(object_record.selections, selection_identity);
    assert_eq!(object.object().target, case.target);
    assert_eq!(object.object().target.object_format, case.object_format);
    assert_eq!(object.object().selections, selection_identity);
    assert_eq!(object.object().text_section.name, case.text_section_name);
    assert_eq!(object.object().text_section.bytes, text_bytes);
    assert!(!object.container().bytes.is_empty());
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&object_record.encode()),
        Ok(object_record.clone())
    );
    assert_eq!(
        validate_optimized_relocation_free_object_container(&object).unwrap(),
        object.custody()
    );
    let object_manifest = object_record.encode();
    let object_container_bytes = object.container().bytes.clone();

    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    assert_eq!(artifact.artifact().target, case.target);
    assert_eq!(artifact.artifact().selections, selection_identity);
    assert_eq!(artifact.manifest().record().target, case.target);
    assert_eq!(artifact.manifest().record().selections, selection_identity);
    assert_eq!(
        validate_optimized_object_artifact(&artifact).unwrap(),
        artifact.custody()
    );
    let artifact_record = artifact.artifact().encode();
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&artifact_record),
        Ok(artifact.artifact().clone())
    );
    let artifact_manifest = artifact.manifest().record().encode();
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&artifact_manifest),
        Ok(artifact.manifest().record().clone())
    );

    let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
    assert_eq!(callable.entry().target, case.target);
    assert_eq!(callable.entry().selections, selection_identity);
    assert_eq!(callable.entry().calling_policy, case.calling_policy);
    assert_eq!(callable.entry().exit_policy, case.exit_policy);
    assert_eq!(callable.manifest().record().target, case.target);
    assert_eq!(callable.manifest().record().selections, selection_identity);
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&callable).unwrap(),
        callable.custody()
    );
    let callable_record = callable.entry().encode().unwrap();
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&callable_record),
        Ok(callable.entry().clone())
    );
    let callable_manifest = callable.manifest().record().encode();
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&callable_manifest),
        Ok(callable.manifest().record().clone())
    );

    PublishedSelectedLoweringSnapshot {
        encoding,
        realization_manifest,
        fragment_manifest,
        text_manifest,
        text_bytes,
        object_manifest,
        object_container_bytes,
        artifact_record,
        artifact_manifest,
        callable_record,
        callable_manifest,
    }
}

#[test]
fn applied_exact_selected_lowering_publishes_deterministically_on_hosted_targets() {
    for (subtract, optimization) in [
        (false, Optimization::SelectedIncomingU12ExactAddImmediate),
        (
            true,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ),
    ] {
        for case in hosted_target_cases() {
            let first = publish_exact_selected_lowering(subtract, optimization, case);
            let repeated = publish_exact_selected_lowering(subtract, optimization, case);
            assert_eq!(first, repeated, "{optimization:?} on {:?}", case.target);
        }
    }
}
