//! Positive, disabled, deterministic, and fixed-point behavior.

use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::{
    PostAllocationSelectedTransformation, PressureRematerializationError, choose_spill_victims,
    classify_pressure_recovery, rematerialize_selected_active_resident,
};

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, Optimization, OptimizationSelections,
    OptimizedActiveResidentRematerializationError, StagedOptimizedVerifiedPhysicalPipeline,
    conditional_active_resident_exact_add_chain_artifact, optimize_artifact_sections,
    selected_lowering_budget, stage_optimized_active_resident_rematerialization,
    stage_optimized_allocation_legality, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    staged_active_resident_exact_add_chain, validate_optimized_active_resident_rematerialization,
};

use super::fixture::*;

#[test]
fn active_resident_rule_is_disabled_without_its_exact_selection() {
    for target in targets() {
        let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
        let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(
                selections.clone(),
                OptimizationWorkBudget::new(10_000, 10_000, 100_000, 10_000, 128).unwrap(),
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
        assert_eq!(staged.selections(), selections.identity());
        assert!(
            staged
                .post_allocation_manifest()
                .record()
                .selected_transformations
                .is_empty()
        );
    }
}

#[test]
fn active_resident_rule_declines_when_ordinary_allocation_has_no_pressure() {
    for target in targets() {
        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_active_resident_exact_add_chain(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_active_resident_rematerialization(
                source,
                CHOICE_POLICY,
                CLASSIFICATION_POLICY,
                REMATERIALIZATION_POLICY,
                selected_lowering_budget(),
            ),
            Err(
                OptimizedActiveResidentRematerializationError::Rematerialization(
                    PressureRematerializationError::NoAction
                )
            )
        ));
    }
}

#[test]
fn active_resident_rule_reconstructs_deterministically_and_reaches_a_rule_core_fixed_point() {
    for target in targets() {
        let budget = selected_lowering_budget();
        let first = run(target, budget).unwrap();
        let repeated = run(target, budget).unwrap();

        assert_eq!(
            validate_optimized_active_resident_rematerialization(&first).unwrap(),
            first.custody()
        );
        assert_eq!(first.custody(), repeated.custody());
        assert_eq!(first.choices().plan(), repeated.choices().plan());
        assert_eq!(
            first.classifications().plan(),
            repeated.classifications().plan()
        );
        assert_eq!(
            first.rematerialization().plan(),
            repeated.rematerialization().plan()
        );
        assert_eq!(
            first.rematerialization().transformed(),
            repeated.rematerialization().transformed()
        );
        assert_eq!(first.liveness().plan(), repeated.liveness().plan());
        assert_eq!(first.ranges().plan(), repeated.ranges().plan());
        assert_eq!(first.legality().plan(), repeated.legality().plan());
        assert_eq!(first.homes().plan(), repeated.homes().plan());
        assert_eq!(
            first.post_allocation_manifest().record(),
            repeated.post_allocation_manifest().record()
        );
        assert_eq!(first.custody().applied_count(), 1);
        assert_eq!(first.custody().rewritten_use_count(), 2);
        assert_eq!(
            first
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    first.rematerialization().receipt().identity()
                )
            ]
        );

        // The staged carrier cannot be fed back into its own entrance. Replay the
        // exact rule core over its rebuilt post-transform analyses instead.
        let source = first.source();
        let environment = source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let choices = choose_spill_victims(
            first.legality(),
            first.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            CHOICE_POLICY,
            budget,
        )
        .unwrap();
        assert!(
            choices
                .plan()
                .functions
                .iter()
                .all(|function| function.choice.is_none())
        );
        let classifications = classify_pressure_recovery(
            first.rematerialization(),
            first.ranges(),
            first.legality(),
            &choices,
            CLASSIFICATION_POLICY,
            budget,
        )
        .unwrap();
        assert!(
            classifications
                .plan()
                .functions
                .iter()
                .all(|function| function.classification.is_none())
        );
        let second_application = || {
            rematerialize_selected_active_resident(
                first.rematerialization(),
                first.ranges(),
                first.legality(),
                &choices,
                &classifications,
                source.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                REMATERIALIZATION_POLICY,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 10).unwrap(),
            )
        };
        assert_eq!(
            second_application(),
            Err(PressureRematerializationError::NoAction)
        );
        assert_eq!(second_application(), second_application());
    }
}
