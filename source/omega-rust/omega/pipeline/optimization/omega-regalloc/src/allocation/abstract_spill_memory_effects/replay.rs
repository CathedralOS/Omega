//! Independent keyed reconstruction of abstract spill-area effects.

mod work;

use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::OptimizationWorkBudget;

use crate::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError, AbstractSpillMemoryEffectPlan,
    AbstractSpillMemoryEffectPolicy, FunctionAbstractSpillMemoryEffects,
    HomedSpillPseudoInstruction, SpillPseudoStorage, ValidatedHomedSpillPseudoInstructions,
};

pub(super) fn replay(
    source: &ValidatedHomedSpillPseudoInstructions,
    policy: AbstractSpillMemoryEffectPolicy,
    budget: OptimizationWorkBudget,
) -> Result<AbstractSpillMemoryEffectPlan, AbstractSpillMemoryEffectError> {
    if !matches!(
        policy,
        AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1
    ) {
        return Err(AbstractSpillMemoryEffectError::UnsupportedPolicy);
    }
    let mut functions = Vec::new();
    for (function, row) in source.plan().functions.iter().enumerate() {
        functions.push(reconstruct(function, row)?);
    }
    let usage = work::reconstruct(source, &functions)?;
    if !usage.within(budget) {
        return Err(AbstractSpillMemoryEffectError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let roots = source.receipt();
    Ok(AbstractSpillMemoryEffectPlan {
        homed_spill_pseudo_instructions: roots.identity(),
        register_environment: roots.register_environment(),
        allocator_availability: roots.allocator_availability(),
        optimization_unit: roots.optimization_unit(),
        fuel_schedule: roots.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn reconstruct(
    function: usize,
    source: &crate::FunctionHomedSpillPseudoInstructions,
) -> Result<FunctionAbstractSpillMemoryEffects, AbstractSpillMemoryEffectError> {
    let mut storage_by_id = BTreeMap::new();
    for storage in &source.storage {
        admit_storage(function, source.spill_area_bytes, storage)?;
        if storage_by_id.insert(storage.id, storage).is_some() {
            return Err(AbstractSpillMemoryEffectError::DuplicateStorage {
                function,
                storage: storage.id,
            });
        }
    }
    let mut ids = BTreeSet::new();
    let mut effects = Vec::new();
    for instruction in &source.instructions {
        let rebuilt = rebuild(function, &storage_by_id, *instruction)?;
        if !ids.insert(rebuilt.pseudo()) {
            return Err(AbstractSpillMemoryEffectError::InvalidEffectOrder { function });
        }
        effects.push(rebuilt);
    }
    let count =
        u32::try_from(ids.len()).map_err(|_| AbstractSpillMemoryEffectError::WorkOverflow)?;
    if ids.iter().map(|id| id.ordinal).ne(0..count) {
        return Err(AbstractSpillMemoryEffectError::InvalidEffectOrder { function });
    }
    Ok(FunctionAbstractSpillMemoryEffects {
        machine: source.machine,
        spill_area_bytes: source.spill_area_bytes,
        effects,
    })
}

fn rebuild(
    function: usize,
    storage_by_id: &BTreeMap<crate::GeneralizedSpillActionId, &SpillPseudoStorage>,
    instruction: HomedSpillPseudoInstruction,
) -> Result<AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError> {
    let storage_id = match instruction {
        HomedSpillPseudoInstruction::Store { storage, .. }
        | HomedSpillPseudoInstruction::Reload { storage, .. } => storage,
    };
    let storage = storage_by_id.get(&storage_id).copied().ok_or(
        AbstractSpillMemoryEffectError::MissingStorage {
            function,
            storage: storage_id,
        },
    )?;
    let instruction_block = match instruction {
        HomedSpillPseudoInstruction::Store { block, .. }
        | HomedSpillPseudoInstruction::Reload { block, .. } => block,
    };
    if storage.block != instruction_block {
        return Err(AbstractSpillMemoryEffectError::InvalidStorage {
            function,
            storage: storage_id,
        });
    }
    Ok(match instruction {
        HomedSpillPseudoInstruction::Store {
            id,
            action,
            block,
            point,
            before_instruction,
            before_reload,
            source,
            source_view,
            storage,
        } => AbstractSpillMemoryEffect::Write {
            pseudo: id,
            action,
            block,
            point,
            before_instruction,
            before_reload,
            source,
            source_view,
            storage,
            storage_class: storage_by_id[&storage].class,
            spill_area_offset: storage_by_id[&storage].spill_area_offset,
            size_bytes: storage_by_id[&storage].size_bytes,
            alignment_bytes: storage_by_id[&storage].alignment_bytes,
        },
        HomedSpillPseudoInstruction::Reload {
            id,
            action,
            block,
            point,
            before_instruction,
            storage,
            result,
            destination_class,
            destination_view,
        } => AbstractSpillMemoryEffect::Read {
            pseudo: id,
            action,
            block,
            point,
            before_instruction,
            storage,
            storage_class: storage_by_id[&storage].class,
            spill_area_offset: storage_by_id[&storage].spill_area_offset,
            size_bytes: storage_by_id[&storage].size_bytes,
            alignment_bytes: storage_by_id[&storage].alignment_bytes,
            result,
            destination_class,
            destination_view,
        },
    })
}

fn admit_storage(
    function: usize,
    area: u64,
    storage: &SpillPseudoStorage,
) -> Result<(), AbstractSpillMemoryEffectError> {
    if storage.size_bytes == 0
        || !storage.alignment_bytes.is_power_of_two()
        || !storage
            .spill_area_offset
            .is_multiple_of(storage.alignment_bytes)
        || storage
            .spill_area_offset
            .checked_add(storage.size_bytes)
            .is_none_or(|end| end > area)
    {
        return Err(AbstractSpillMemoryEffectError::InvalidStorage {
            function,
            storage: storage.id,
        });
    }
    Ok(())
}
