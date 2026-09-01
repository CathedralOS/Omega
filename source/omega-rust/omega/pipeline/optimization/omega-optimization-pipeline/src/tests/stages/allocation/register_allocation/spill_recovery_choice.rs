//! Deterministic epoch-one second-victim evidence.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;

use super::{
    reload_value_homes::ReloadSources,
    spill_recovery_worklist::{pressure_sources, seed},
};

fn choose(
    sources: &ReloadSources,
    budget: OptimizationWorkBudget,
) -> Result<omega_regalloc::ValidatedSpillRecoveryChoices, omega_regalloc::SpillRecoveryChoiceError>
{
    let worklist = seed(sources, selected_lowering_budget()).unwrap();
    let ranges = sources.legality().live_range_stage();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    omega_regalloc::choose_spill_recovery_victims(
        &worklist,
        sources.insertion(),
        sources.legality().legality(),
        ranges.ranges(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        omega_regalloc::SpillRecoveryChoicePolicy::EpochOneFarthestEndThenHighestVregV1,
        budget,
    )
}

fn validate(
    sources: &ReloadSources,
    plan: omega_regalloc::SpillRecoveryChoicePlan,
) -> Result<omega_regalloc::ValidatedSpillRecoveryChoices, omega_regalloc::SpillRecoveryChoiceError>
{
    let worklist = seed(sources, selected_lowering_budget()).unwrap();
    let ranges = sources.legality().live_range_stage();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    omega_regalloc::validate_spill_recovery_choices(
        &worklist,
        sources.insertion(),
        sources.legality().legality(),
        ranges.ranges(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        plan,
    )
}

#[test]
fn epoch_one_choice_is_deterministic_and_exact_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let first = choose(&sources, selected_lowering_budget()).unwrap();
        let second = choose(&sources, selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().choice_count(), 1);
        assert_eq!(first.receipt().contender_count(), 2);
        assert_eq!(first.receipt().usage(), first.plan().usage);
        assert_eq!(
            first.plan().usage,
            OptimizationWorkUsage {
                rule_evaluations: 5,
                candidates: 2,
                validation_steps: 10,
                commits: 1,
                iterations: 1,
            }
        );

        let choice = &first.plan().choices[0];
        assert_eq!(choice.work_item.epoch, 1);
        assert_eq!(choice.work_item.ordinal, 0);
        assert_eq!(choice.function, 0);
        assert_eq!(choice.point, LiveRangePoint(12));
        assert_eq!(choice.selected_victim, VirtualRegisterId(3));
        assert_eq!(choice.selected_victim_view, choice.reclaimed_view);
        assert_eq!(
            choice
                .active_residents
                .iter()
                .map(|resident| (
                    resident.virtual_register,
                    resident.start,
                    resident.exclusive_end,
                ))
                .collect::<Vec<_>>(),
            vec![
                (VirtualRegisterId(3), LiveRangePoint(9), LiveRangePoint(15),),
                (VirtualRegisterId(4), LiveRangePoint(11), LiveRangePoint(13),),
            ]
        );
        assert_eq!(choice.contenders.len(), 2);
        assert!(choice.contenders.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(choice.contenders.iter().all(|contender| {
            choice.reload_candidates.contains(&contender.reclaimed_view)
                && contender.resident_view == contender.reclaimed_view
        }));
    }
}

#[test]
fn independent_replay_rejects_root_resident_contender_selection_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let canonical = choose(&sources, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        let mut root = canonical.clone();
        root.worklist = omega_regalloc::SpillRecoveryWorklistIdentity::from_bytes([0x55; 32]);
        assert_eq!(
            validate(&sources, root),
            Err(omega_regalloc::SpillRecoveryChoiceError::RootMismatch)
        );

        for corrupt in [
            |plan: &mut omega_regalloc::SpillRecoveryChoicePlan| {
                plan.choices[0].active_residents.reverse();
            },
            |plan: &mut omega_regalloc::SpillRecoveryChoicePlan| {
                plan.choices[0].contenders.pop();
            },
            |plan: &mut omega_regalloc::SpillRecoveryChoicePlan| {
                plan.choices[0].selected_victim = VirtualRegisterId(4);
            },
            |plan: &mut omega_regalloc::SpillRecoveryChoicePlan| {
                plan.choices[0].reclaimed_view.0 += 1;
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                validate(&sources, changed),
                Err(omega_regalloc::SpillRecoveryChoiceError::NonCanonicalChoice)
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            validate(&sources, usage),
            Err(omega_regalloc::SpillRecoveryChoiceError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_and_every_representable_first_over_axis_are_typed_on_both_architectures() {
    let exact = OptimizationWorkBudget::new(5, 2, 10, 1, 1).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(4, 2, 10, 1, 1).unwrap(),
        OptimizationWorkBudget::new(5, 1, 10, 1, 1).unwrap(),
        OptimizationWorkBudget::new(5, 2, 9, 1, 1).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        assert!(choose(&sources, exact).is_ok());
        for budget in insufficient {
            assert!(matches!(
                choose(&sources, budget),
                Err(omega_regalloc::SpillRecoveryChoiceError::BudgetExceeded {
                    required: OptimizationWorkUsage {
                        rule_evaluations: 5,
                        candidates: 2,
                        validation_steps: 10,
                        commits: 1,
                        iterations: 1,
                    },
                    budget: actual,
                }) if actual == budget
            ));
        }
    }
}
