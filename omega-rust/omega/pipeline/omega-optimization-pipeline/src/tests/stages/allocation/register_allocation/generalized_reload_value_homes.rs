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
    recovery: omega_selected_instructions_to_register_homes::ValidatedSpillRecoveryActions,
    generalized: omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillInsertion,
}

impl Sources {
    pub(super) fn new(target: NativeTarget) -> Self {
        Self::from_reload_sources(pressure_sources(target))
    }

    pub(super) fn from_legality(legality: StagedOptimizedAllocationLegality) -> Self {
        Self::from_reload_sources(ReloadSources::from_legality(legality))
    }

    fn from_reload_sources(reloads: ReloadSources) -> Self {
        let recovery = plan_recovery(&reloads, selected_lowering_budget()).unwrap();
        let generalized = omega_selected_instructions_to_register_homes::schedule_generalized_spill_insertion(
            reloads.insertion(),
            &recovery,
            omega_selected_instructions_to_register_homes::GeneralizedSpillInsertionPolicy::EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
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
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeError,
    > {
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_selected_instructions_to_register_homes::assign_generalized_reload_value_homes(
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
            omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomePolicy::EpochZeroAndOneBlockLocalLowestCompatibleViewV1,
            budget,
        )
    }

    pub(super) fn choose_generalized_victim(
        &self,
        homes: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        worklist: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryWorklist,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryChoices,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError,
    > {
        self.choose_generalized_victim_with_policy(
            homes,
            worklist,
            omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1,
            budget,
        )
    }

    pub(super) fn choose_generalized_victim_with_policy(
        &self,
        homes: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        worklist: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryWorklist,
        policy: omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryChoices,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError,
    > {
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_selected_instructions_to_register_homes::choose_generalized_spill_recovery_victims(
            worklist,
            homes,
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            policy,
            budget,
        )
    }

    pub(super) fn validate_generalized_victim(
        &self,
        homes: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        worklist: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryWorklist,
        plan: omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePlan,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryChoices,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoiceError,
    > {
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_selected_instructions_to_register_homes::validate_generalized_spill_recovery_choices(
            worklist,
            homes,
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            plan,
        )
    }

    pub(super) fn plan_generalized_recovery_actions(
        &self,
        homes: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        choices: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryChoices,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionError,
    > {
        omega_selected_instructions_to_register_homes::plan_generalized_spill_recovery_actions(
            &self.generalized,
            homes,
            choices,
            omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionPolicy::EpochTwoReloadVictimLaterGeneralizedRewritesV1,
            budget,
        )
    }

    pub(super) fn validate_generalized_recovery_actions(
        &self,
        homes: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        choices: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryChoices,
        plan: omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionPlan,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionError,
    > {
        omega_selected_instructions_to_register_homes::validate_generalized_spill_recovery_actions(
            &self.generalized,
            homes,
            choices,
            plan,
        )
    }

    pub(super) fn plan_original_recovery_actions(
        &self,
        homes: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        choices: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryChoices,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionError,
    > {
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        omega_selected_instructions_to_register_homes::plan_generalized_original_spill_recovery_actions(
            &self.generalized,
            homes,
            choices,
            selected.selected(),
            ranges.ranges(),
            budget,
        )
    }

    pub(super) fn validate_original_recovery_actions(
        &self,
        homes: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        choices: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryChoices,
        plan: omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionPlan,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        omega_selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionError,
    > {
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        omega_selected_instructions_to_register_homes::validate_generalized_original_spill_recovery_actions(
            &self.generalized,
            homes,
            choices,
            selected.selected(),
            ranges.ranges(),
            plan,
        )
    }

    pub(super) fn schedule_recursive_spills(
        &self,
        recovery: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedRecursiveSpillInsertion,
        omega_selected_instructions_to_register_homes::RecursiveSpillInsertionError,
    > {
        omega_selected_instructions_to_register_homes::schedule_recursive_spill_insertion(
            &self.generalized,
            recovery,
            omega_selected_instructions_to_register_homes::RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
            budget,
        )
    }

    pub(super) fn schedule_original_recursive_spills(
        &self,
        recovery: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedRecursiveSpillInsertion,
        omega_selected_instructions_to_register_homes::RecursiveSpillInsertionError,
    > {
        omega_selected_instructions_to_register_homes::schedule_recursive_spill_insertion(
            &self.generalized,
            recovery,
            omega_selected_instructions_to_register_homes::RecursiveSpillInsertionPolicy::EpochTwoOriginalVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV2,
            budget,
        )
    }

    pub(super) fn validate_recursive_spills(
        &self,
        recovery: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        plan: omega_selected_instructions_to_register_homes::RecursiveSpillInsertionPlan,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedRecursiveSpillInsertion,
        omega_selected_instructions_to_register_homes::RecursiveSpillInsertionError,
    > {
        omega_selected_instructions_to_register_homes::validate_recursive_spill_insertion(
            &self.generalized,
            recovery,
            plan,
        )
    }

    pub(super) fn assign_recursive_reload_homes(
        &self,
        recursive: &omega_selected_instructions_to_register_homes::ValidatedRecursiveSpillInsertion,
        recovery: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        prior: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        budget: OptimizationWorkBudget,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedRecursiveReloadValueHomes,
        omega_selected_instructions_to_register_homes::RecursiveReloadValueHomeError,
    > {
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_selected_instructions_to_register_homes::assign_recursive_reload_value_homes(
            recursive,
            recovery,
            prior,
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            omega_selected_instructions_to_register_homes::RecursiveReloadValueHomePolicy::CompleteBlockLocalLowestCompatibleViewV1,
            budget,
        )
    }

    pub(super) fn validate_recursive_reload_homes(
        &self,
        recursive: &omega_selected_instructions_to_register_homes::ValidatedRecursiveSpillInsertion,
        recovery: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
        prior: &omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        plan: omega_selected_instructions_to_register_homes::RecursiveReloadValueHomePlan,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedRecursiveReloadValueHomes,
        omega_selected_instructions_to_register_homes::RecursiveReloadValueHomeError,
    > {
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_selected_instructions_to_register_homes::validate_recursive_reload_value_homes(
            recursive,
            recovery,
            prior,
            selected.selected(),
            ranges.ranges(),
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
        candidate: omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomePlan,
    ) -> Result<
        omega_selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
        omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeError,
    > {
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        omega_selected_instructions_to_register_homes::validate_generalized_reload_value_homes(
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

        let omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeOutcome::Assigned(assigned) =
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
                == omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
        }));

        let omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeOutcome::Pressure(pressure) =
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
            home.value == omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Reload(action(0, 0))
        }));
        assert!(pressure.blocking_homes.iter().any(|home| {
            home.value
                == omega_selected_instructions_to_register_homes::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(5))
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
            omega_selected_instructions_to_register_homes::GeneralizedSpillInsertionIdentity::from_bytes([0xa7; 32]);
        assert_eq!(
            sources.validate(root),
            Err(omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeError::RootMismatch)
        );

        for corrupt in [
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomePlan| {
                let omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeOutcome::Assigned(row) =
                    &mut plan.functions[0].outcomes[0]
                else {
                    unreachable!()
                };
                row.view.0 += 1;
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomePlan| {
                let omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeOutcome::Assigned(row) =
                    &mut plan.functions[0].outcomes[0]
                else {
                    unreachable!()
                };
                row.coexisting_homes.clear();
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomePlan| {
                let omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeOutcome::Pressure(row) =
                    &mut plan.functions[0].outcomes[1]
                else {
                    unreachable!()
                };
                row.candidates.reverse();
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomePlan| {
                let omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeOutcome::Pressure(row) =
                    &mut plan.functions[0].outcomes[1]
                else {
                    unreachable!()
                };
                row.blocking_homes.pop();
            },
            |plan: &mut omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomePlan| {
                plan.functions[0].outcomes.swap(0, 1);
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate(changed),
                Err(
                    omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeError::NonCanonicalAssignments {
                        function: 0,
                    }
                )
            );
        }
        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate(usage),
            Err(omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeError::UsageMismatch)
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
                Err(omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeError::BudgetExceeded {
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
        Err(omega_selected_instructions_to_register_homes::GeneralizedReloadValueHomeError::RootMismatch)
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
        rule_evaluations: 3,
        candidates: 4,
        validation_steps: 18,
        commits: 1,
        iterations: 3,
    }
}
