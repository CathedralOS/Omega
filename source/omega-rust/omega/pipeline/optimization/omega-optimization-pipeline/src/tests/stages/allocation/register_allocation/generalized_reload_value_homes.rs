//! Replayed epoch-zero home success and exact epoch-one recursive pressure.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;
use omega_selected_instructions::VirtualRegisterId;

use super::{
    reload_value_homes::ReloadSources, spill_recovery_actions::plan as plan_recovery,
    spill_recovery_worklist::pressure_sources,
};

pub(super) struct Sources {
    reloads: ReloadSources,
    recovery: omega_regalloc::ValidatedSpillRecoveryActions,
    generalized: omega_regalloc::ValidatedGeneralizedSpillInsertion,
}

impl Sources {
    pub(super) fn new(target: NativeTarget) -> Self {
        let reloads = pressure_sources(target);
        let recovery = plan_recovery(&reloads, selected_lowering_budget()).unwrap();
        let generalized = omega_regalloc::schedule_generalized_spill_insertion(
            reloads.insertion(),
            &recovery,
            omega_regalloc::GeneralizedSpillInsertionPolicy::EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
            selected_lowering_budget(),
        )
        .unwrap();
        Self {
            reloads,
            recovery,
            generalized,
        }
    }

    pub(super) fn assign(
        &self,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_regalloc::ValidatedGeneralizedReloadValueHomes,
        omega_regalloc::GeneralizedReloadValueHomeError,
    > {
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_regalloc::assign_generalized_reload_value_homes(
            &self.generalized,
            self.reloads.insertion(),
            &self.recovery,
            selected.selected(),
            ranges.ranges(),
            self.reloads.legality().legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            omega_regalloc::GeneralizedReloadValueHomePolicy::EpochZeroAndOneBlockLocalLowestCompatibleViewV1,
            budget,
        )
    }

    pub(super) fn choose_generalized_victim(
        &self,
        homes: &omega_regalloc::ValidatedGeneralizedReloadValueHomes,
        worklist: &omega_regalloc::ValidatedGeneralizedSpillRecoveryWorklist,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_regalloc::ValidatedGeneralizedSpillRecoveryChoices,
        omega_regalloc::GeneralizedSpillRecoveryChoiceError,
    > {
        let legality = self.reloads.legality();
        let environment = legality
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        omega_regalloc::choose_generalized_spill_recovery_victims(
            worklist,
            homes,
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            omega_regalloc::GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1,
            budget,
        )
    }

    pub(super) fn validate_generalized_victim(
        &self,
        homes: &omega_regalloc::ValidatedGeneralizedReloadValueHomes,
        worklist: &omega_regalloc::ValidatedGeneralizedSpillRecoveryWorklist,
        plan: omega_regalloc::GeneralizedSpillRecoveryChoicePlan,
    ) -> Result<
        omega_regalloc::ValidatedGeneralizedSpillRecoveryChoices,
        omega_regalloc::GeneralizedSpillRecoveryChoiceError,
    > {
        let legality = self.reloads.legality();
        let environment = legality
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        omega_regalloc::validate_generalized_spill_recovery_choices(
            worklist,
            homes,
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            plan,
        )
    }

    fn validate(
        &self,
        candidate: omega_regalloc::GeneralizedReloadValueHomePlan,
    ) -> Result<
        omega_regalloc::ValidatedGeneralizedReloadValueHomes,
        omega_regalloc::GeneralizedReloadValueHomeError,
    > {
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_regalloc::validate_generalized_reload_value_homes(
            &self.generalized,
            self.reloads.insertion(),
            &self.recovery,
            selected.selected(),
            ranges.ranges(),
            self.reloads.legality().legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            candidate,
        )
    }
}

#[test]
fn first_reload_gets_a_home_and_second_retains_exact_pressure_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = Sources::new(target);
        let first = sources.assign(selected_lowering_budget()).unwrap();
        let second = sources.assign(selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().assignment_count(), 1);
        assert_eq!(first.receipt().pressure_count(), 1);
        assert_eq!(first.receipt().retained_home_count(), 4);
        assert_eq!(first.plan().functions[0].outcomes.len(), 2);
        assert_eq!(first.receipt().usage(), exact_usage());

        let omega_regalloc::GeneralizedReloadValueHomeOutcome::Assigned(assigned) =
            &first.plan().functions[0].outcomes[0]
        else {
            panic!("epoch-zero reload must be assigned")
        };
        assert_eq!(assigned.result, action(0, 0));
        assert_eq!(assigned.start, LiveRangePoint(12));
        assert_eq!(assigned.exclusive_end, LiveRangePoint(17));
        assert!(assigned.candidates.contains(&assigned.view));
        assert_eq!(assigned.coexisting_homes.len(), 2);
        assert!(assigned.coexisting_homes.iter().any(|home| {
            home.value
                == omega_regalloc::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
        }));

        let omega_regalloc::GeneralizedReloadValueHomeOutcome::Pressure(pressure) =
            &first.plan().functions[0].outcomes[1]
        else {
            panic!("epoch-one reload must retain recursive pressure")
        };
        assert_eq!(pressure.result, action(1, 0));
        assert_eq!(pressure.start, LiveRangePoint(14));
        assert_eq!(pressure.exclusive_end, LiveRangePoint(15));
        assert_eq!(pressure.candidates.len(), 2);
        assert_eq!(pressure.blocking_homes.len(), 2);
        assert!(pressure.blocking_homes.iter().any(|home| {
            home.value == omega_regalloc::GeneralizedReloadCoexistingValue::Reload(action(0, 0))
        }));
        assert!(pressure.blocking_homes.iter().any(|home| {
            home.value
                == omega_regalloc::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
        }));
    }
}

#[test]
fn replay_rejects_root_assigned_pressure_roster_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = Sources::new(target);
        let canonical = sources
            .assign(selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();
        let mut root = canonical.clone();
        root.generalized_spill_insertion =
            omega_regalloc::GeneralizedSpillInsertionIdentity::from_bytes([0xa7; 32]);
        assert_eq!(
            sources.validate(root),
            Err(omega_regalloc::GeneralizedReloadValueHomeError::RootMismatch)
        );

        for corrupt in [
            |plan: &mut omega_regalloc::GeneralizedReloadValueHomePlan| {
                let omega_regalloc::GeneralizedReloadValueHomeOutcome::Assigned(row) =
                    &mut plan.functions[0].outcomes[0]
                else {
                    unreachable!()
                };
                row.view.0 += 1;
            },
            |plan: &mut omega_regalloc::GeneralizedReloadValueHomePlan| {
                let omega_regalloc::GeneralizedReloadValueHomeOutcome::Assigned(row) =
                    &mut plan.functions[0].outcomes[0]
                else {
                    unreachable!()
                };
                row.coexisting_homes.clear();
            },
            |plan: &mut omega_regalloc::GeneralizedReloadValueHomePlan| {
                let omega_regalloc::GeneralizedReloadValueHomeOutcome::Pressure(row) =
                    &mut plan.functions[0].outcomes[1]
                else {
                    unreachable!()
                };
                row.candidates.reverse();
            },
            |plan: &mut omega_regalloc::GeneralizedReloadValueHomePlan| {
                let omega_regalloc::GeneralizedReloadValueHomeOutcome::Pressure(row) =
                    &mut plan.functions[0].outcomes[1]
                else {
                    unreachable!()
                };
                row.blocking_homes.pop();
            },
            |plan: &mut omega_regalloc::GeneralizedReloadValueHomePlan| {
                plan.functions[0].outcomes.swap(0, 1);
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate(changed),
                Err(
                    omega_regalloc::GeneralizedReloadValueHomeError::NonCanonicalAssignments {
                        function: 0,
                    }
                )
            );
        }
        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate(usage),
            Err(omega_regalloc::GeneralizedReloadValueHomeError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_representable_first_over_axes_and_cross_target_roots_are_typed() {
    let exact = OptimizationWorkBudget::new(3, 4, 18, 1, 3).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(2, 4, 18, 1, 3).unwrap(),
        OptimizationWorkBudget::new(3, 3, 18, 1, 3).unwrap(),
        OptimizationWorkBudget::new(3, 4, 17, 1, 3).unwrap(),
        OptimizationWorkBudget::new(3, 4, 18, 1, 2).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = Sources::new(target);
        assert!(sources.assign(exact).is_ok());
        for budget in insufficient {
            assert!(matches!(
                sources.assign(budget),
                Err(omega_regalloc::GeneralizedReloadValueHomeError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == exact_usage() && actual == budget
            ));
        }
    }

    let x86 = Sources::new(NativeTarget::linux_x64());
    let plan = x86.assign(exact).unwrap().plan().clone();
    let arm = Sources::new(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate(plan),
        Err(omega_regalloc::GeneralizedReloadValueHomeError::RootMismatch)
    );
}

const fn action(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 3,
        candidates: 4,
        validation_steps: 18,
        commits: 1,
        iterations: 3,
    }
}
