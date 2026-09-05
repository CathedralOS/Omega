//! AArch64 CBNZ realization through direct and selected-lowering sources.

use crate::SelectedFormMachineDisposition;
use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, FunctionRelativeOptimizationRealizationError,
    FunctionRelativeOptimizationRealizationManifest, NativeTarget, Optimization,
    OptimizationSelections, OptimizedResolvedSelectedFormLayoutError,
    OptimizedSelectedFormEncodingError, StagedOptimizedPostAllocationMachineOptimization,
    WholeFunctionExitContractError, WholeFunctionExitLayoutCustody,
    conditional_exact_binary_artifact, optimization_pipeline_report, optimize_artifact_sections,
    selected_lowering_budget, stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
    stage_optimized_resolved_selected_form_layout,
    stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    validate_optimized_aarch64_cbnz_fusion_custody,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_post_allocation_machine_function_relative_realization_custody,
    validate_whole_function_exit_contract,
};

#[test]
fn compiler_facing_physical_pipeline_routes_aarch64_cbnz_through_the_generic_post_allocation_join()
{
    let target = NativeTarget::linux_arm64();
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let mut staged =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let realization = (staged)
        .post_allocation_machine_for_test()
        .unwrap_or_else(|| {
            panic!("the exact post-allocation phase must use its symbolic machine route")
        });
    let allocation = realization.allocation().current();
    assert!(matches!(
        allocation.evidence(),
        omega_selected_instructions_to_register_homes::AllocationEvidence::RegisterHomes(_)
    ));
    let homes = &allocation;
    let machine = realization.machine();
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(optimization) =
        realization.optimization()
    else {
        panic!("the generic realization must retain the CBNZ result")
    };
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(staged.selected_lowering_completion(), None);
    assert_eq!(staged.function_relative_manifest(), realization.manifest());
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization)
            .unwrap(),
        *realization.custody()
    );
    let manifest = realization.manifest().record();
    assert_eq!(
        manifest.post_allocation_machine_selections,
        selections.identity()
    );
    assert_eq!(
        manifest.function_relative_layout_selections,
        OptimizationSelections::default().identity()
    );
    assert_eq!(
        manifest.baseline_pre_layout,
        realization.baseline_encoding().identity()
    );
    assert_eq!(manifest.pre_layout, realization.encoding().identity());
    assert_eq!(
        manifest.baseline_resolved_layout,
        realization.baseline_layout().identity()
    );
    assert_eq!(manifest.resolved_layout, realization.layout().identity());
    assert_eq!(
        manifest.post_allocation_machine_optimization,
        realization.optimization().custody()
    );
    assert_eq!(manifest.x86_branch_relaxation, None);
    assert!(matches!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            artifact_identity,
        } if artifact_identity == optimization.fusion().receipt().identity().bytes()
    ));
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&manifest.encode()),
        Ok(manifest.clone())
    );
    assert!(
        optimization_pipeline_report(&staged)
            .function_relative()
            .is_some()
    );
    assert_eq!(optimization.fusion().receipt().action_count(), 1);
    assert_eq!(
        validate_optimized_aarch64_cbnz_fusion_custody(homes, machine, optimization).unwrap(),
        optimization.custody()
    );
    assert_eq!(
        optimization.custody().post_allocation_machine_selections(),
        selections.identity()
    );

    let selected_stage = homes;
    let physical = selected_stage.register_environment().physical();
    let baseline_encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(),
        machine,
        physical,
    )
    .unwrap();
    let baseline_layout = stage_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        machine,
        physical,
        &baseline_encoding,
    )
    .unwrap();
    let fused_encoding =
        stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            optimization,
        )
        .unwrap();
    let fused_layout = stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected_stage.selected(),
        machine,
        physical,
        &fused_encoding,
        optimization,
    )
    .unwrap();
    let action = &optimization.fusion().plan().actions[0];
    assert_eq!(
        fused_encoding.machine_optimization().unwrap().fusion(),
        optimization.fusion().receipt().identity()
    );
    assert_eq!(
        fused_layout.machine_optimization(),
        fused_encoding.machine_optimization()
    );
    assert_ne!(baseline_encoding.identity(), fused_encoding.identity());
    assert_ne!(baseline_layout.identity(), fused_layout.identity());
    assert_eq!(
        baseline_layout.functions()[0].byte_count,
        fused_layout.functions()[0].byte_count + 4
    );
    let fused_rows = fused_layout.functions()[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|row| (row.instruction, row))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert!(fused_rows[&action.compare].bytes.is_empty());
    assert!(fused_rows[&action.compare].branch.is_none());
    let branch = fused_rows[&action.branch];
    assert_eq!(branch.bytes.len(), 4);
    let source_register = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == action.source_read.view)
        .unwrap()
        .name
        .strip_prefix('x')
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert_eq!(
        u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap()) & 0xff00_001f,
        0xb500_0000 | source_register
    );
    assert_eq!(
        branch.branch.as_ref().unwrap().decoded_register_reads,
        [action.source_read.view]
    );
    assert!(
        branch
            .branch
            .as_ref()
            .unwrap()
            .decoded_effects
            .implicit_unit_uses
            .iter()
            .all(|unit| !action.nzcv_units.contains(unit))
    );
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
        selected_stage.selected(),
        machine,
        physical,
        optimization,
        &fused_encoding,
    )
    .unwrap();
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected_stage.selected(),
        machine,
        physical,
        &fused_encoding,
        optimization,
        &fused_layout,
    )
    .unwrap();
    assert!(matches!(
        validate_whole_function_exit_contract(
            selected_stage.selected(),
            machine,
            physical,
            &fused_encoding,
            &fused_layout,
            realization.exit_contract(),
        ),
        Err(WholeFunctionExitContractError::Layout(
            OptimizedResolvedSelectedFormLayoutError::PreLayout(
                OptimizedSelectedFormEncodingError::ArtifactMismatch
            )
        ))
    ));

    let mut corrupt_encoding = fused_encoding.clone();
    let branch_disposition = &mut corrupt_encoding
        .rows_mut()
        .iter_mut()
        .find(|row| row.instruction == action.branch)
        .unwrap()
        .machine_disposition;
    let SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 { source_read, .. } =
        branch_disposition
    else {
        panic!("expected fused branch disposition")
    };
    source_read.view = physical.model().view_named("x2").unwrap().id;
    assert_eq!(
        validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            optimization,
            &corrupt_encoding,
        ),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    );

    let mut corrupt_layout = fused_layout.clone();
    corrupt_layout.functions_mut()[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.instruction == action.branch)
        .unwrap()
        .bytes[0] ^= 0x20;
    assert_eq!(
        validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            &fused_encoding,
            optimization,
            &corrupt_layout,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    );
    let mut corrupt_layout = fused_layout.clone();
    corrupt_layout.functions_mut()[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.instruction == action.compare)
        .unwrap()
        .bytes
        .push(0);
    assert_eq!(
        validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            &fused_encoding,
            optimization,
            &corrupt_layout,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    );
    let mut corrupt_layout = fused_layout.clone();
    corrupt_layout.functions_mut()[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.instruction == action.branch)
        .unwrap()
        .branch
        .as_mut()
        .unwrap()
        .decoded_register_reads
        .clear();
    assert_eq!(
        validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            &fused_encoding,
            optimization,
            &corrupt_layout,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    );

    let mut rehashed_corruption = optimization.fusion().plan().clone();
    rehashed_corruption.actions[0].source_read.view = physical.model().view_named("x2").unwrap().id;
    rehashed_corruption.identity =
        omega_post_allocation_machine_to_optimized_machine::aarch64_cbnz_fusion_identity(
            &rehashed_corruption,
        );
    assert_eq!(
        omega_post_allocation_machine_to_optimized_machine::validate_aarch64_cbnz_fusion(
            selected_stage.selected(),
            homes.liveness(),
            machine.machine(),
            physical,
            rehashed_corruption,
        ),
        Err(omega_post_allocation_machine_to_optimized_machine::Aarch64CbnzFusionError::ArtifactMismatch)
    );

    let realization = (staged)
        .post_allocation_machine_mut_for_test()
        .unwrap_or_else(|| unreachable!());
    let original_layout = realization.manifest().record().resolved_layout;
    realization.manifest_mut().record_mut().resolved_layout =
        realization.baseline_layout().identity();
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );
    realization.manifest_mut().record_mut().resolved_layout = original_layout;
    let original_custody = realization.exit_contract().contract().layout_custody;
    realization
        .exit_contract_mut()
        .contract_mut()
        .layout_custody = WholeFunctionExitLayoutCustody::BaselineNearLayoutV1;
    assert!(matches!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::ExitContract(
            WholeFunctionExitContractError::ArtifactMismatch
        ))
    ));
    realization
        .exit_contract_mut()
        .contract_mut()
        .layout_custody = original_custody;
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization)
            .unwrap(),
        *realization.custody()
    );
}

#[test]
fn aarch64_cbnz_fusion_composes_after_exact_selected_lowering() {
    let target = NativeTarget::linux_arm64();
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let staged =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let realization = (staged)
        .post_allocation_machine_for_test()
        .unwrap_or_else(|| {
            panic!("selected lowering must retain custody before post-allocation fusion")
        });
    let allocation = realization.allocation().current();
    assert!(matches!(
        allocation.evidence(),
        omega_selected_instructions_to_register_homes::AllocationEvidence::SelectedLowering(_)
    ));
    let homes = &allocation;
    let machine = realization.machine();
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(optimization) =
        realization.optimization()
    else {
        panic!("the generic realization must retain the CBNZ result")
    };
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(
        staged.selected_lowering_completion(),
        homes
            .post_allocation_manifest()
            .record()
            .selected_lowering_completion
    );
    assert_eq!(optimization.fusion().receipt().action_count(), 1);
    assert_eq!(staged.function_relative_manifest(), realization.manifest());
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization)
            .unwrap(),
        *realization.custody()
    );
    assert_eq!(
        realization.manifest().record().selected_lowering_completion,
        staged.selected_lowering_completion()
    );
    assert_eq!(
        realization
            .manifest()
            .record()
            .post_allocation_machine_optimization,
        realization.optimization().custody()
    );
    assert_eq!(
        validate_optimized_aarch64_cbnz_fusion_custody(homes, machine, optimization,).unwrap(),
        optimization.custody()
    );
}
