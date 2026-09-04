//! AArch64 MOVN materialization, realization custody, and corruption rejection.

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, FunctionRelativeOptimizationRealizationError,
    FunctionRelativeOptimizationRealizationManifest, IntegerValue, NativeTarget, Optimization,
    OptimizationSelections, OptimizedResolvedSelectedFormLayoutError,
    OptimizedSelectedFormEncodingError, SelectedFormEncodingRow, SelectedFormEncodingState,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedVerifiedPhysicalPipeline,
    StagedPostAllocationMachineFunctionRelativeSource, WholeFunctionExitLayoutCustody,
    conditional_active_resident_exact_add_chain_artifact_with_false_literal,
    lower_optimized_to_target_operations, optimization_pipeline_report, optimize_artifact_sections,
    selected_lowering_budget, stage_optimized_aarch64_movn_materialization,
    stage_optimized_allocation_legality, stage_optimized_instruction_selection,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization,
    stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan, stage_optimized_register_homes,
    stage_optimized_resolved_selected_form_layout,
    stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    validate_optimized_aarch64_movn_materialization_custody,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
    validate_post_allocation_machine_function_relative_realization_custody,
};

#[test]
fn named_aarch64_movn_materialization_shrinks_pre_layout_bytes_and_replays() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact_with_false_literal(
        IntegerValue::Unsigned(u64::MAX as u128),
    );
    let selections = OptimizationSelections::new([
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_arm64()).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let materialization = stage_optimized_aarch64_movn_materialization(&homes, &machine).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let baseline = stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(),
        &machine,
        physical,
    )
    .unwrap();
    let encoded =
        stage_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization(
            selected_stage.selected(),
            &machine,
            physical,
            &materialization,
        )
        .unwrap();
    let baseline_layout = stage_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        physical,
        &baseline,
    )
    .unwrap();
    let resolved =
        stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization(
            selected_stage.selected(),
            &machine,
            physical,
            &encoded,
            &materialization,
        )
        .unwrap();

    let receipt = materialization.custody();
    assert!(receipt.action_count() > 0);
    assert!(receipt.selected_words() < receipt.baseline_words());
    assert_eq!(receipt.selections(), selections.identity());
    assert_eq!(
        receipt.post_allocation_machine_selections(),
        selections.identity()
    );
    assert_eq!(
        encoded.movn_optimization().unwrap().materialization(),
        receipt.materialization()
    );
    assert_eq!(resolved.movn_optimization(), encoded.movn_optimization());
    assert!(resolved.machine_optimization().is_none());
    assert_ne!(baseline.identity(), encoded.identity());
    assert_ne!(baseline_layout.identity(), resolved.identity());

    let baseline_bytes = baseline_layout
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum::<u64>();
    let resolved_bytes = resolved
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum::<u64>();
    let expected_shrink = receipt
        .baseline_words()
        .checked_sub(receipt.selected_words())
        .unwrap()
        .checked_mul(4)
        .unwrap();
    assert_eq!(
        baseline_bytes.checked_sub(resolved_bytes),
        Some(expected_shrink)
    );

    let action = &materialization.materialization().plan().actions[0];
    let baseline_row = baseline
        .rows()
        .iter()
        .find(|row| row.instruction == action.instruction)
        .unwrap();
    let optimized_row = encoded
        .rows()
        .iter()
        .find(|row| row.instruction == action.instruction)
        .unwrap();
    let encoded_len = |row: &SelectedFormEncodingRow| match &row.state {
        SelectedFormEncodingState::Encoded { bytes, .. } => bytes.len(),
        SelectedFormEncodingState::UnresolvedInternalMachineCall { bytes, .. } => bytes.len(),
        SelectedFormEncodingState::DeferredControl { .. } => 0,
    };
    assert_eq!(
        encoded_len(optimized_row),
        action.recipe.word_count().unwrap() as usize * 4
    );
    assert!(encoded_len(optimized_row) < encoded_len(baseline_row));
    validate_optimized_aarch64_movn_materialization_custody(&homes, &machine, &materialization)
        .unwrap();
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization(
        selected_stage.selected(),
        &machine,
        physical,
        &materialization,
        &encoded,
    )
    .unwrap();
    validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization(
        selected_stage.selected(),
        &machine,
        physical,
        &encoded,
        &materialization,
        &resolved,
    )
    .unwrap();

    assert!(matches!(
        validate_optimized_resolved_selected_form_layout(
            selected_stage.selected(),
            &machine,
            physical,
            &encoded,
            &resolved,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::PreLayout(
            OptimizedSelectedFormEncodingError::ArtifactMismatch
        ))
    ));
    assert!(matches!(
        validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization(
            selected_stage.selected(),
            &machine,
            physical,
            &baseline,
            &materialization,
            &baseline_layout,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::PreLayout(
            OptimizedSelectedFormEncodingError::ArtifactMismatch
        ))
    ));

    let mut corrupted_layout = resolved.clone();
    let row = corrupted_layout
        .functions_mut()
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find(|row| {
            row.instruction == materialization.materialization().plan().actions[0].instruction
        })
        .unwrap();
    row.bytes[0] ^= 1;
    assert_eq!(
        validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization(
            selected_stage.selected(),
            &machine,
            physical,
            &encoded,
            &materialization,
            &corrupted_layout,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    );

    let mut corrupted = encoded.clone();
    let row = corrupted
        .rows_mut()
        .iter_mut()
        .find(|row| row.instruction == action.instruction)
        .unwrap();
    let SelectedFormEncodingState::Encoded { bytes, .. } = &mut row.state else {
        panic!("MOVN materialization must own pre-layout bytes")
    };
    bytes[0] ^= 1;
    assert_eq!(
        validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization(
            selected_stage.selected(),
            &machine,
            physical,
            &materialization,
            &corrupted,
        ),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    );
}

#[test]
fn compiler_facing_physical_pipeline_routes_aarch64_movn_through_the_generic_post_allocation_join()
{
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact_with_false_literal(
        IntegerValue::Unsigned(u64::MAX as u128),
    );
    let selections = OptimizationSelections::new([
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let mut staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();

    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(staged.selected_lowering_completion(), None);
    assert!(staged.function_relative_manifest().is_some());
    assert!(
        optimization_pipeline_report(&staged)
            .function_relative()
            .is_some()
    );
    assert!(staged.post_allocation_machine_optimization().is_some());
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
        &mut staged
    else {
        panic!("the exact MOVN selection must use its function-relative realization")
    };
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) =
        realization.optimization()
    else {
        panic!("the generic join must retain the exact MOVN result")
    };
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization)
            .unwrap(),
        *realization.custody()
    );
    assert_eq!(
        realization.custody().exit_contract(),
        realization.exit_contract().identity()
    );
    assert_eq!(
        realization.custody().realization(),
        realization.manifest().record().identity
    );
    assert!(matches!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
            artifact_identity,
        } if artifact_identity == materialization.custody().materialization().bytes()
    ));
    assert_eq!(
        realization
            .manifest()
            .record()
            .post_allocation_machine_optimization,
        realization.optimization().custody()
    );
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(
            &realization.manifest().record().encode()
        ),
        Ok(realization.manifest().record().clone())
    );
    assert!(
        realization
            .manifest()
            .record()
            .render_text()
            .contains("post-allocation machine optimization:")
    );
    assert_eq!(
        realization.baseline_encoding().identity(),
        realization.manifest().record().baseline_pre_layout
    );
    assert_eq!(
        realization.encoding().identity(),
        realization.manifest().record().pre_layout
    );
    let baseline_bytes = realization
        .baseline_layout()
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum::<u64>();
    let selected_bytes = realization
        .layout()
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum::<u64>();
    assert!(selected_bytes < baseline_bytes);

    realization.manifest_mut().record_mut().resolved_layout =
        realization.baseline_layout().identity();
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );
}

fn staged_direct_aarch64_movn_physical_pipeline() -> StagedOptimizedVerifiedPhysicalPipeline {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact_with_false_literal(
        IntegerValue::Unsigned(u64::MAX as u128),
    );
    let selections = OptimizationSelections::new([
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap()
}

#[test]
fn generic_post_allocation_realization_rejects_manifest_and_exit_corruption() {
    let mut staged = staged_direct_aarch64_movn_physical_pipeline();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
        &mut staged
    else {
        unreachable!()
    };
    realization
        .manifest_mut()
        .record_mut()
        .post_allocation_machine_optimization = None;
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );

    let mut staged = staged_direct_aarch64_movn_physical_pipeline();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
        &mut staged
    else {
        unreachable!()
    };
    realization
        .exit_contract_mut()
        .contract_mut()
        .layout_custody = WholeFunctionExitLayoutCustody::BaselineNearLayoutV1;
    assert!(matches!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::ExitContract(
            _
        ))
    ));
}

#[test]
fn aarch64_movn_function_relative_realization_composes_after_exact_selected_lowering() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact_with_false_literal(
        IntegerValue::Unsigned(u64::MAX as u128),
    );
    let selections = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = &staged
    else {
        panic!("selected lowering must retain custody before MOVN realization")
    };
    let StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) =
        realization.source()
    else {
        panic!("selected lowering must remain the generic realization source")
    };
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) =
        realization.optimization()
    else {
        panic!("the generic realization must retain the MOVN result")
    };
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(
        staged.selected_lowering_completion(),
        Some(homes.selected_lowering_run().custody().identity())
    );
    assert_eq!(
        staged.function_relative_manifest(),
        Some(realization.manifest())
    );
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
    assert!(matches!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
            artifact_identity,
        } if artifact_identity == materialization.custody().materialization().bytes()
    ));
}
