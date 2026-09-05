//! Exact epoch-two victim choice over the retained blocker roster.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;
use omega_selected_instructions::VirtualRegisterId;

use super::generalized_reload_value_homes::Sources;

fn sources(
    target: NativeTarget,
) -> (
    Sources,
    omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
    omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryWorklist,
) {
    let sources = Sources::new(target);
    let homes = sources.assign(selected_lowering_budget()).unwrap();
    let worklist = omega_selected_instructions_to_register_homes::seed_generalized_spill_recovery_worklist(
        &homes,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1,
        selected_lowering_budget(),
    )
    .unwrap();
    (sources, homes, worklist)
}

#[test]
fn exact_epoch_two_choice_retains_complete_blockers_and_selects_farthest_end() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        let first = sources
            .choose_generalized_victim(&homes, &worklist, selected_lowering_budget())
            .unwrap();
        let second = sources
            .choose_generalized_victim(&homes, &worklist, selected_lowering_budget())
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().choice_count(), 1);
        assert_eq!(first.receipt().contender_count(), 2);
        assert_eq!(first.receipt().usage(), exact_usage());
        assert_eq!(first.receipt().worklist(), worklist.receipt().identity());
        assert_eq!(
            first.receipt().reload_value_homes(),
            homes.receipt().identity()
        );

        let choice = &first.plan().choices[0];
        assert_eq!(choice.work_item.epoch, 2);
        assert_eq!(choice.work_item.ordinal, 0);
        assert_eq!(choice.function, 0);
        assert_eq!(
            choice.block,
            omega_selected_instructions::SelectedBlockId(1)
        );
        assert_eq!(choice.point, LiveRangePoint(14));
        assert_eq!(choice.source_pressure, action(1, 0));
        assert_eq!(choice.reload_candidates.len(), 2);
        assert_eq!(choice.blocking_residents.len(), 2);
        assert_eq!(choice.contenders.len(), 2);
        assert_eq!(
            choice.selected_victim,
            omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Reload(
                action(0, 0)
            )
        );
        let selected = choice
            .blocking_residents
            .iter()
            .find(|resident| resident.value == choice.selected_victim)
            .unwrap();
        assert_eq!(selected.start, LiveRangePoint(12));
        assert_eq!(selected.exclusive_end, LiveRangePoint(17));
        assert_eq!(choice.selected_victim_view, selected.view);
        assert!(choice.reload_candidates.contains(&choice.reclaimed_view));
        assert!(choice.blocking_residents.iter().any(|resident| {
            resident.value
                == omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
        }));
    }
}

#[test]
fn original_first_policy_rejects_the_current_use_original_before_ranking() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        let legacy = sources
            .choose_generalized_victim(&homes, &worklist, selected_lowering_budget())
            .unwrap();
        let guarded = sources
            .choose_generalized_victim_with_policy(
                &homes,
                &worklist,
                omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1,
                selected_lowering_budget(),
            )
            .unwrap();
        assert_ne!(legacy.receipt().identity(), guarded.receipt().identity());
        assert_eq!(guarded.receipt().selected(), homes.receipt().selected());
        assert_eq!(guarded.receipt().ranges(), homes.receipt().ranges());
        let choice = &guarded.plan().choices[0];
        assert!(choice.contenders.iter().any(|contender| {
            contender.value
                == omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
        }));
        assert_eq!(
            choice.selected_victim,
            omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Reload(
                action(0, 0)
            )
        );
        assert_eq!(guarded.receipt().usage(), guarded_exact_usage());

        let original = choice
            .contenders
            .iter()
            .find(|contender| {
                contender.value
                    == omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Original(
                        VirtualRegisterId(5),
                    )
            })
            .unwrap();
        let mut forged = guarded.plan().clone();
        forged.choices[0].selected_victim = original.value;
        forged.choices[0].selected_victim_view = original.resident_view;
        forged.choices[0].reclaimed_view = original.reclaimed_view;
        assert_eq!(
            sources.validate_generalized_victim(&homes, &worklist, forged),
            Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::NonCanonicalChoices)
        );
    }
}

#[test]
fn original_eligibility_policy_has_exact_budget_and_cross_target_custody() {
    let exact = OptimizationWorkBudget::new(4, 2, 43, 1, 1).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(3, 2, 43, 1, 1).unwrap(),
        OptimizationWorkBudget::new(4, 1, 43, 1, 1).unwrap(),
        OptimizationWorkBudget::new(4, 2, 42, 1, 1).unwrap(),
    ];
    let policy = omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1;
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        assert!(
            sources
                .choose_generalized_victim_with_policy(&homes, &worklist, policy, exact)
                .is_ok()
        );
        for budget in insufficient {
            assert!(matches!(
                sources.choose_generalized_victim_with_policy(
                    &homes,
                    &worklist,
                    policy,
                    budget,
                ),
                Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == guarded_exact_usage() && actual == budget
            ));
        }
    }

    let (x86, x86_homes, x86_worklist) = sources(NativeTarget::linux_x64());
    let foreign = x86
        .choose_generalized_victim_with_policy(&x86_homes, &x86_worklist, policy, exact)
        .unwrap()
        .plan()
        .clone();
    let (arm, arm_homes, arm_worklist) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate_generalized_victim(&arm_homes, &arm_worklist, foreign),
        Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::RootMismatch)
    );
}

#[test]
fn independent_replay_rejects_every_choice_surface_and_source_root_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        let canonical = sources
            .choose_generalized_victim(&homes, &worklist, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        for corrupt_root in [
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.worklist =
                    omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistIdentity::from_bytes(
                        [0xc1; 32],
                    );
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.reload_value_homes =
                    omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeIdentity::from_bytes([0xc2; 32]);
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.selected =
                    omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes(
                        [0xc7; 32],
                    );
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.ranges = omega_selected_instructions_to_register_homes::LiveRangeIdentity::from_bytes([0xc8; 32]);
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.legality = omega_selected_instructions_to_register_homes::AllocationLegalityIdentity::from_bytes([0xc3; 32]);
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.register_environment =
                    omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xc4; 32]);
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.allocator_availability =
                    omega_selected_instructions_to_register_homes::AllocatorAvailabilityIdentity::from_bytes([0xc5; 32]);
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.optimization_unit =
                    omega_optimization_core::OptimizationUnitIdentity::from_bytes([0xc6; 32]);
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.fuel_schedule = psi_core::FuelScheduleIdentity::new(99_931).unwrap();
            },
        ] {
            let mut root = canonical.clone();
            corrupt_root(&mut root);
            assert_eq!(
                sources.validate_generalized_victim(&homes, &worklist, root),
                Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::RootMismatch)
            );
        }

        for corrupt in [
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].work_item.epoch += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].source_pressure.ordinal += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].function += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].machine = MachineId::new(99_932).unwrap();
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].block.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].point.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].reload_class.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].reload_candidates.reverse();
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].blocking_residents.pop();
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].blocking_residents[0].exclusive_end.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].contenders.reverse();
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].contenders[0].exclusive_end.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].selected_victim =
                    omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(
                        5,
                    ));
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].selected_victim_view.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices[0].reclaimed_view.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan| {
                plan.choices.clear();
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate_generalized_victim(&homes, &worklist, changed),
                Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::NonCanonicalChoices)
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate_generalized_victim(&homes, &worklist, usage),
            Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_every_representable_axis_and_cross_target_roots_fail_closed() {
    let exact = OptimizationWorkBudget::new(2, 2, 13, 1, 1).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(1, 2, 13, 1, 1).unwrap(),
        OptimizationWorkBudget::new(2, 1, 13, 1, 1).unwrap(),
        OptimizationWorkBudget::new(2, 2, 12, 1, 1).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, homes, worklist) = sources(target);
        assert!(
            sources
                .choose_generalized_victim(&homes, &worklist, exact)
                .is_ok()
        );
        for budget in insufficient {
            assert!(matches!(
                sources.choose_generalized_victim(&homes, &worklist, budget),
                Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == exact_usage() && actual == budget
            ));
        }
    }

    let (x86, x86_homes, x86_worklist) = sources(NativeTarget::linux_x64());
    let foreign = x86
        .choose_generalized_victim(&x86_homes, &x86_worklist, exact)
        .unwrap()
        .plan()
        .clone();
    let (arm, arm_homes, arm_worklist) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate_generalized_victim(&arm_homes, &arm_worklist, foreign),
        Err(omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError::RootMismatch)
    );
}

const fn action(
    epoch: u32,
    ordinal: u32,
) -> omega_selected_instructions_to_register_homes::GeneralizedSpillActionId {
    omega_selected_instructions_to_register_homes::GeneralizedSpillActionId { epoch, ordinal }
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

const fn guarded_exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 4,
        candidates: 2,
        validation_steps: 43,
        commits: 1,
        iterations: 1,
    }
}
