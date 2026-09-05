//! Target-neutral logical second-spill actions after epoch-one victim choice.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;
use omega_selected_instructions::{SelectedInstructionId, VirtualRegisterId};

use super::{
    reload_value_homes::ReloadSources,
    spill_recovery_choice::choose,
    spill_recovery_worklist::{pressure_sources, seed},
};

pub(super) fn plan(
    sources: &ReloadSources,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_selected_instructions_to_register_homes::ValidatedSpillRecoveryActions,
    omega_selected_instructions_to_register_homes::SpillRecoveryActionError,
> {
    let ranges = sources.legality().live_range_stage();
    let selected = ranges.liveness_stage().selected_stage();
    let worklist = seed(sources, selected_lowering_budget()).unwrap();
    let choices = choose(sources, selected_lowering_budget()).unwrap();
    omega_selected_instructions_to_register_homes::plan_spill_recovery_actions(
        selected.selected(),
        ranges.ranges(),
        sources.legality().legality(),
        sources.insertion(),
        &worklist,
        &choices,
        omega_selected_instructions_to_register_homes::SpillRecoveryActionPolicy::EpochOneActiveResidentInstructionResultU64LaterFlexibleUsesV1,
        budget,
    )
}

fn validate(
    sources: &ReloadSources,
    candidate: omega_selected_instructions_to_register_homes::SpillRecoveryActionPlan,
) -> Result<
    omega_selected_instructions_to_register_homes::ValidatedSpillRecoveryActions,
    omega_selected_instructions_to_register_homes::SpillRecoveryActionError,
> {
    let ranges = sources.legality().live_range_stage();
    let selected = ranges.liveness_stage().selected_stage();
    let worklist = seed(sources, selected_lowering_budget()).unwrap();
    let choices = choose(sources, selected_lowering_budget()).unwrap();
    omega_selected_instructions_to_register_homes::validate_spill_recovery_actions(
        selected.selected(),
        ranges.ranges(),
        sources.legality().legality(),
        sources.insertion(),
        &worklist,
        &choices,
        candidate,
    )
}

#[test]
fn epoch_one_action_is_exact_deterministic_and_target_neutral_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let first = plan(&sources, selected_lowering_budget()).unwrap();
        let second = plan(&sources, selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().action_count(), 1);
        assert_eq!(first.receipt().rewrite_count(), 1);
        assert_eq!(first.receipt().usage(), first.plan().usage);
        assert_eq!(
            first.plan().usage,
            OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 5,
                commits: 1,
                iterations: 1,
            }
        );

        let action = &first.plan().actions[0];
        assert_eq!(action.source_work_item.epoch, 1);
        assert_eq!(action.source_work_item.ordinal, 0);
        assert_eq!(action.function, 0);
        assert_eq!(action.pressure_point, LiveRangePoint(12));
        assert_eq!(action.victim, VirtualRegisterId(3));
        assert_eq!(action.current_view, action.reclaimed_view);
        assert_eq!(action.store.before_source_reload, action.source_reload);
        assert_eq!(action.store.before_instruction, SelectedInstructionId(6));
        assert_eq!(action.store.source, action.victim);
        assert_eq!(action.store.storage, action.storage.id);
        assert_eq!(action.reload.before_instruction, SelectedInstructionId(7));
        assert_eq!(action.reload.storage, action.storage.id);
        assert_eq!(action.reload.result.epoch, 1);
        assert_eq!(action.reload.result.ordinal, 0);
        assert_eq!(
            action
                .rewrites
                .iter()
                .map(|rewrite| (rewrite.point, rewrite.instruction, rewrite.operand))
                .collect::<Vec<_>>(),
            vec![(LiveRangePoint(14), SelectedInstructionId(7), 0)]
        );
        assert!(
            action
                .rewrites
                .iter()
                .all(|rewrite| rewrite.result == action.reload.result)
        );
    }
}

#[test]
fn independent_replay_rejects_root_action_namespace_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        let canonical = plan(&sources, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        let mut root = canonical.clone();
        root.choices =
            omega_selected_instructions_to_register_homes::SpillRecoveryChoiceIdentity::from_bytes(
                [0x3a; 32],
            );
        assert_eq!(
            validate(&sources, root),
            Err(omega_selected_instructions_to_register_homes::SpillRecoveryActionError::RootMismatch)
        );

        for corrupt in [
            |candidate: &mut omega_selected_instructions_to_register_homes::SpillRecoveryActionPlan| {
                candidate.actions[0].victim = VirtualRegisterId(4);
            },
            |candidate: &mut omega_selected_instructions_to_register_homes::SpillRecoveryActionPlan| {
                candidate.actions[0].store.before_instruction.0 += 1;
            },
            |candidate: &mut omega_selected_instructions_to_register_homes::SpillRecoveryActionPlan| {
                candidate.actions[0].reload.before_instruction.0 += 1;
            },
            |candidate: &mut omega_selected_instructions_to_register_homes::SpillRecoveryActionPlan| {
                candidate.actions[0].rewrites[0].operand += 1;
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                validate(&sources, changed),
                Err(omega_selected_instructions_to_register_homes::SpillRecoveryActionError::NonCanonicalActions)
            );
        }

        let mut namespace = canonical.clone();
        namespace.actions[0].storage.id.ordinal += 1;
        assert_eq!(
            validate(&sources, namespace),
            Err(omega_selected_instructions_to_register_homes::SpillRecoveryActionError::NonCanonicalNamespace)
        );

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            validate(&sources, usage),
            Err(omega_selected_instructions_to_register_homes::SpillRecoveryActionError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_all_axes_and_cross_target_custody_are_enforced() {
    let exact = OptimizationWorkBudget::new(1, 1, 5, 1, 1).unwrap();
    // The other four exact axes are already the minimum representable
    // nonzero budget, so validation work is the sole representable first-over.
    let first_over = OptimizationWorkBudget::new(1, 1, 4, 1, 1).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = pressure_sources(target);
        assert!(plan(&sources, exact).is_ok());
        assert!(matches!(
            plan(&sources, first_over),
            Err(omega_selected_instructions_to_register_homes::SpillRecoveryActionError::BudgetExceeded {
                required: OptimizationWorkUsage {
                    rule_evaluations: 1,
                    candidates: 1,
                    validation_steps: 5,
                    commits: 1,
                    iterations: 1,
                },
                budget: actual,
            }) if actual == first_over
        ));
    }

    let x86 = pressure_sources(NativeTarget::linux_x64());
    let arm = pressure_sources(NativeTarget::linux_arm64());
    let x86_plan = plan(&x86, exact).unwrap().plan().clone();
    assert_eq!(
        validate(&arm, x86_plan),
        Err(omega_selected_instructions_to_register_homes::SpillRecoveryActionError::RootMismatch)
    );
}
