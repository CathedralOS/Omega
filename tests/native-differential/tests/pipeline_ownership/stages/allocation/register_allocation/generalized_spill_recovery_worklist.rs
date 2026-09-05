//! Epoch-one generalized pressure becomes exact bounded epoch-two work.

use crate::tests::*;
use optimization_core::OptimizationWorkUsage;
use selected_instructions::VirtualRegisterId;

use super::generalized_reload_value_homes::Sources;

fn seed(
    source: &selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
    budget: OptimizationWorkBudget,
) -> Result<
    selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryWorklist,
    selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistError,
> {
    selected_instructions_to_register_homes::seed_generalized_spill_recovery_worklist(
        source,
        selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1,
        budget,
    )
}

fn validate(
    source: &selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
    plan: selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPlan,
) -> Result<
    selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryWorklist,
    selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistError,
> {
    selected_instructions_to_register_homes::validate_generalized_spill_recovery_worklist(
        source, plan,
    )
}

#[test]
fn exact_epoch_two_work_retains_the_pressure_domain_and_blockers_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = Sources::new(target);
        let homes = sources.assign(selected_lowering_budget()).unwrap();
        let first = seed(&homes, selected_lowering_budget()).unwrap();
        let second = seed(&homes, selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().work_item_count(), 1);
        assert_eq!(first.receipt().blocking_home_count(), 2);
        assert_eq!(first.receipt().usage(), exact_usage());
        assert_eq!(
            first.receipt().reload_value_homes(),
            homes.receipt().identity()
        );

        let item = first.plan().functions[0].item.as_ref().unwrap();
        assert_eq!(item.id.epoch, 2);
        assert_eq!(item.id.ordinal, 0);
        assert_eq!(item.source_pressure, action(1, 0));
        assert!(matches!(
            item.source,
            selected_instructions_to_register_homes::GeneralizedSpillActionSource::EpochOne { .. }
        ));
        assert_eq!(item.block, selected_instructions::SelectedBlockId(1));
        assert_eq!(item.start, LiveRangePoint(14));
        assert_eq!(item.exclusive_end, LiveRangePoint(15));
        assert_eq!(item.candidates.len(), 2);
        assert!(item.candidates.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(item.blocking_homes.len(), 2);
        assert!(item.blocking_homes.iter().any(|home| {
            home.value == selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Reload(action(0, 0))
        }));
        assert!(item.blocking_homes.iter().any(|home| {
            home.value
                == selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
        }));
    }
}

#[test]
fn independent_replay_rejects_root_item_domain_blocker_order_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = Sources::new(target);
        let homes = sources.assign(selected_lowering_budget()).unwrap();
        let canonical = seed(&homes, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        let mut root = canonical.clone();
        root.reload_value_homes =
            selected_instructions_to_register_homes::GeneralizedReloadValueHomeIdentity::from_bytes(
                [0xb1; 32],
            );
        assert_eq!(
            validate(&homes, root),
            Err(selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistError::RootMismatch)
        );

        for corrupt in [
            |plan: &mut selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPlan| {
                plan.functions[0].item.as_mut().unwrap().id.epoch = 3;
            },
            |plan: &mut selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPlan| {
                plan.functions[0]
                    .item
                    .as_mut()
                    .unwrap()
                    .source_pressure
                    .ordinal = 1;
            },
            |plan: &mut selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPlan| {
                plan.functions[0]
                    .item
                    .as_mut()
                    .unwrap()
                    .candidates
                    .reverse();
            },
            |plan: &mut selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPlan| {
                plan.functions[0]
                    .item
                    .as_mut()
                    .unwrap()
                    .blocking_homes
                    .pop();
            },
            |plan: &mut selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPlan| {
                plan.functions[0].item = None;
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                validate(&homes, changed),
                Err(
                    selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistError::NonCanonicalWorklist {
                        function: 0,
                    }
                )
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            validate(&homes, usage),
            Err(selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_and_every_representable_first_over_axis_are_typed() {
    let exact = OptimizationWorkBudget::new(2, 2, 13, 1, 1).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(1, 2, 13, 1, 1).unwrap(),
        OptimizationWorkBudget::new(2, 1, 13, 1, 1).unwrap(),
        OptimizationWorkBudget::new(2, 2, 12, 1, 1).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = Sources::new(target);
        let homes = sources.assign(selected_lowering_budget()).unwrap();
        assert!(seed(&homes, exact).is_ok());
        for budget in insufficient {
            assert!(matches!(
                seed(&homes, budget),
                Err(selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == exact_usage() && actual == budget
            ));
        }
    }

    let x86_sources = Sources::new(NativeTarget::linux_x64());
    let x86_homes = x86_sources.assign(selected_lowering_budget()).unwrap();
    let foreign = seed(&x86_homes, exact).unwrap().plan().clone();
    let arm_sources = Sources::new(NativeTarget::linux_arm64());
    let arm_homes = arm_sources.assign(selected_lowering_budget()).unwrap();
    assert_eq!(
        validate(&arm_homes, foreign),
        Err(selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistError::RootMismatch)
    );
}

const fn action(
    epoch: u32,
    ordinal: u32,
) -> selected_instructions_to_register_homes::GeneralizedSpillActionId {
    selected_instructions_to_register_homes::GeneralizedSpillActionId { epoch, ordinal }
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 2,
        candidates: 2,
        validation_steps: 13,
        commits: 1,
        iterations: 1,
    }
}
