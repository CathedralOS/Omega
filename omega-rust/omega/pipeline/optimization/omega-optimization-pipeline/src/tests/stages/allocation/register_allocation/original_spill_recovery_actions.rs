//! Exact epoch-two logical obligations for one selected original victim.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;
use omega_selected_instructions::VirtualRegisterId;

use super::generalized_reload_value_homes::Sources;

fn sources(
    target: NativeTarget,
) -> (
    Sources,
    omega_regalloc::ValidatedGeneralizedReloadValueHomes,
    omega_regalloc::ValidatedGeneralizedSpillRecoveryChoices,
) {
    let sources = Sources::from_legality(
        staged_active_resident_original_victim_chain_two_view_legality(target),
    );
    let homes = sources.assign(selected_lowering_budget()).unwrap();
    let worklist = omega_regalloc::seed_generalized_spill_recovery_worklist(
        &homes,
        omega_regalloc::GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let choices = sources
        .choose_generalized_victim_with_policy(
            &homes,
            &worklist,
            omega_regalloc::GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1,
            selected_lowering_budget(),
        )
        .unwrap();
    (sources, homes, choices)
}

#[test]
fn selected_original_becomes_exact_target_neutral_logical_obligations() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, choices) = sources(target);
        let first = sources
            .plan_original_recovery_actions(&homes, &choices, exact_budget())
            .unwrap();
        let second = sources
            .plan_original_recovery_actions(&homes, &choices, exact_budget())
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().action_count(), 1);
        assert_eq!(first.receipt().rewrite_count(), 1);
        assert_eq!(first.receipt().usage(), exact_usage());
        assert_eq!(
            first.receipt().selected(),
            Some(choices.receipt().selected())
        );
        assert_eq!(first.receipt().ranges(), Some(choices.receipt().ranges()));
        let mut detached = first.plan().clone();
        detached.selected = None;
        assert_ne!(
            omega_regalloc::generalized_spill_recovery_action_identity(&detached),
            first.receipt().identity()
        );

        let action = &first.plan().actions[0];
        let victim = omega_regalloc::GeneralizedSpillRecoveryVictim::Original(VirtualRegisterId(5));
        assert_eq!(action.source_work_item.epoch, 2);
        assert_eq!(action.source_work_item.ordinal, 0);
        assert_eq!(action.pressure_point, LiveRangePoint(14));
        assert_eq!(action.source_pressure, id(1, 0));
        assert_eq!(action.victim, victim);
        assert_eq!(action.current_view, action.reclaimed_view);
        assert_eq!(action.storage.id, id(2, 0));
        assert_eq!(action.store.before_pressure_reload, id(1, 0));
        assert_eq!(action.store.before_instruction.0, 7);
        assert_eq!(action.store.source, victim);
        assert_eq!(action.reload.before_instruction.0, 8);
        assert_eq!(action.reload.result, id(2, 0));
        assert_eq!(action.rewrites.len(), 1);
        assert_eq!(action.rewrites[0].point, LiveRangePoint(16));
        assert_eq!(action.rewrites[0].instruction.0, 8);
        assert_eq!(action.rewrites[0].operand, 0);
        assert_eq!(action.rewrites[0].result, id(2, 0));
    }
}

#[test]
fn independent_replay_rejects_every_new_root_and_action_surface_mutation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, choices) = sources(target);
        let canonical = sources
            .plan_original_recovery_actions(&homes, &choices, exact_budget())
            .unwrap()
            .plan()
            .clone();
        for corrupt in [
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.selected = Some(
                    omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes(
                        [0xc1; 32],
                    ),
                )
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.ranges = Some(omega_regalloc::LiveRangeIdentity::from_bytes([0xc2; 32]))
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| plan.selected = None,
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| plan.ranges = None,
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.policy = omega_regalloc::GeneralizedSpillRecoveryActionPolicy::EpochTwoReloadVictimLaterGeneralizedRewritesV1
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate_original_recovery_actions(&homes, &choices, changed),
                Err(omega_regalloc::GeneralizedSpillRecoveryActionError::RootMismatch)
            );
        }
        for corrupt in [
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].victim =
                    omega_regalloc::GeneralizedSpillRecoveryVictim::Original(VirtualRegisterId(6))
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].store.source =
                    omega_regalloc::GeneralizedSpillRecoveryVictim::Reload(id(0, 0))
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].victim_class.0 += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].reload.before_instruction.0 += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].rewrites[0].operand += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| plan.actions.clear(),
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate_original_recovery_actions(&homes, &choices, changed),
                Err(omega_regalloc::GeneralizedSpillRecoveryActionError::NonCanonicalActions)
            );
        }
        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate_original_recovery_actions(&homes, &choices, usage),
            Err(omega_regalloc::GeneralizedSpillRecoveryActionError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_cross_target_custody_and_recursive_refusal_are_typed() {
    let insufficient = OptimizationWorkBudget::new(1, 1, 37, 1, 1).unwrap();
    for nonrepresentable in [
        OptimizationWorkBudget::new(0, 1, 38, 1, 1),
        OptimizationWorkBudget::new(1, 0, 38, 1, 1),
        OptimizationWorkBudget::new(1, 1, 38, 0, 1),
        OptimizationWorkBudget::new(1, 1, 38, 1, 0),
    ] {
        assert!(nonrepresentable.is_err());
    }
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, choices) = sources(target);
        let actions = sources
            .plan_original_recovery_actions(&homes, &choices, exact_budget())
            .unwrap();
        assert!(matches!(
            sources.plan_original_recovery_actions(&homes, &choices, insufficient),
            Err(omega_regalloc::GeneralizedSpillRecoveryActionError::BudgetExceeded {
                required,
                budget,
            }) if required == exact_usage() && budget == insufficient
        ));
        assert_eq!(
            sources.schedule_recursive_spills(&actions, selected_lowering_budget()),
            Err(
                omega_regalloc::RecursiveSpillInsertionError::UnsupportedRecoveryVictim {
                    function: 0,
                    action: id(2, 0),
                    victim: omega_regalloc::GeneralizedSpillRecoveryVictim::Original(
                        VirtualRegisterId(5),
                    ),
                }
            )
        );
    }

    let (x86, x86_homes, x86_choices) = sources(NativeTarget::linux_x64());
    let foreign = x86
        .plan_original_recovery_actions(&x86_homes, &x86_choices, exact_budget())
        .unwrap()
        .plan()
        .clone();
    let (arm, arm_homes, arm_choices) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate_original_recovery_actions(&arm_homes, &arm_choices, foreign),
        Err(omega_regalloc::GeneralizedSpillRecoveryActionError::RootMismatch)
    );
}

const fn id(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
}

fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1, 1, 38, 1, 1).unwrap()
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 1,
        validation_steps: 38,
        commits: 1,
        iterations: 1,
    }
}
