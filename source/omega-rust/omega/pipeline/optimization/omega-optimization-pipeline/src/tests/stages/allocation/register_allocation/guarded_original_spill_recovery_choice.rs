//! The exact public fork/join graph reaches a typed guarded-original epoch-two choice.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;
use omega_selected_instructions::VirtualRegisterId;

use super::generalized_reload_value_homes::Sources;

fn sources(
    target: NativeTarget,
) -> (
    Sources,
    omega_regalloc::ValidatedGeneralizedReloadValueHomes,
    omega_regalloc::ValidatedGeneralizedSpillRecoveryWorklist,
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
    (sources, homes, worklist)
}

#[test]
fn exact_graph_selects_an_eligible_original_before_the_reload() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        let first = sources
            .choose_generalized_victim_with_policy(&homes, &worklist, policy(), exact_budget())
            .unwrap();
        let second = sources
            .choose_generalized_victim_with_policy(&homes, &worklist, policy(), exact_budget())
            .unwrap();
        let legacy = sources
            .choose_generalized_victim(&homes, &worklist, selected_lowering_budget())
            .unwrap();
        assert_eq!(first, second);
        assert_ne!(first.receipt().identity(), legacy.receipt().identity());
        assert_eq!(
            legacy.plan().choices[0].selected_victim,
            omega_regalloc::GeneralizedReloadCoexistingValue::Reload(action(0, 0))
        );
        assert_eq!(first.receipt().choice_count(), 1);
        assert_eq!(first.receipt().contender_count(), 2);
        assert_eq!(first.receipt().usage(), exact_usage());
        assert_eq!(first.receipt().selected(), homes.receipt().selected());
        assert_eq!(first.receipt().ranges(), homes.receipt().ranges());

        let choice = &first.plan().choices[0];
        assert_eq!(choice.point, LiveRangePoint(14));
        assert_eq!(
            choice.selected_victim,
            omega_regalloc::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
        );
        assert_eq!(choice.selected_victim_view, choice.reclaimed_view);
        assert_eq!(
            choice
                .blocking_residents
                .iter()
                .map(|resident| resident.value)
                .collect::<Vec<_>>(),
            vec![
                omega_regalloc::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5)),
                omega_regalloc::GeneralizedReloadCoexistingValue::Reload(action(0, 0)),
            ]
        );
        assert_eq!(
            choice
                .contenders
                .iter()
                .map(|contender| (contender.value, contender.exclusive_end))
                .collect::<Vec<_>>(),
            vec![
                (
                    omega_regalloc::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(
                        5
                    ),),
                    LiveRangePoint(17),
                ),
                (
                    omega_regalloc::GeneralizedReloadCoexistingValue::Reload(action(0, 0)),
                    LiveRangePoint(19),
                ),
            ]
        );
        let original = choice
            .blocking_residents
            .iter()
            .find(|resident| resident.value == choice.selected_victim)
            .unwrap();
        assert_eq!(original.start, LiveRangePoint(13));
        assert_eq!(original.exclusive_end, LiveRangePoint(17));
    }
}

#[test]
fn logical_action_planning_still_refuses_the_original_victim() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        let choice = sources
            .choose_generalized_victim_with_policy(&homes, &worklist, policy(), exact_budget())
            .unwrap();
        assert_eq!(
            sources.plan_generalized_recovery_actions(&homes, &choice, selected_lowering_budget(),),
            Err(
                omega_regalloc::GeneralizedSpillRecoveryActionError::UnsupportedVictim {
                    function: 0,
                }
            )
        );
    }
}

#[test]
fn independent_replay_rejects_reload_original_and_root_forgery() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        let canonical = sources
            .choose_generalized_victim_with_policy(&homes, &worklist, policy(), exact_budget())
            .unwrap()
            .plan()
            .clone();
        assert!(
            sources
                .validate_generalized_victim(&homes, &worklist, canonical.clone())
                .is_ok()
        );

        let mut reload = canonical.clone();
        let contender = reload.choices[0]
            .contenders
            .iter()
            .find(|contender| {
                contender.value
                    == omega_regalloc::GeneralizedReloadCoexistingValue::Reload(action(0, 0))
            })
            .copied()
            .unwrap();
        reload.choices[0].selected_victim = contender.value;
        reload.choices[0].selected_victim_view = contender.resident_view;
        reload.choices[0].reclaimed_view = contender.reclaimed_view;
        assert_eq!(
            sources.validate_generalized_victim(&homes, &worklist, reload),
            Err(omega_regalloc::GeneralizedSpillRecoveryChoiceError::NonCanonicalChoices)
        );

        let mut original = canonical.clone();
        original.choices[0].selected_victim =
            omega_regalloc::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(6));
        assert_eq!(
            sources.validate_generalized_victim(&homes, &worklist, original),
            Err(omega_regalloc::GeneralizedSpillRecoveryChoiceError::NonCanonicalChoices)
        );

        let mut root = canonical;
        root.selected =
            omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0x85; 32]);
        assert_eq!(
            sources.validate_generalized_victim(&homes, &worklist, root),
            Err(omega_regalloc::GeneralizedSpillRecoveryChoiceError::RootMismatch)
        );
    }

    let (x86, x86_homes, x86_worklist) = sources(NativeTarget::linux_x64());
    let foreign = x86
        .choose_generalized_victim_with_policy(&x86_homes, &x86_worklist, policy(), exact_budget())
        .unwrap()
        .plan()
        .clone();
    let (arm, arm_homes, arm_worklist) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate_generalized_victim(&arm_homes, &arm_worklist, foreign),
        Err(omega_regalloc::GeneralizedSpillRecoveryChoiceError::RootMismatch)
    );
}

#[test]
fn guarded_original_choice_has_exact_representable_budget_boundaries() {
    let insufficient = [
        OptimizationWorkBudget::new(3, 2, 46, 1, 1).unwrap(),
        OptimizationWorkBudget::new(4, 1, 46, 1, 1).unwrap(),
        OptimizationWorkBudget::new(4, 2, 45, 1, 1).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        assert!(
            sources
                .choose_generalized_victim_with_policy(&homes, &worklist, policy(), exact_budget(),)
                .is_ok()
        );
        for budget in insufficient {
            assert!(matches!(
                sources.choose_generalized_victim_with_policy(
                    &homes,
                    &worklist,
                    policy(),
                    budget,
                ),
                Err(omega_regalloc::GeneralizedSpillRecoveryChoiceError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == exact_usage() && actual == budget
            ));
        }
    }
}

const fn policy() -> omega_regalloc::GeneralizedSpillRecoveryChoicePolicy {
    omega_regalloc::GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1
}

const fn action(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
}

fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(4, 2, 46, 1, 1).unwrap()
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 4,
        candidates: 2,
        validation_steps: 46,
        commits: 1,
        iterations: 1,
    }
}
