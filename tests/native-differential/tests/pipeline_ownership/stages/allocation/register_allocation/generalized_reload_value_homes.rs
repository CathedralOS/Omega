//! Replayed epoch-zero home success and exact epoch-one recursive pressure.
use crate::tests::{
    LiveRangePoint, NativeTarget, OptimizationWorkBudget, StagedOptimizedAllocationLegality,
    selected_lowering_budget,
};
use optimization_core::OptimizationWorkUsage;
use selected_instructions::VirtualRegisterId;

use super::{
    reload_value_homes::ReloadSources, spill_recovery_actions::plan as plan_recovery,
    spill_recovery_worklist::pressure_sources,
};

pub(super) struct Sources {
    reloads: ReloadSources,
    recovery: selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedSpillRecoveryActions,
    generalized: selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillInsertion,
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
        let generalized = selected_instructions_to_register_homes::unsequenced_spill_stages::schedule_generalized_spill_insertion(
            reloads.insertion(),
            &recovery,
            selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillInsertionPolicy::EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
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
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeError,
    >{
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::assign_generalized_reload_value_homes(
            &self.generalized,
            self.reloads.insertion(),
            &self.recovery,
            selected.selected(),
            ranges.ranges(),
            self.reloads.legality().legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomePolicy::EpochZeroAndOneBlockLocalLowestCompatibleViewV1,
            budget,
        )
    }

    pub(super) fn choose_generalized_victim(
        &self,
        homes: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        worklist: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryWorklist,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryChoices,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryChoiceError,
    >{
        self.choose_generalized_victim_with_policy(
            homes,
            worklist,
            selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1,
            budget,
        )
    }

    pub(super) fn choose_generalized_victim_with_policy(
        &self,
        homes: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        worklist: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryWorklist,
        policy: selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryChoicePolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryChoices,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryChoiceError,
    >{
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::choose_generalized_spill_recovery_victims(
            worklist,
            homes,
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            policy,
            budget,
        )
    }

    pub(super) fn validate_generalized_victim(
        &self,
        homes: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        worklist: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryWorklist,
        plan: selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryChoicePlan,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryChoices,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryChoiceError,
    >{
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::validate_generalized_spill_recovery_choices(
            worklist,
            homes,
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            plan,
        )
    }

    pub(super) fn plan_generalized_recovery_actions(
        &self,
        homes: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        choices: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryChoices,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryActionError,
    >{
        selected_instructions_to_register_homes::unsequenced_spill_stages::plan_generalized_spill_recovery_actions(
            &self.generalized,
            homes,
            choices,
            selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryActionPolicy::EpochTwoReloadVictimLaterGeneralizedRewritesV1,
            budget,
        )
    }

    pub(super) fn validate_generalized_recovery_actions(
        &self,
        homes: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        choices: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryChoices,
        plan: selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryActionPlan,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryActionError,
    >{
        selected_instructions_to_register_homes::unsequenced_spill_stages::validate_generalized_spill_recovery_actions(
            &self.generalized,
            homes,
            choices,
            plan,
        )
    }

    pub(super) fn plan_original_recovery_actions(
        &self,
        homes: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        choices: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryChoices,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryActionError,
    >{
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        selected_instructions_to_register_homes::unsequenced_spill_stages::plan_generalized_original_spill_recovery_actions(
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
        homes: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        choices: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryChoices,
        plan: selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryActionPlan,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillRecoveryActionError,
    >{
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        selected_instructions_to_register_homes::unsequenced_spill_stages::validate_generalized_original_spill_recovery_actions(
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
        recovery: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedRecursiveSpillInsertion,
        selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveSpillInsertionError,
    >{
        selected_instructions_to_register_homes::unsequenced_spill_stages::schedule_recursive_spill_insertion(
            &self.generalized,
            recovery,
            selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
            budget,
        )
    }

    pub(super) fn schedule_original_recursive_spills(
        &self,
        recovery: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedRecursiveSpillInsertion,
        selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveSpillInsertionError,
    >{
        selected_instructions_to_register_homes::unsequenced_spill_stages::schedule_recursive_spill_insertion(
            &self.generalized,
            recovery,
            selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveSpillInsertionPolicy::EpochTwoOriginalVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV2,
            budget,
        )
    }

    pub(super) fn validate_recursive_spills(
        &self,
        recovery: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        plan: selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveSpillInsertionPlan,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedRecursiveSpillInsertion,
        selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveSpillInsertionError,
    >{
        selected_instructions_to_register_homes::unsequenced_spill_stages::validate_recursive_spill_insertion(
            &self.generalized,
            recovery,
            plan,
        )
    }

    pub(super) fn assign_recursive_reload_homes(
        &self,
        recursive: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedRecursiveSpillInsertion,
        recovery: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        prior: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedRecursiveReloadValueHomes,
        selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveReloadValueHomeError,
    >{
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::assign_recursive_reload_value_homes(
            recursive,
            recovery,
            prior,
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveReloadValueHomePolicy::CompleteBlockLocalLowestCompatibleViewV1,
            budget,
        )
    }

    pub(super) fn validate_recursive_reload_homes(
        &self,
        recursive: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedRecursiveSpillInsertion,
        recovery: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedSpillRecoveryActions,
        prior: &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        plan: selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveReloadValueHomePlan,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedRecursiveReloadValueHomes,
        selected_instructions_to_register_homes::unsequenced_spill_stages::RecursiveReloadValueHomeError,
    >{
        let legality = self.reloads.legality();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::validate_recursive_reload_value_homes(
            recursive,
            recovery,
            prior,
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            plan,
        )
    }

    fn validate(
        &self,
        candidate: selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomePlan,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedGeneralizedReloadValueHomes,
        selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeError,
    >{
        let ranges = self.reloads.legality().live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::validate_generalized_reload_value_homes(
            &self.generalized,
            self.reloads.insertion(),
            &self.recovery,
            selected.selected(),
            ranges.ranges(),
            self.reloads.legality().legality(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
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

        let selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeOutcome::Assigned(
            assigned,
        ) = &first.plan().functions[0].outcomes[0]
        else {
            panic!("epoch-zero reload must be assigned")
        };
        assert_eq!(assigned.result, action(0, 0));
        assert_eq!(assigned.start, LiveRangePoint(14));
        assert_eq!(assigned.exclusive_end, LiveRangePoint(19));
        assert!(assigned.candidates.contains(&assigned.view));
        assert_eq!(assigned.coexisting_homes.len(), 2);
        assert!(assigned.coexisting_homes.iter().any(|home| {
            home.value
                == selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(6))
        }));

        let selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeOutcome::Pressure(
            pressure,
        ) = &first.plan().functions[0].outcomes[1]
        else {
            panic!("epoch-one reload must retain recursive pressure")
        };
        assert_eq!(pressure.result, action(1, 0));
        assert_eq!(pressure.start, LiveRangePoint(16));
        assert_eq!(pressure.exclusive_end, LiveRangePoint(17));
        assert_eq!(pressure.candidates.len(), 2);
        assert_eq!(pressure.blocking_homes.len(), 2);
        assert!(pressure.blocking_homes.iter().any(|home| {
            home.value == selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadCoexistingValue::Reload(action(0, 0))
        }));
        assert!(pressure.blocking_homes.iter().any(|home| {
            home.value
                == selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(6))
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
            selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillInsertionIdentity::from_bytes(
                [0xa7; 32],
            );
        assert_eq!(
            sources.validate(root),
            Err(selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeError::RootMismatch)
        );

        for corrupt in [
            |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomePlan| {
                let selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeOutcome::Assigned(row) =
                    &mut plan.functions[0].outcomes[0]
                else {
                    unreachable!()
                };
                row.view.0 += 1;
            },
            |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomePlan| {
                let selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeOutcome::Assigned(row) =
                    &mut plan.functions[0].outcomes[0]
                else {
                    unreachable!()
                };
                row.coexisting_homes.clear();
            },
            |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomePlan| {
                let selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeOutcome::Pressure(row) =
                    &mut plan.functions[0].outcomes[1]
                else {
                    unreachable!()
                };
                row.candidates.reverse();
            },
            |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomePlan| {
                let selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeOutcome::Pressure(row) =
                    &mut plan.functions[0].outcomes[1]
                else {
                    unreachable!()
                };
                row.blocking_homes.pop();
            },
            |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomePlan| {
                plan.functions[0].outcomes.swap(0, 1);
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                sources.validate(changed),
                Err(
                    selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeError::NonCanonicalAssignments {
                        function: 0,
                    }
                )
            );
        }
        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate(usage),
            Err(selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeError::UsageMismatch)
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
                Err(selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeError::BudgetExceeded {
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
        Err(selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedReloadValueHomeError::RootMismatch)
    );
}

const fn action(
    epoch: u32,
    ordinal: u32,
) -> selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillActionId {
    selected_instructions_to_register_homes::unsequenced_spill_stages::GeneralizedSpillActionId {
        epoch,
        ordinal,
    }
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
