//! Psi-only and selected-lowering route selection and reporting.

use omega_optimization_core::PostTerminalOptimizationSelections;

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, FunctionRelativeOptimizationUnavailableData,
    NativeTarget, Optimization, OptimizationReportRequest, OptimizationSelections,
    OptimizedVerifiedPhysicalPipelineError, StagedOptimizedVerifiedPhysicalPipeline,
    conditional_exact_binary_artifact,
    lower_optimized_to_target_operations_with_provider_executions, optimization_pipeline_report,
    optimize_artifact_sections, selected_lowering_budget,
    stage_optimized_verified_physical_pipeline,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
};

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
            StagedOptimizedVerifiedPhysicalPipeline::FixedFrame { .. }
        ));
        assert_eq!(staged.selections(), psi_only_selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert!(
            staged
                .selected_lowering_function_relative_realization()
                .is_none()
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
        assert!(report.function_relative().is_some());
        assert_eq!(
            report.render_human_text(OptimizationReportRequest::Suppressed),
            None
        );
        let text = report
            .render_human_text(OptimizationReportRequest::EmitHumanText)
            .expect("explicit human report projection");
        assert!(text.contains("[pre-physical]"));
        assert!(text.contains("[post-allocation]"));
        assert!(text.contains("[function-relative realization]"));

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
                staged
                    .selected_lowering_function_relative_realization()
                    .unwrap()
                    .custody(),
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
fn physical_stage_rejects_a_substituted_post_terminal_projection() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            selected_lowering_budget(),
        )
        .unwrap(),
    )
    .unwrap();
    let optimized_target = lower_optimized_to_target_operations_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    let substituted = PostTerminalOptimizationSelections::new(
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap(),
    )
    .unwrap();

    assert!(matches!(
        stage_optimized_verified_physical_pipeline(optimized_target, &substituted),
        Err(OptimizedVerifiedPhysicalPipelineError::PostTerminalSelectionMismatch)
    ));
}
