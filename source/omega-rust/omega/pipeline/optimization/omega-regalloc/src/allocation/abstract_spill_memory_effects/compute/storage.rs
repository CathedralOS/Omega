//! Producer-local abstract storage admission and effect projection.

use crate::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError, HomedSpillPseudoInstruction,
    SpillPseudoStorage,
};

pub(super) fn project(
    function: usize,
    owner: &crate::FunctionHomedSpillPseudoInstructions,
    instruction: HomedSpillPseudoInstruction,
) -> Result<AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError> {
    let storage_id = match instruction {
        HomedSpillPseudoInstruction::Store { storage, .. }
        | HomedSpillPseudoInstruction::Reload { storage, .. } => storage,
    };
    let storage = owner
        .storage
        .iter()
        .find(|row| row.id == storage_id)
        .ok_or(AbstractSpillMemoryEffectError::MissingStorage {
            function,
            storage: storage_id,
        })?;
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
            storage_class: storage_class(owner, storage),
            spill_area_offset: storage_offset(owner, storage),
            size_bytes: storage_size(owner, storage),
            alignment_bytes: storage_alignment(owner, storage),
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
            storage_class: storage_class(owner, storage),
            spill_area_offset: storage_offset(owner, storage),
            size_bytes: storage_size(owner, storage),
            alignment_bytes: storage_alignment(owner, storage),
            result,
            destination_class,
            destination_view,
        },
    })
}

pub(super) fn validate(
    function: usize,
    area: u64,
    storage: &SpillPseudoStorage,
) -> Result<(), AbstractSpillMemoryEffectError> {
    let valid = storage.size_bytes != 0
        && storage.alignment_bytes.is_power_of_two()
        && storage.spill_area_offset % storage.alignment_bytes == 0
        && storage
            .spill_area_offset
            .checked_add(storage.size_bytes)
            .is_some_and(|end| end <= area);
    if valid {
        Ok(())
    } else {
        Err(AbstractSpillMemoryEffectError::InvalidStorage {
            function,
            storage: storage.id,
        })
    }
}

fn storage_row(
    owner: &crate::FunctionHomedSpillPseudoInstructions,
    id: crate::GeneralizedSpillActionId,
) -> &SpillPseudoStorage {
    owner
        .storage
        .iter()
        .find(|row| row.id == id)
        .expect("storage was admitted")
}

fn storage_class(
    owner: &crate::FunctionHomedSpillPseudoInstructions,
    id: crate::GeneralizedSpillActionId,
) -> crate::LogicalSpillStorageClass {
    storage_row(owner, id).class
}

fn storage_offset(
    owner: &crate::FunctionHomedSpillPseudoInstructions,
    id: crate::GeneralizedSpillActionId,
) -> u64 {
    storage_row(owner, id).spill_area_offset
}

fn storage_size(
    owner: &crate::FunctionHomedSpillPseudoInstructions,
    id: crate::GeneralizedSpillActionId,
) -> u64 {
    storage_row(owner, id).size_bytes
}

fn storage_alignment(
    owner: &crate::FunctionHomedSpillPseudoInstructions,
    id: crate::GeneralizedSpillActionId,
) -> u64 {
    storage_row(owner, id).alignment_bytes
}
