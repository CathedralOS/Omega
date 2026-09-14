//! Direct projection from homed pseudos to abstract spill-area effects.

mod storage;
mod work;

use optimization_core::OptimizationWorkBudget;

use crate::{
    AbstractSpillMemoryEffectError, AbstractSpillMemoryEffectPlan, AbstractSpillMemoryEffectPolicy,
    FunctionAbstractSpillMemoryEffects, ValidatedHomedSpillPseudoInstructions,
};

pub(super) fn compute(
    source: &ValidatedHomedSpillPseudoInstructions,
    policy: AbstractSpillMemoryEffectPolicy,
    budget: OptimizationWorkBudget,
) -> Result<AbstractSpillMemoryEffectPlan, AbstractSpillMemoryEffectError> {
    if policy != AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1 {
        return Err(AbstractSpillMemoryEffectError::UnsupportedPolicy);
    }
    let mut functions = Vec::with_capacity(source.plan().functions.len());
    for (function, row) in source.plan().functions.iter().enumerate() {
        functions.push(project(function, row)?);
    }
    let usage = work::usage(source, &functions)?;
    if !usage.within(budget) {
        return Err(AbstractSpillMemoryEffectError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let receipt = source.receipt();
    Ok(AbstractSpillMemoryEffectPlan {
        homed_spill_pseudo_instructions: receipt.identity(),
        register_environment: receipt.register_environment(),
        allocator_availability: receipt.allocator_availability(),
        optimization_unit: receipt.optimization_unit(),
        fuel_schedule: receipt.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn project(
    function: usize,
    source: &crate::FunctionHomedSpillPseudoInstructions,
) -> Result<FunctionAbstractSpillMemoryEffects, AbstractSpillMemoryEffectError> {
    for (index, storage) in source.storage.iter().enumerate() {
        if source.storage[..index]
            .iter()
            .any(|row| row.id == storage.id)
        {
            return Err(AbstractSpillMemoryEffectError::DuplicateStorage {
                function,
                storage: storage.id,
            });
        }
        storage::validate(function, source.spill_area_bytes, storage)?;
    }
    let mut effects = Vec::with_capacity(source.instructions.len());
    for instruction in &source.instructions {
        let expected = u32::try_from(effects.len())
            .map_err(|_| AbstractSpillMemoryEffectError::WorkOverflow)?;
        if instruction.id().ordinal != expected {
            return Err(AbstractSpillMemoryEffectError::InvalidEffectOrder { function });
        }
        effects.push(storage::project(function, source, *instruction)?);
    }
    Ok(FunctionAbstractSpillMemoryEffects {
        machine: source.machine,
        spill_area_bytes: source.spill_area_bytes,
        effects,
    })
}
