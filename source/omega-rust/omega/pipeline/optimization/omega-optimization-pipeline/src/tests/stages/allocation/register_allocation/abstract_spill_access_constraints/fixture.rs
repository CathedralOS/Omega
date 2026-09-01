use crate::tests::*;
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::super::{homed_spill_pseudo_instructions, recursive_reload_value_homes::Bundle};

pub(super) struct ConstraintBundle {
    pub(super) effects: omega_regalloc::ValidatedAbstractSpillMemoryEffects,
}

pub(super) fn build(
    constructor: fn(NativeTarget) -> Bundle,
    target: NativeTarget,
) -> ConstraintBundle {
    let homed_source = homed_spill_pseudo_instructions::build(constructor, target);
    let homed =
        homed_spill_pseudo_instructions::lower(&homed_source, selected_lowering_budget()).unwrap();
    let effects = omega_regalloc::derive_abstract_spill_memory_effects(
        &homed,
        omega_regalloc::AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1,
        OptimizationWorkBudget::new(7, 9, 15, 6, 10).unwrap(),
    )
    .unwrap();
    ConstraintBundle { effects }
}

pub(super) fn constrain(
    source: &ConstraintBundle,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_regalloc::ValidatedAbstractSpillAccessConstraints,
    omega_regalloc::AbstractSpillAccessConstraintError,
> {
    omega_regalloc::constrain_abstract_spill_accesses(
        &source.effects,
        omega_regalloc::AbstractSpillAccessConstraintPolicy::BlockLocalDataBarrierAndOverlapV1,
        budget,
    )
}

pub(super) fn validate(
    source: &ConstraintBundle,
    plan: omega_regalloc::AbstractSpillAccessConstraintPlan,
) -> Result<
    omega_regalloc::ValidatedAbstractSpillAccessConstraints,
    omega_regalloc::AbstractSpillAccessConstraintError,
> {
    omega_regalloc::validate_abstract_spill_access_constraints(&source.effects, plan)
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
