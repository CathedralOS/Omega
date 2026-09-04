//! Closed epoch-zero/one slot lifetimes and canonical abstract insertion events.

use super::{
    reload_value_homes::ReloadSources, spill_recovery_actions::plan as plan_recovery,
    spill_recovery_worklist::pressure_sources,
};
use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;

fn schedule(
    sources: &ReloadSources,
    recovery: &omega_regalloc::ValidatedSpillRecoveryActions,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_regalloc::ValidatedGeneralizedSpillInsertion,
    omega_regalloc::GeneralizedSpillInsertionError,
> {
    omega_regalloc::schedule_generalized_spill_insertion(
        sources.insertion(),
        recovery,
        omega_regalloc::GeneralizedSpillInsertionPolicy::EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
        budget,
    )
}

fn validate(
    sources: &ReloadSources,
    recovery: &omega_regalloc::ValidatedSpillRecoveryActions,
    candidate: omega_regalloc::GeneralizedSpillInsertionPlan,
) -> Result<
    omega_regalloc::ValidatedGeneralizedSpillInsertion,
    omega_regalloc::GeneralizedSpillInsertionError,
> {
    omega_regalloc::validate_generalized_spill_insertion(sources.insertion(), recovery, candidate)
}

#[test]
fn both_spills_receive_closed_slots_and_one_ordered_target_neutral_schedule() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let recovery = plan_recovery(&sources, selected_lowering_budget()).unwrap();
        let first = schedule(&sources, &recovery, selected_lowering_budget()).unwrap();
        let second = schedule(&sources, &recovery, selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().action_count(), 2);
        assert_eq!(first.receipt().event_count(), 7);
        assert_eq!(first.receipt().max_spill_area_bytes(), 16);
        assert_eq!(
            first.receipt().usage(),
            OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 2,
                validation_steps: 10,
                commits: 2,
                iterations: 3,
            }
        );

        let function = &first.plan().functions[0];
        assert_eq!(function.spill_area_bytes, 16);
        assert_eq!(function.slots.len(), 2);
        assert_eq!(function.slots[0].action, action(0, 0));
        assert_eq!(function.slots[0].live_from, LiveRangePoint(9));
        assert_eq!(function.slots[0].live_through, LiveRangePoint(12));
        assert_eq!(function.slots[0].spill_area_offset, 0);
        assert_eq!(function.slots[1].action, action(1, 0));
        assert_eq!(function.slots[1].live_from, LiveRangePoint(12));
        assert_eq!(function.slots[1].live_through, LiveRangePoint(14));
        assert_eq!(function.slots[1].spill_area_offset, 8);

        assert_eq!(
            function
                .schedule
                .iter()
                .map(event_shape)
                .collect::<Vec<_>>(),
            vec![
                (9, 0, 0, 0, 4),
                (12, 0, 1, 0, 6),
                (12, 1, 0, 0, 6),
                (12, 2, 0, 0, 6),
                (14, 1, 1, 0, 7),
                (14, 2, 1, 0, 7),
                (16, 2, 0, 0, 8),
            ]
        );
        let omega_regalloc::GeneralizedSpillEvent::Store { before_reload, .. } =
            function.schedule[1]
        else {
            panic!("epoch-one store must be the point-12 store event")
        };
        assert_eq!(before_reload, Some(action(0, 0)));
    }
}

#[test]
fn independent_replay_rejects_root_slot_schedule_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let recovery = plan_recovery(&sources, selected_lowering_budget()).unwrap();
        let canonical = schedule(&sources, &recovery, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        let mut root = canonical.clone();
        root.spill_recovery_actions =
            omega_regalloc::SpillRecoveryActionIdentity::from_bytes([0x84; 32]);
        assert_eq!(
            validate(&sources, &recovery, root),
            Err(omega_regalloc::GeneralizedSpillInsertionError::RootMismatch)
        );

        let mut slot = canonical.clone();
        slot.functions[0].slots[1].spill_area_offset = 0;
        assert_eq!(
            validate(&sources, &recovery, slot),
            Err(omega_regalloc::GeneralizedSpillInsertionError::NonCanonicalSlots { function: 0 })
        );

        let mut schedule_row = canonical.clone();
        let omega_regalloc::GeneralizedSpillEvent::Store { before_reload, .. } =
            &mut schedule_row.functions[0].schedule[1]
        else {
            panic!("expected epoch-one store")
        };
        *before_reload = None;
        assert_eq!(
            validate(&sources, &recovery, schedule_row),
            Err(
                omega_regalloc::GeneralizedSpillInsertionError::NonCanonicalSchedule {
                    function: 0,
                }
            )
        );

        let mut usage = canonical;
        usage.usage.iterations += 1;
        assert_eq!(
            validate(&sources, &recovery, usage),
            Err(omega_regalloc::GeneralizedSpillInsertionError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_representable_first_over_axes_and_cross_target_roots_are_typed() {
    let exact = OptimizationWorkBudget::new(1, 2, 10, 2, 3).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(1, 1, 10, 2, 3).unwrap(),
        OptimizationWorkBudget::new(1, 2, 9, 2, 3).unwrap(),
        OptimizationWorkBudget::new(1, 2, 10, 1, 3).unwrap(),
        OptimizationWorkBudget::new(1, 2, 10, 2, 2).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let recovery = plan_recovery(&sources, selected_lowering_budget()).unwrap();
        assert!(schedule(&sources, &recovery, exact).is_ok());
        for budget in insufficient {
            assert!(matches!(
                schedule(&sources, &recovery, budget),
                Err(omega_regalloc::GeneralizedSpillInsertionError::BudgetExceeded {
                    required: OptimizationWorkUsage {
                        rule_evaluations: 1,
                        candidates: 2,
                        validation_steps: 10,
                        commits: 2,
                        iterations: 3,
                    },
                    budget: actual,
                }) if actual == budget
            ));
        }
    }

    let x86 = pressure_sources(NativeTarget::linux_x64());
    let x86_recovery = plan_recovery(&x86, selected_lowering_budget()).unwrap();
    let plan = schedule(&x86, &x86_recovery, exact).unwrap().plan().clone();
    let arm = pressure_sources(NativeTarget::linux_arm64());
    let arm_recovery = plan_recovery(&arm, selected_lowering_budget()).unwrap();
    assert_eq!(
        validate(&arm, &arm_recovery, plan),
        Err(omega_regalloc::GeneralizedSpillInsertionError::RootMismatch)
    );
}

const fn action(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
}

fn event_shape(event: &omega_regalloc::GeneralizedSpillEvent) -> (u32, u8, u32, u32, u32) {
    match *event {
        omega_regalloc::GeneralizedSpillEvent::Store {
            action,
            point,
            before_instruction,
            ..
        } => (
            point.0,
            0,
            action.epoch,
            action.ordinal,
            before_instruction.0,
        ),
        omega_regalloc::GeneralizedSpillEvent::Reload {
            action,
            point,
            before_instruction,
            ..
        } => (
            point.0,
            1,
            action.epoch,
            action.ordinal,
            before_instruction.0,
        ),
        omega_regalloc::GeneralizedSpillEvent::Rewrite {
            action,
            point,
            instruction,
            ..
        } => (point.0, 2, action.epoch, action.ordinal, instruction.0),
    }
}
