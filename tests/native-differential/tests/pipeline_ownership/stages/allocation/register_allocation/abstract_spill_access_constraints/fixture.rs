use crate::tests::*;
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::super::{homed_spill_pseudo_instructions, recursive_reload_value_homes::Bundle};

pub(super) struct ConstraintBundle {
    pub(super) effects:
        selected_instructions_to_register_homes::ValidatedAbstractSpillMemoryEffects,
}

pub(super) fn build(
    constructor: fn(NativeTarget) -> Bundle,
    target: NativeTarget,
) -> ConstraintBundle {
    let homed_source = homed_spill_pseudo_instructions::build(constructor, target);
    let homed =
        homed_spill_pseudo_instructions::lower(&homed_source, selected_lowering_budget()).unwrap();
    let effects = selected_instructions_to_register_homes::derive_abstract_spill_memory_effects(
        &homed,
        selected_instructions_to_register_homes::AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1,
        OptimizationWorkBudget::new(7, 9, 15, 6, 10).unwrap(),
    )
    .unwrap();
    ConstraintBundle { effects }
}

pub(super) fn constrain(
    source: &ConstraintBundle,
    budget: OptimizationWorkBudget,
) -> Result<
    selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints,
    selected_instructions_to_register_homes::AbstractSpillAccessConstraintError,
> {
    selected_instructions_to_register_homes::constrain_abstract_spill_accesses(
        &source.effects,
        selected_instructions_to_register_homes::AbstractSpillAccessConstraintPolicy::BlockLocalDataBarrierAndOverlapV1,
        budget,
    )
}

pub(super) fn validate(
    source: &ConstraintBundle,
    plan: selected_instructions_to_register_homes::AbstractSpillAccessConstraintPlan,
) -> Result<
    selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints,
    selected_instructions_to_register_homes::AbstractSpillAccessConstraintError,
> {
    selected_instructions_to_register_homes::validate_abstract_spill_access_constraints(
        &source.effects,
        plan,
    )
}

pub(super) const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 7,
    candidates: 15,
    validation_steps: 33,
    commits: 18,
    iterations: 22,
};

pub(super) fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(7, 15, 33, 18, 22).unwrap()
}
