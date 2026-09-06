//! Logical reload-value reanalysis and bounded physical-view assignment.

use crate::tests::*;

#[test]
fn reload_value_gets_a_deterministic_home_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = ReloadSources::new(target);
        let first = sources.assign(selected_lowering_budget()).unwrap();
        let second = sources.assign(selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().assignment_count(), 1);
        assert!(first.receipt().coexisting_home_count() > 0);
        assert_eq!(
            first.receipt().legality(),
            sources.legality.legality().receipt().identity()
        );
        assert_eq!(
            first.receipt().ranges(),
            sources
                .legality
                .live_range_stage()
                .ranges()
                .receipt()
                .identity()
        );

        let action = sources.insertion.plan().functions[0]
            .action
            .as_ref()
            .unwrap();
        let assignment = first.plan().functions[0].assignment.as_ref().unwrap();
        assert_eq!(assignment.result, action.reload.result);
        assert_eq!(assignment.start, action.rewrites[0].point);
        assert_eq!(
            assignment.exclusive_end,
            LiveRangePoint(action.rewrites[1].point.0 + 1)
        );
        assert_eq!(assignment.class, action.reload.destination_class);
        assert!(assignment.candidates.contains(&assignment.view));
        assert!(
            assignment
                .coexisting_homes
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert!(
            !assignment
                .coexisting_homes
                .iter()
                .any(|home| home.virtual_register == action.victim)
        );
    }
}

#[test]
fn independent_replay_rejects_root_assignment_domain_roster_and_usage_corruption() {
    let sources = ReloadSources::new(NativeTarget::linux_x64());
    let assigned = sources.assign(selected_lowering_budget()).unwrap();
    let canonical = assigned.plan().clone();

    let mut root = canonical.clone();
    root.abstract_spill_insertion =
        selected_instructions_to_register_homes::AbstractSpillInsertionIdentity::from_bytes(
            [0x91; 32],
        );
    assert_eq!(
        sources.validate(root),
        Err(selected_instructions_to_register_homes::ReloadValueHomeError::RootMismatch)
    );

    for corrupt in [
        |plan: &mut selected_instructions_to_register_homes::ReloadValueHomePlan| {
            plan.functions[0].assignment.as_mut().unwrap().view.0 += 1;
        },
        |plan: &mut selected_instructions_to_register_homes::ReloadValueHomePlan| {
            plan.functions[0]
                .assignment
                .as_mut()
                .unwrap()
                .candidates
                .reverse();
        },
        |plan: &mut selected_instructions_to_register_homes::ReloadValueHomePlan| {
            let assignment = plan.functions[0].assignment.as_mut().unwrap();
            let unused = assignment
                .candidates
                .iter()
                .position(|candidate| *candidate != assignment.view)
                .unwrap();
            assignment.candidates.remove(unused);
        },
        |plan: &mut selected_instructions_to_register_homes::ReloadValueHomePlan| {
            plan.functions[0]
                .assignment
                .as_mut()
                .unwrap()
                .coexisting_homes
                .clear();
        },
    ] {
        let mut changed = canonical.clone();
        corrupt(&mut changed);
        assert_eq!(
            sources.validate(changed),
            Err(selected_instructions_to_register_homes::ReloadValueHomeError::NonCanonicalAssignment { function: 0 })
        );
    }

    let mut usage = canonical;
    usage.usage.validation_steps += 1;
    assert_eq!(
        sources.validate(usage),
        Err(selected_instructions_to_register_homes::ReloadValueHomeError::UsageMismatch)
    );
}

#[test]
fn budget_is_exact_and_empty_pressure_has_no_reload_assignment() {
    let sources = ReloadSources::new(NativeTarget::linux_x64());
    let assigned = sources.assign(selected_lowering_budget()).unwrap();
    let usage = assigned.plan().usage;
    assert!(matches!(
        sources.assign(
            OptimizationWorkBudget::new(
                usage.rule_evaluations,
                usage.candidates,
                usage.validation_steps - 1,
                usage.commits,
                usage.iterations,
            )
            .unwrap()
        ),
        Err(selected_instructions_to_register_homes::ReloadValueHomeError::BudgetExceeded { .. })
    ));

    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let empty = ReloadSources::from_legality(legality);
    let assigned = empty.assign(selected_lowering_budget()).unwrap();
    assert!(
        assigned
            .plan()
            .functions
            .iter()
            .all(|function| function.assignment.is_none())
    );
    assert_eq!(assigned.receipt().assignment_count(), 0);
    assert_eq!(assigned.receipt().coexisting_home_count(), 0);
}

#[test]
fn bridge_chain_reaches_exact_reload_pressure_through_public_validation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = ReloadSources::from_legality(
            staged_active_resident_bridge_chain_two_view_legality(target),
        );
        let action = sources.insertion.plan().functions[0]
            .action
            .as_ref()
            .expect("the public two-view chain must retain one spill action");
        assert_eq!(action.pressure_point, LiveRangePoint(9));
        assert_eq!(action.incoming, VirtualRegisterId(3));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.store.before_instruction, SelectedInstructionId(4));
        assert_eq!(action.reload.before_instruction, SelectedInstructionId(6));
        assert_eq!(
            action
                .rewrites
                .iter()
                .map(|rewrite| (rewrite.point, rewrite.instruction, rewrite.operand))
                .collect::<Vec<_>>(),
            vec![
                (LiveRangePoint(12), SelectedInstructionId(6), 0),
                (LiveRangePoint(16), SelectedInstructionId(8), 0),
            ]
        );
        assert_eq!(
            sources.assign(selected_lowering_budget()),
            Err(
                selected_instructions_to_register_homes::ReloadValueHomeError::ReloadPressure {
                    function: 0,
                    result: 0,
                }
            )
        );
    }
}

pub(super) struct ReloadSources {
    legality: StagedOptimizedAllocationLegality,
    logical: selected_instructions_to_register_homes::ValidatedLogicalSpillOperations,
    insertion: selected_instructions_to_register_homes::ValidatedAbstractSpillInsertion,
}

impl ReloadSources {
    pub(super) fn new(target: NativeTarget) -> Self {
        Self::from_legality(staged_active_resident_two_view_legality(target))
    }

    pub(super) fn from_legality(legality: StagedOptimizedAllocationLegality) -> Self {
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        let choices = choose_spill_victims(
            legality.legality(),
            ranges.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let logical = selected_instructions_to_register_homes::plan_logical_spill_operations(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let slots = selected_instructions_to_register_homes::color_logical_spill_stack_slots(
            &logical,
            selected_instructions_to_register_homes::StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let insertion = selected_instructions_to_register_homes::schedule_abstract_spill_insertion(
            &logical,
            &slots,
            selected_instructions_to_register_homes::AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
            selected_lowering_budget(),
        )
        .unwrap();
        Self {
            legality,
            logical,
            insertion,
        }
    }

    pub(super) fn assign(
        &self,
        budget: OptimizationWorkBudget,
    ) -> Result<
        selected_instructions_to_register_homes::ValidatedReloadValueHomes,
        selected_instructions_to_register_homes::ReloadValueHomeError,
    > {
        let ranges = self.legality.live_range_stage();
        let environment = ranges
            .liveness_stage()
            .selected_stage()
            .register_environment();
        selected_instructions_to_register_homes::assign_reload_value_homes(
            &self.insertion,
            &self.logical,
            self.legality.legality(),
            ranges.ranges(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            selected_instructions_to_register_homes::ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1,
            budget,
        )
    }

    pub(super) const fn insertion(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedAbstractSpillInsertion {
        &self.insertion
    }

    pub(super) const fn logical(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedLogicalSpillOperations {
        &self.logical
    }

    pub(super) const fn legality(&self) -> &StagedOptimizedAllocationLegality {
        &self.legality
    }

    fn validate(
        &self,
        plan: selected_instructions_to_register_homes::ReloadValueHomePlan,
    ) -> Result<
        selected_instructions_to_register_homes::ValidatedReloadValueHomes,
        selected_instructions_to_register_homes::ReloadValueHomeError,
    > {
        let ranges = self.legality.live_range_stage();
        let environment = ranges
            .liveness_stage()
            .selected_stage()
            .register_environment();
        selected_instructions_to_register_homes::validate_reload_value_homes(
            &self.insertion,
            &self.logical,
            self.legality.legality(),
            ranges.ranges(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            plan,
        )
    }
}
