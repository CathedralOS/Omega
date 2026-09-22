//! Logical reload-value reanalysis and bounded physical-view assignment.

use crate::tests::{
    LiveRangePoint, NativeTarget, OptimizationWorkBudget, RegisterViewId, SelectedInstructionId,
    SelectedInstructionKind, SpillChoicePolicy, StagedOptimizedAllocationLegality,
    VirtualRegisterId, call_spanning_reload_allowlist, call_spanning_reload_caller,
    call_spanning_reload_surviving_view, choose_spill_victims, selected_lowering_budget,
    stage_optimized_allocation_legality, stage_optimized_live_ranges, stage_optimized_liveness,
    staged_active_resident_bridge_chain_two_view_legality,
    staged_active_resident_two_view_legality, staged_call_spanning_reload_legality,
    staged_exact_add_conditional,
};
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
        selected_instructions_to_register_homes::unsequenced_spill_stages::AbstractSpillInsertionIdentity::from_bytes(
            [0x91; 32],
        );
    assert_eq!(
        sources.validate(root),
        Err(selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::RootMismatch)
    );

    for corrupt in [
        |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomePlan| {
            plan.functions[0].assignment.as_mut().unwrap().view.0 += 1;
        },
        |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomePlan| {
            plan.functions[0]
                .assignment
                .as_mut()
                .unwrap()
                .candidates
                .reverse();
        },
        |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomePlan| {
            let assignment = plan.functions[0].assignment.as_mut().unwrap();
            let unused = assignment
                .candidates
                .iter()
                .position(|candidate| *candidate != assignment.view)
                .unwrap();
            assignment.candidates.remove(unused);
        },
        |plan: &mut selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomePlan| {
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
            Err(selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::NonCanonicalAssignment { function: 0 })
        );
    }

    let mut usage = canonical;
    usage.usage.validation_steps += 1;
    assert_eq!(
        sources.validate(usage),
        Err(selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::UsageMismatch)
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
        Err(selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::BudgetExceeded { .. })
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
        assert_eq!(action.pressure_point, LiveRangePoint(11));
        assert_eq!(action.incoming, VirtualRegisterId(4));
        assert_eq!(action.victim, VirtualRegisterId(2));
        assert_eq!(action.store.before_instruction, SelectedInstructionId(5));
        assert_eq!(action.reload.before_instruction, SelectedInstructionId(7));
        assert_eq!(
            action
                .rewrites
                .iter()
                .map(|rewrite| (rewrite.point, rewrite.instruction, rewrite.operand))
                .collect::<Vec<_>>(),
            vec![
                (LiveRangePoint(14), SelectedInstructionId(7), 0),
                (LiveRangePoint(18), SelectedInstructionId(9), 0),
            ]
        );
        assert_eq!(
            sources.assign(selected_lowering_budget()),
            Err(
                selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::ReloadPressure {
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
    insertion: selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedAbstractSpillInsertion,
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
        let insertion = selected_instructions_to_register_homes::unsequenced_spill_stages::schedule_abstract_spill_insertion(
            &logical,
            &slots,
            selected_instructions_to_register_homes::unsequenced_spill_stages::AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
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
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedReloadValueHomes,
        selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError,
    >{
        let ranges = self.legality.live_range_stage();
        let environment = ranges
            .liveness_stage()
            .selected_stage()
            .register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::assign_reload_value_homes(
            &self.insertion,
            &self.logical,
            self.legality.legality(),
            ranges.ranges(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1,
            budget,
        )
    }

    pub(super) const fn insertion(
        &self,
    ) -> &selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedAbstractSpillInsertion{
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
        plan: selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomePlan,
    ) -> Result<
        selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedReloadValueHomes,
        selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError,
    >{
        let ranges = self.legality.live_range_stage();
        let environment = ranges
            .liveness_stage()
            .selected_stage()
            .register_environment();
        selected_instructions_to_register_homes::unsequenced_spill_stages::validate_reload_value_homes(
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

#[test]
fn call_spanning_reload_interval_selects_the_callee_saved_home_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let sources = ReloadSources::from_legality(staged_call_spanning_reload_legality(target));
        let ranges = sources.legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        let model = environment.physical().model();
        let caller = call_spanning_reload_caller();
        let saved = model
            .view_named(call_spanning_reload_surviving_view(target))
            .unwrap()
            .id;

        let insertion = sources
            .insertion
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .expect("the caller must be scheduled");
        let action = insertion
            .action
            .as_ref()
            .expect("the fixture must retain one spill action");
        let first = action.rewrites.first().unwrap();
        let last = action.rewrites.last().unwrap();
        let block = selected
            .selected()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == first.block)
            .unwrap();
        // The rewritten uses of the spilled victim straddle a `CallUnit`, so
        // the reload interval covers that call's clobber point.
        let call = block
            .instructions
            .iter()
            .find(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
                    && first.instruction < instruction.id
                    && instruction.id < last.instruction
            })
            .expect("a clobbering CallUnit must sit inside the reload interval");
        // Every other allowlisted view overlaps the call's clobber set; the
        // surviving callee-saved view does not.
        for name in call_spanning_reload_allowlist(target) {
            let view = model.view_named(name).unwrap();
            let clobbered = view
                .units
                .iter()
                .chain(&view.write_units)
                .any(|unit| call.clobbers.contains(unit));
            assert_eq!(
                clobbered,
                view.id != saved,
                "{target:?}: allowlisted view {name} clobbered={clobbered}"
            );
        }

        let assigned = sources.assign(selected_lowering_budget()).unwrap();
        let function = assigned
            .plan()
            .functions
            .iter()
            .position(|function| function.machine == caller)
            .unwrap();
        let assignment = assigned.plan().functions[function]
            .assignment
            .as_ref()
            .expect("the call-spanning reload must be assigned");
        assert_eq!(assignment.start, first.point);
        assert_eq!(assignment.exclusive_end, LiveRangePoint(last.point.0 + 1));
        assert_eq!(assignment.candidates, vec![saved]);
        assert_eq!(assignment.view, saved);

        let canonical = assigned.plan().clone();
        assert!(sources.validate(canonical.clone()).is_ok());

        let mut root = canonical.clone();
        root.abstract_spill_insertion =
            selected_instructions_to_register_homes::unsequenced_spill_stages::AbstractSpillInsertionIdentity::from_bytes(
                [0x5c; 32],
            );
        assert_eq!(
            sources.validate(root),
            Err(selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::RootMismatch)
        );

        let caller_saved = call_spanning_reload_allowlist(target)
            .iter()
            .map(|name| model.view_named(name).unwrap().id)
            .find(|view| *view != saved)
            .unwrap();
        type Plan =
            selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomePlan;
        for corrupt in [
            // A caller-saved home cannot legitimately carry the reload across
            // the clobbering call.
            |plan: &mut Plan, function: usize, caller_saved: RegisterViewId| {
                plan.functions[function].assignment.as_mut().unwrap().view = caller_saved;
            },
            |plan: &mut Plan, function: usize, _| {
                plan.functions[function]
                    .assignment
                    .as_mut()
                    .unwrap()
                    .candidates
                    .clear();
            },
            // Shrinking the interval below the intervening call must mismatch
            // the recomputed reload bounds.
            |plan: &mut Plan, function: usize, _| {
                let assignment = plan.functions[function].assignment.as_mut().unwrap();
                assignment.exclusive_end = LiveRangePoint(assignment.start.0 + 1);
            },
            |plan: &mut Plan, function: usize, _| {
                plan.functions[function]
                    .assignment
                    .as_mut()
                    .unwrap()
                    .coexisting_homes
                    .clear();
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed, function, caller_saved);
            assert_eq!(
                sources.validate(changed),
                Err(
                    selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::NonCanonicalAssignment {
                        function,
                    }
                ),
                "{target:?}: corrupted plan must fail independent replay"
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate(usage),
            Err(selected_instructions_to_register_homes::unsequenced_spill_stages::ReloadValueHomeError::UsageMismatch)
        );
    }
}
