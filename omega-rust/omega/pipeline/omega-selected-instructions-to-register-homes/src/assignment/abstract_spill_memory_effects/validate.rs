//! Independent replay comparison and receipt sealing.

use crate::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError, AbstractSpillMemoryEffectPlan,
    AbstractSpillMemoryEffectReceipt, ValidatedAbstractSpillMemoryEffects,
    ValidatedHomedSpillPseudoInstructions, abstract_spill_memory_effect_plan_identity,
};

pub fn validate_abstract_spill_memory_effects(
    source: &ValidatedHomedSpillPseudoInstructions,
    candidate: AbstractSpillMemoryEffectPlan,
) -> Result<ValidatedAbstractSpillMemoryEffects, AbstractSpillMemoryEffectError> {
    let roots = source.receipt();
    if candidate.homed_spill_pseudo_instructions != roots.identity()
        || candidate.register_environment != roots.register_environment()
        || candidate.allocator_availability != roots.allocator_availability()
        || candidate.optimization_unit != roots.optimization_unit()
        || candidate.fuel_schedule != roots.fuel_schedule()
    {
        return Err(AbstractSpillMemoryEffectError::RootMismatch);
    }
    let expected = super::replay::replay(source, candidate.policy, candidate.budget)?;
    if candidate.usage != expected.usage {
        return Err(AbstractSpillMemoryEffectError::UsageMismatch);
    }
    if candidate.functions != expected.functions {
        return Err(AbstractSpillMemoryEffectError::NonCanonicalFunctions);
    }
    let read_count = candidate
        .functions
        .iter()
        .flat_map(|row| &row.effects)
        .filter(|effect| matches!(effect, AbstractSpillMemoryEffect::Read { .. }))
        .count();
    let write_count = candidate
        .functions
        .iter()
        .flat_map(|row| &row.effects)
        .filter(|effect| matches!(effect, AbstractSpillMemoryEffect::Write { .. }))
        .count();
    let max_spill_area_bytes = candidate
        .functions
        .iter()
        .map(|row| row.spill_area_bytes)
        .max()
        .unwrap_or(0);
    let receipt = AbstractSpillMemoryEffectReceipt {
        identity: abstract_spill_memory_effect_plan_identity(&candidate),
        homed_spill_pseudo_instructions: candidate.homed_spill_pseudo_instructions,
        register_environment: candidate.register_environment,
        allocator_availability: candidate.allocator_availability,
        optimization_unit: candidate.optimization_unit,
        fuel_schedule: candidate.fuel_schedule,
        usage: candidate.usage,
        function_count: candidate.functions.len(),
        read_count,
        write_count,
        max_spill_area_bytes,
    };
    Ok(ValidatedAbstractSpillMemoryEffects {
        plan: candidate,
        receipt,
    })
}
