//! Bounded recursive spill-recovery custody at the exact reload-pressure boundary.

use crate::tests::*;
use optimization_core::OptimizationWorkUsage;
use selected_instructions::SelectedBlockId;

use super::reload_value_homes::ReloadSources;

pub(super) fn pressure_sources(target: NativeTarget) -> ReloadSources {
    ReloadSources::from_legality(staged_active_resident_bridge_chain_two_view_legality(
        target,
    ))
}

pub(super) fn seed(
    sources: &ReloadSources,
    budget: OptimizationWorkBudget,
) -> Result<
    selected_instructions_to_register_homes::ValidatedSpillRecoveryWorklist,
    selected_instructions_to_register_homes::SpillRecoveryWorklistError,
> {
    let ranges = sources.legality().live_range_stage();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    selected_instructions_to_register_homes::seed_spill_recovery_worklist(
        sources.insertion(),
        sources.logical(),
        sources.legality().legality(),
        ranges.ranges(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        selected_instructions_to_register_homes::ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1,
        selected_lowering_budget(),
        selected_instructions_to_register_homes::SpillRecoveryWorklistPolicy::SingleReloadPressureEpochOneV1,
        budget,
    )
}

fn validate(
    sources: &ReloadSources,
    plan: selected_instructions_to_register_homes::SpillRecoveryWorklistPlan,
) -> Result<
    selected_instructions_to_register_homes::ValidatedSpillRecoveryWorklist,
    selected_instructions_to_register_homes::SpillRecoveryWorklistError,
> {
    let ranges = sources.legality().live_range_stage();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    selected_instructions_to_register_homes::validate_spill_recovery_worklist(
        sources.insertion(),
        sources.logical(),
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
fn reload_pressure_seeds_one_deterministic_epoch_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let first = seed(&sources, selected_lowering_budget()).unwrap();
        let second = seed(&sources, selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().epoch_count(), 1);
        assert_eq!(first.receipt().work_item_count(), 1);
        assert_eq!(first.receipt().usage(), first.plan().usage);
        assert_eq!(
            first.receipt().optimization_unit(),
            first.plan().optimization_unit
        );
        assert_eq!(first.receipt().fuel_schedule(), first.plan().fuel_schedule);

        let epoch = &first.plan().epochs[0];
        let item = &epoch.work_items[0];
        assert_eq!(epoch.epoch, 1);
        assert_eq!(item.synthetic.epoch, 1);
        assert_eq!(item.synthetic.ordinal, 0);
        assert_eq!(item.source_reload.0, 0);
        assert_eq!(item.block, SelectedBlockId(1));
        assert_eq!(item.start, LiveRangePoint(12));
        assert_eq!(item.exclusive_end, LiveRangePoint(17));
        assert_eq!(item.candidates.len(), 2);
        assert!(item.candidates.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            first.plan().usage,
            OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 2,
                validation_steps: 15,
                commits: 1,
                iterations: 1,
            }
        );
    }
}

#[test]
fn independent_replay_rejects_identity_worklist_and_usage_corruption_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let canonical = seed(&sources, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        let mut root = canonical.clone();
        root.abstract_spill_insertion =
            selected_instructions_to_register_homes::AbstractSpillInsertionIdentity::from_bytes(
                [0x7d; 32],
            );
        assert_eq!(
            validate(&sources, root),
            Err(selected_instructions_to_register_homes::SpillRecoveryWorklistError::RootMismatch)
        );

        for corrupt in [
            |plan: &mut selected_instructions_to_register_homes::SpillRecoveryWorklistPlan| {
                plan.epochs[0].epoch = 2;
            },
            |plan: &mut selected_instructions_to_register_homes::SpillRecoveryWorklistPlan| {
                plan.epochs[0].work_items[0].synthetic.ordinal = 1;
            },
            |plan: &mut selected_instructions_to_register_homes::SpillRecoveryWorklistPlan| {
                plan.epochs[0].work_items[0].start.0 += 1;
            },
            |plan: &mut selected_instructions_to_register_homes::SpillRecoveryWorklistPlan| {
                plan.epochs[0].work_items[0].candidates.reverse();
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                validate(&sources, changed),
                Err(selected_instructions_to_register_homes::SpillRecoveryWorklistError::NonCanonicalWorklist)
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            validate(&sources, usage),
            Err(selected_instructions_to_register_homes::SpillRecoveryWorklistError::UsageMismatch)
        );
    }
}

#[test]
fn worklist_budget_is_independent_exact_and_first_over_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let exact = OptimizationWorkBudget::new(1, 2, 15, 1, 1).unwrap();
        assert!(seed(&sources, exact).is_ok());
        let first_over = OptimizationWorkBudget::new(1, 2, 14, 1, 1).unwrap();
        assert!(matches!(
            seed(&sources, first_over),
            Err(selected_instructions_to_register_homes::SpillRecoveryWorklistError::BudgetExceeded {
                required: OptimizationWorkUsage {
                    validation_steps: 15,
                    ..
                },
                budget,
            }) if budget == first_over
        ));
    }
}

#[test]
fn successful_reload_assignment_cannot_seed_recursive_recovery() {
    let sources = ReloadSources::new(NativeTarget::linux_x64());
    assert_eq!(
        seed(&sources, selected_lowering_budget()),
        Err(selected_instructions_to_register_homes::SpillRecoveryWorklistError::ReloadPressureRequired)
    );
}
