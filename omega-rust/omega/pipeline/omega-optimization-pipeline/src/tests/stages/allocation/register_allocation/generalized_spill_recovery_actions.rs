//! Exact epoch-two logical recovery actions for a compiler-private reload victim.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;

use super::generalized_reload_value_homes::Sources;

fn sources(
    target: NativeTarget,
) -> (
    Sources,
    omega_regalloc::ValidatedGeneralizedReloadValueHomes,
    omega_regalloc::ValidatedGeneralizedSpillRecoveryChoices,
) {
    let sources = Sources::new(target);
    let homes = sources.assign(selected_lowering_budget()).unwrap();
    let worklist = omega_regalloc::seed_generalized_spill_recovery_worklist(
        &homes,
        omega_regalloc::GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let choices = sources
        .choose_generalized_victim(&homes, &worklist, selected_lowering_budget())
        .unwrap();
    (sources, homes, choices)
}

#[test]
fn epoch_two_reload_victim_becomes_exact_target_neutral_logical_obligations() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, choices) = sources(target);
        let first = sources
            .plan_generalized_recovery_actions(&homes, &choices, selected_lowering_budget())
            .unwrap();
        let second = sources
            .plan_generalized_recovery_actions(&homes, &choices, selected_lowering_budget())
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().action_count(), 1);
        assert_eq!(first.receipt().rewrite_count(), 1);
        assert_eq!(first.receipt().usage(), exact_usage());
        assert_eq!(first.receipt().selected(), None);
        assert_eq!(first.receipt().ranges(), None);
        assert_eq!(
            first.receipt().reload_value_homes(),
            homes.receipt().identity()
        );
        assert_eq!(first.receipt().choices(), choices.receipt().identity());

        let action = &first.plan().actions[0];
        assert_eq!(action.source_work_item.epoch, 2);
        assert_eq!(action.source_work_item.ordinal, 0);
        assert_eq!(action.pressure_point, LiveRangePoint(14));
        assert_eq!(action.source_pressure, id(1, 0));
        assert_eq!(
            action.victim,
            omega_regalloc::GeneralizedSpillRecoveryVictim::Reload(id(0, 0))
        );
        assert_eq!(action.current_view, action.reclaimed_view);
        assert_eq!(action.storage.id, id(2, 0));
        assert_eq!(action.store.before_pressure_reload, id(1, 0));
        assert_eq!(action.store.before_instruction.0, 7);
        assert_eq!(
            action.store.source,
            omega_regalloc::GeneralizedSpillRecoveryVictim::Reload(id(0, 0))
        );
        assert_eq!(action.reload.before_instruction.0, 8);
        assert_eq!(action.reload.result, id(2, 0));
        assert_eq!(action.rewrites.len(), 1);
        assert_eq!(action.rewrites[0].point, LiveRangePoint(16));
        assert_eq!(action.rewrites[0].instruction.0, 8);
        assert_eq!(action.rewrites[0].result, id(2, 0));
    }
}

#[test]
fn legacy_reload_victim_identity_encoding_remains_byte_stable() {
    let (sources, homes, choices) = sources(NativeTarget::linux_x64());
    let actions = sources
        .plan_generalized_recovery_actions(&homes, &choices, selected_lowering_budget())
        .unwrap();
    assert_eq!(
        actions.receipt().identity().bytes(),
        [
            254, 115, 107, 46, 104, 52, 194, 173, 214, 253, 118, 21, 182, 138, 242, 16, 243, 219,
            150, 197, 220, 61, 44, 7, 17, 45, 208, 208, 194, 227, 9, 64,
        ]
    );
    let mut wrong_policy = actions.plan().clone();
    wrong_policy.policy =
        omega_regalloc::GeneralizedSpillRecoveryActionPolicy::EpochTwoOriginalVictimLaterSelectedRewritesV1;
    assert_eq!(
        sources.validate_generalized_recovery_actions(&homes, &choices, wrong_policy),
        Err(omega_regalloc::GeneralizedSpillRecoveryActionError::UnsupportedPolicy)
    );
}

#[test]
fn replay_rejects_every_root_and_logical_action_surface_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, choices) = sources(target);
        let canonical = sources
            .plan_generalized_recovery_actions(&homes, &choices, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();
        for corrupt in [
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.generalized_spill_insertion =
                    omega_regalloc::GeneralizedSpillInsertionIdentity::from_bytes([0xd1; 32]);
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.reload_value_homes =
                    omega_regalloc::GeneralizedReloadValueHomeIdentity::from_bytes([0xd2; 32]);
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.choices =
                    omega_regalloc::GeneralizedSpillRecoveryChoiceIdentity::from_bytes([0xd3; 32]);
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.selected = Some(
                    omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes(
                        [0xd7; 32],
                    ),
                );
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.ranges = Some(omega_regalloc::LiveRangeIdentity::from_bytes([0xd8; 32]));
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.register_environment =
                    omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xd4; 32]);
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.allocator_availability =
                    omega_regalloc::AllocatorAvailabilityIdentity::from_bytes([0xd5; 32]);
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.optimization_unit =
                    omega_optimization_core::OptimizationUnitIdentity::from_bytes([0xd6; 32]);
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.fuel_schedule = psi_core::FuelScheduleIdentity::new(99_940).unwrap();
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate_generalized_recovery_actions(&homes, &choices, changed),
                Err(omega_regalloc::GeneralizedSpillRecoveryActionError::RootMismatch)
            );
        }

        for corrupt in [
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                let omega_regalloc::GeneralizedSpillRecoveryVictim::Reload(victim) =
                    &mut plan.actions[0].victim
                else {
                    unreachable!()
                };
                victim.ordinal += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].pressure_point.0 += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].storage.id.epoch += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].store.before_instruction.0 += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].reload.result.ordinal += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| {
                plan.actions[0].rewrites[0].operand += 1
            },
            |plan: &mut omega_regalloc::GeneralizedSpillRecoveryActionPlan| plan.actions.clear(),
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate_generalized_recovery_actions(&homes, &choices, changed),
                Err(omega_regalloc::GeneralizedSpillRecoveryActionError::NonCanonicalActions)
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate_generalized_recovery_actions(&homes, &choices, usage),
            Err(omega_regalloc::GeneralizedSpillRecoveryActionError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_each_representable_axis_and_cross_target_roots_fail_closed() {
    let exact = OptimizationWorkBudget::new(1, 1, 7, 1, 1).unwrap();
    let insufficient = [OptimizationWorkBudget::new(1, 1, 6, 1, 1).unwrap()];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, choices) = sources(target);
        assert!(
            sources
                .plan_generalized_recovery_actions(&homes, &choices, exact)
                .is_ok()
        );
        for budget in insufficient {
            assert!(matches!(
                sources.plan_generalized_recovery_actions(&homes, &choices, budget),
                Err(omega_regalloc::GeneralizedSpillRecoveryActionError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == exact_usage() && actual == budget
            ));
        }
    }

    let (x86, x86_homes, x86_choices) = sources(NativeTarget::linux_x64());
    let foreign = x86
        .plan_generalized_recovery_actions(&x86_homes, &x86_choices, exact)
        .unwrap()
        .plan()
        .clone();
    let (arm, arm_homes, arm_choices) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate_generalized_recovery_actions(&arm_homes, &arm_choices, foreign),
        Err(omega_regalloc::GeneralizedSpillRecoveryActionError::RootMismatch)
    );
}

const fn id(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 1,
        validation_steps: 7,
        commits: 1,
        iterations: 1,
    }
}
