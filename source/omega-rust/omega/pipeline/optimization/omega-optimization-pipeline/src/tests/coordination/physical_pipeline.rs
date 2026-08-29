use crate::tests::*;

#[test]
fn compiler_facing_physical_pipeline_routes_psi_only_and_selected_lowering_suites() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let psi_only_selections =
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(
                psi_only_selections.clone(),
                selected_lowering_budget(),
            )
            .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        assert!(matches!(
            staged,
            StagedOptimizedVerifiedPhysicalPipeline::PsiOnly { .. }
        ));
        assert_eq!(staged.selections(), psi_only_selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert!(staged.function_relative_realization().is_none());
        let report = optimization_pipeline_report(&staged);
        assert_eq!(
            report.pre_physical().identity,
            staged.pre_physical_manifest().record().identity
        );
        assert_eq!(
            report.post_allocation().identity,
            staged.post_allocation_manifest().record().identity
        );
        assert!(report.function_relative().is_none());
        assert_eq!(
            report.render_human_text(OptimizationReportRequest::Suppressed),
            None
        );
        let text = report
            .render_human_text(OptimizationReportRequest::EmitHumanText)
            .expect("explicit human report projection");
        assert!(text.contains("[pre-physical]"));
        assert!(text.contains("[post-allocation]"));
        assert!(!text.contains("[function-relative realization]"));

        for selections in [
            OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap(),
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap(),
        ] {
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                    .unwrap(),
            )
            .unwrap();
            let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                target,
                &[],
            )
            .unwrap();
            let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } = &staged
            else {
                panic!("selected-lowering phase must run when its exact family is selected")
            };
            let homes = realization.homes();
            let machine = realization.machine();
            assert_eq!(staged.selections(), selections.identity());
            assert_eq!(
                staged.selected_lowering_completion(),
                Some(homes.selected_lowering_run().custody().identity())
            );
            assert_eq!(
                staged.function_relative_realization().unwrap().custody(),
                realization.custody()
            );
            assert!(homes.selected_lowering_run().steps().is_empty());
            assert_eq!(
                machine.machine().receipt().post_allocation_manifest(),
                homes.post_allocation_manifest().record().identity
            );
            assert_eq!(
                realization.manifest().record().selections,
                selections.identity()
            );
            assert_eq!(
                realization.manifest().record().publication,
                FunctionRelativeOptimizationUnavailableData::Unavailable
            );
            let report = optimization_pipeline_report(&staged);
            assert_eq!(
                report.pre_physical().identity,
                staged.pre_physical_manifest().record().identity
            );
            assert_eq!(
                report.post_allocation().identity,
                staged.post_allocation_manifest().record().identity
            );
            assert_eq!(
                report
                    .function_relative()
                    .expect("selected lowering has function-relative custody")
                    .identity,
                realization.manifest().record().identity
            );
            assert!(
                report
                    .render_human_text(OptimizationReportRequest::EmitHumanText)
                    .expect("explicit human report projection")
                    .contains("[function-relative realization]")
            );
        }
    }
}

#[test]
fn compiler_facing_physical_pipeline_runs_only_the_named_shared_entry_copy() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_forwarded_parameter_artifact();
        let selections = OptimizationSelections::new([
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery { homes, machine } =
            &staged
        else {
            panic!("the exact allocation-recovery phase must use its fixed-copy route")
        };
        let reanalysis = homes.reanalysis_stage();
        let copies = reanalysis.transformation_stage();
        let plan = copies.copies().plan();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert!(staged.function_relative_manifest().is_none());
        assert!(staged.post_allocation_machine_optimization().is_none());
        assert_eq!(
            copies.custody().policy(),
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
        );
        assert_eq!(copies.custody().copy_count(), 1);
        assert_eq!(plan.copies.len(), 1);
        assert_eq!(plan.copies[0].destinations.len(), 2);
        assert_eq!(reanalysis.custody().entry_transition_count(), 0);
        assert_eq!(reanalysis.legality().receipt().entry_transition_count(), 0);
        assert_eq!(
            machine.machine().receipt().post_allocation_manifest(),
            homes.post_allocation_manifest().record().identity
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                homes, machine,
            )
            .unwrap(),
            machine.custody()
        );
    }
}

#[test]
fn compiler_facing_physical_pipeline_runs_only_the_named_active_resident_rematerialization() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
        let selections = OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::ActiveResidentRematerialization {
            realization,
        } = &staged
        else {
            panic!("the exact rematerialization selection must use its owning realization")
        };
        let rematerialization = realization.source().pre_layout().source();
        let manifest = realization.manifest().record();
        let empty = OptimizationSelections::default().identity();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert!(staged.function_relative_realization().is_none());
        assert_eq!(
            staged
                .active_resident_rematerialization_function_relative_realization()
                .unwrap()
                .custody(),
            realization.custody()
        );
        assert_eq!(
            staged.function_relative_manifest(),
            Some(realization.manifest())
        );
        assert!(staged.post_allocation_machine_optimization().is_none());
        assert_eq!(
            manifest.allocation_recovery_selections,
            selections.identity()
        );
        assert_eq!(manifest.selected_lowering_selections, empty);
        assert_eq!(manifest.post_allocation_machine_selections, empty);
        assert_eq!(manifest.function_relative_layout_selections, empty);
        assert_eq!(manifest.selected_lowering_completion, None);
        assert_eq!(rematerialization.custody().applied_count(), 1);
        assert_eq!(rematerialization.custody().rewritten_use_count(), 2);
        assert_eq!(
            staged
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization.rematerialization().receipt().identity(),
                )
            ]
        );
        assert_eq!(
            staged.machine().machine().receipt().selected(),
            manifest.selected
        );
        assert_eq!(
            manifest.publication,
            FunctionRelativeOptimizationUnavailableData::Unavailable
        );
    }
}

#[test]
fn allocation_recovery_compositions_reject_instead_of_dispatching_a_hidden_policy() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    for selections in [
        OptimizationSelections::new([
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap(),
        OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap(),
    ] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                NativeTarget::linux_x64(),
                &[],
            ),
            Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition)
        ));
    }
}

#[test]
fn compiler_facing_physical_pipeline_runs_only_the_named_aarch64_cbnz_fusion() {
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
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = &staged
    else {
        panic!("the exact post-allocation phase must use its symbolic machine route")
    };
    let homes = realization.homes();
    let machine = realization.machine();
    let optimization = realization.fusion();
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(staged.selected_lowering_completion(), None);
    assert!(staged.function_relative_manifest().is_some());
    assert_eq!(
        staged.function_relative_manifest(),
        Some(realization.manifest())
    );
    assert_eq!(
        validate_aarch64_cbnz_function_relative_realization_custody(realization).unwrap(),
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
        manifest.aarch64_cbnz_fusion,
        Some(realization.fusion().fusion().receipt().identity())
    );
    assert_eq!(manifest.x86_branch_relaxation, None);
    assert!(matches!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 { fusion }
            if fusion == realization.fusion().fusion().receipt().identity()
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

    let ranges = homes.legality_stage().live_range_stage();
    let selected_stage = ranges.liveness_stage().selected_stage();
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
    let omega_machine_optimizer::Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
        source_read,
        ..
    } = branch_disposition
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

    let mut rehashed_corruption = optimization.fusion().plan().clone();
    rehashed_corruption.actions[0].source_read.view = physical.model().view_named("x2").unwrap().id;
    rehashed_corruption.identity =
        omega_machine_optimizer::aarch64_cbnz_fusion_identity(&rehashed_corruption);
    assert_eq!(
        omega_machine_optimizer::validate_aarch64_cbnz_fusion(
            selected_stage.selected(),
            ranges.liveness_stage().liveness(),
            machine.machine(),
            physical,
            rehashed_corruption,
        ),
        Err(omega_machine_optimizer::Aarch64CbnzFusionError::ArtifactMismatch)
    );

    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
        &mut staged
    else {
        unreachable!()
    };
    let original_layout = realization.manifest().record().resolved_layout;
    realization.manifest_mut().record_mut().resolved_layout =
        realization.baseline_layout().identity();
    assert_eq!(
        validate_aarch64_cbnz_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );
    realization.manifest_mut().record_mut().resolved_layout = original_layout;
    let original_custody = realization.exit_contract().contract().layout_custody;
    realization
        .exit_contract_mut()
        .contract_mut()
        .layout_custody = WholeFunctionExitLayoutCustody::BaselineNearLayoutV1;
    assert!(matches!(
        validate_aarch64_cbnz_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::ExitContract(
            WholeFunctionExitContractError::ArtifactMismatch
        ))
    ));
    realization
        .exit_contract_mut()
        .contract_mut()
        .layout_custody = original_custody;
    assert_eq!(
        validate_aarch64_cbnz_function_relative_realization_custody(realization).unwrap(),
        *realization.custody()
    );
}

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
fn compiler_facing_physical_pipeline_routes_only_the_named_aarch64_movn_materialization() {
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
    assert!(staged.post_allocation_machine_optimization().is_none());
    assert!(staged.post_allocation_movn_optimization().is_some());
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachineMovn { realization } =
        &mut staged
    else {
        panic!("the exact MOVN selection must use its function-relative realization")
    };
    assert_eq!(
        validate_optimized_aarch64_movn_function_relative_realization(realization).unwrap(),
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
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization
        } if materialization == realization.materialization().custody().materialization()
    ));
    assert_eq!(
        realization.manifest().record().aarch64_movn_materialization,
        Some(realization.materialization().custody().materialization())
    );
    assert_eq!(realization.manifest().record().aarch64_cbnz_fusion, None);
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(
            &realization.manifest().record().encode()
        ),
        Ok(realization.manifest().record().clone())
    );
    let mut conflicting = realization.manifest().record().clone();
    conflicting.aarch64_cbnz_fusion =
        Some(omega_machine_optimizer::Aarch64CbnzFusionIdentity::from_bytes([0xc2; 32]));
    conflicting.identity = conflicting.recomputed_identity();
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&conflicting.encode()),
        Err(
            FunctionRelativeOptimizationRealizationManifestDecodeError::ConflictingPhysicalTransformations
        )
    );
    assert!(
        realization
            .manifest()
            .record()
            .render_text()
            .contains("AArch64 MOVN materialization:")
    );
    let layout = realization.source();
    let custody = validate_optimized_aarch64_movn_resolved_selected_form_layout(layout).unwrap();
    assert_eq!(custody, *layout.custody());
    assert_eq!(
        custody.materialization(),
        layout.materialization().custody()
    );
    assert_eq!(
        custody.baseline_encoding(),
        layout.baseline_encoding().identity()
    );
    assert_eq!(custody.encoding(), layout.encoding().identity());
    assert_eq!(
        custody.baseline_layout(),
        layout.baseline_layout().identity()
    );
    assert_eq!(custody.layout(), layout.layout().identity());
    assert!(custody.selected_bytes() < custody.baseline_bytes());

    crate::stages::realization::aarch64_movn_function_relative_realization::corrupt_aarch64_movn_function_relative_manifest_for_test(
        realization,
    );
    assert!(matches!(
        validate_optimized_aarch64_movn_function_relative_realization(realization),
        Err(OptimizedAarch64MovnFunctionRelativeRealizationError::RootMismatch)
    ));
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
fn aarch64_movn_function_relative_realization_rejects_source_exit_and_receipt_corruption() {
    let mut staged = staged_direct_aarch64_movn_physical_pipeline();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachineMovn { realization } =
        &mut staged
    else {
        unreachable!()
    };
    crate::stages::realization::aarch64_movn_function_relative_realization::corrupt_aarch64_movn_function_relative_source_for_test(
        realization,
    );
    assert!(matches!(
        validate_optimized_aarch64_movn_function_relative_realization(realization),
        Err(
            OptimizedAarch64MovnFunctionRelativeRealizationError::Source(
                OptimizedAarch64MovnResolvedSelectedFormLayoutError::Layout(
                    OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch
                )
            )
        )
    ));

    let mut staged = staged_direct_aarch64_movn_physical_pipeline();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachineMovn { realization } =
        &mut staged
    else {
        unreachable!()
    };
    crate::stages::realization::aarch64_movn_function_relative_realization::corrupt_aarch64_movn_function_relative_exit_for_test(
        realization,
    );
    assert!(matches!(
        validate_optimized_aarch64_movn_function_relative_realization(realization),
        Err(
            OptimizedAarch64MovnFunctionRelativeRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch
            )
        )
    ));

    let mut staged = staged_direct_aarch64_movn_physical_pipeline();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachineMovn { realization } =
        &mut staged
    else {
        unreachable!()
    };
    crate::stages::realization::aarch64_movn_function_relative_realization::corrupt_aarch64_movn_function_relative_receipt_for_test(
        realization,
    );
    assert!(matches!(
        validate_optimized_aarch64_movn_function_relative_realization(realization),
        Err(OptimizedAarch64MovnFunctionRelativeRealizationError::ReceiptMismatch)
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
    let StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachineMovn {
        realization,
    } = &staged
    else {
        panic!("selected lowering must retain custody before MOVN realization")
    };
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(
        staged.selected_lowering_completion(),
        Some(
            realization
                .homes()
                .selected_lowering_run()
                .custody()
                .identity()
        )
    );
    assert_eq!(
        staged.function_relative_manifest(),
        Some(realization.manifest())
    );
    assert_eq!(
        validate_selected_lowering_aarch64_movn_function_relative_realization(realization).unwrap(),
        *realization.custody()
    );
    assert_eq!(
        realization.manifest().record().selected_lowering_completion,
        staged.selected_lowering_completion()
    );
    assert_eq!(
        realization.manifest().record().aarch64_movn_materialization,
        Some(realization.materialization().custody().materialization())
    );
    assert!(matches!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization
        } if materialization == realization.materialization().custody().materialization()
    ));
}

#[test]
fn aarch64_post_allocation_machine_composition_rejects_without_hidden_ordering_policy() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact_with_false_literal(
        IntegerValue::Unsigned(u64::MAX as u128),
    );
    let selections = OptimizationSelections::new([
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
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

    assert!(matches!(
        stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_arm64(),
            &[],
        ),
        Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition)
    ));
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
    let StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachine {
        realization,
    } = &staged
    else {
        panic!("selected lowering must retain custody before post-allocation fusion")
    };
    let homes = realization.homes();
    let machine = realization.machine();
    let optimization = realization.fusion();
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(
        staged.selected_lowering_completion(),
        Some(homes.selected_lowering_run().custody().identity())
    );
    assert_eq!(optimization.fusion().receipt().action_count(), 1);
    assert_eq!(
        staged.function_relative_manifest(),
        Some(realization.manifest())
    );
    assert_eq!(
        validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody(realization)
            .unwrap(),
        *realization.custody()
    );
    assert_eq!(
        realization.manifest().record().selected_lowering_completion,
        staged.selected_lowering_completion()
    );
    assert_eq!(
        realization.manifest().record().aarch64_cbnz_fusion,
        Some(optimization.fusion().receipt().identity())
    );
    assert_eq!(
        validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
            homes,
            machine,
            optimization,
        )
        .unwrap(),
        optimization.custody()
    );
}
