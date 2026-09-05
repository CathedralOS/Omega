use crate::tests::*;
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::super::{homed_spill_pseudo_instructions, recursive_reload_value_homes::Bundle};

pub(super) struct EffectBundle {
    pub(super) homed:
        selected_instructions_to_register_homes::ValidatedHomedSpillPseudoInstructions,
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
    selected_instructions_to_register_homes::ValidatedAbstractSpillMemoryEffects,
    selected_instructions_to_register_homes::AbstractSpillMemoryEffectError,
> {
    selected_instructions_to_register_homes::derive_abstract_spill_memory_effects(
        &source.homed,
        selected_instructions_to_register_homes::AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1,
        budget,
    )
}

pub(super) fn validate(
    source: &EffectBundle,
    plan: selected_instructions_to_register_homes::AbstractSpillMemoryEffectPlan,
) -> Result<
    selected_instructions_to_register_homes::ValidatedAbstractSpillMemoryEffects,
    selected_instructions_to_register_homes::AbstractSpillMemoryEffectError,
> {
    selected_instructions_to_register_homes::validate_abstract_spill_memory_effects(
        &source.homed,
        plan,
    )
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
