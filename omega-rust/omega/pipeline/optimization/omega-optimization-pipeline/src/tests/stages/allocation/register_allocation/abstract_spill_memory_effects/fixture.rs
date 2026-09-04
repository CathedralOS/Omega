use crate::tests::*;
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::super::{homed_spill_pseudo_instructions, recursive_reload_value_homes::Bundle};

pub(super) struct EffectBundle {
    pub(super) homed: omega_regalloc::ValidatedHomedSpillPseudoInstructions,
}

pub(super) fn build(constructor: fn(NativeTarget) -> Bundle, target: NativeTarget) -> EffectBundle {
    let source = homed_spill_pseudo_instructions::build(constructor, target);
    let homed =
        homed_spill_pseudo_instructions::lower(&source, selected_lowering_budget()).unwrap();
    EffectBundle { homed }
}

pub(super) fn lower(
    source: &EffectBundle,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_regalloc::ValidatedAbstractSpillMemoryEffects,
    omega_regalloc::AbstractSpillMemoryEffectError,
> {
    omega_regalloc::derive_abstract_spill_memory_effects(
        &source.homed,
        omega_regalloc::AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1,
        budget,
    )
}

pub(super) fn validate(
    source: &EffectBundle,
    plan: omega_regalloc::AbstractSpillMemoryEffectPlan,
) -> Result<
    omega_regalloc::ValidatedAbstractSpillMemoryEffects,
    omega_regalloc::AbstractSpillMemoryEffectError,
> {
    omega_regalloc::validate_abstract_spill_memory_effects(&source.homed, plan)
}

pub(super) const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 7,
    candidates: 9,
    validation_steps: 15,
    commits: 6,
    iterations: 10,
};

pub(super) fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(7, 9, 15, 6, 10).unwrap()
}
