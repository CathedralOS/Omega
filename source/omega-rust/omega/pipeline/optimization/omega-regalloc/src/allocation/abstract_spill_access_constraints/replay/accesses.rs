//! Replay-local keyed block placement.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessKind, AbstractSpillAccessPlacement,
    AbstractSpillMemoryEffect, FunctionAbstractSpillMemoryEffects,
};

pub(super) fn reconstruct(
    function: usize,
    source: &FunctionAbstractSpillMemoryEffects,
) -> Result<Vec<AbstractSpillAccessPlacement>, AbstractSpillAccessConstraintError> {
    let mut effects = BTreeMap::new();
    let mut pseudos = BTreeSet::new();
    for effect in &source.effects {
        if !pseudos.insert(effect.pseudo()) {
            return Err(AbstractSpillAccessConstraintError::DuplicateAccess {
                function,
                pseudo: effect.pseudo(),
            });
        }
        let (block, point) = position(*effect);
        if effects
            .insert((block, point, effect.pseudo()), *effect)
            .is_some()
        {
            return Err(AbstractSpillAccessConstraintError::DuplicateAccess {
                function,
                pseudo: effect.pseudo(),
            });
        }
    }
    let mut block_counts = BTreeMap::new();
    let mut placements = Vec::new();
    for ((block, point, pseudo), effect) in effects {
        let block_ordinal = *block_counts.entry(block).or_insert(0_u32);
        block_counts.insert(
            block,
            block_ordinal
                .checked_add(1)
                .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?,
        );
        let (kind, storage, spill_area_offset, size_bytes, alignment_bytes, before_instruction) =
            fields(effect);
        let placement = AbstractSpillAccessPlacement {
            pseudo,
            block,
            block_ordinal,
            point,
            before_instruction,
            kind,
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
        };
        validate_geometry(function, source.spill_area_bytes, placement)?;
        placements.push(placement);
    }
    Ok(placements)
}

fn fields(
    effect: AbstractSpillMemoryEffect,
) -> (
    AbstractSpillAccessKind,
    crate::GeneralizedSpillActionId,
    u64,
    u64,
    u64,
    omega_selected_instructions::SelectedInstructionId,
) {
    match effect {
        AbstractSpillMemoryEffect::Write {
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
            before_instruction,
            ..
        } => (
            AbstractSpillAccessKind::Write,
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
            before_instruction,
        ),
        AbstractSpillMemoryEffect::Read {
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
            before_instruction,
            ..
        } => (
            AbstractSpillAccessKind::Read,
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
            before_instruction,
        ),
    }
}

fn position(
    effect: AbstractSpillMemoryEffect,
) -> (
    omega_selected_instructions::SelectedBlockId,
    crate::LiveRangePoint,
) {
    match effect {
        AbstractSpillMemoryEffect::Write { block, point, .. }
        | AbstractSpillMemoryEffect::Read { block, point, .. } => (block, point),
    }
}

fn validate_geometry(
    function: usize,
    area: u64,
    placement: AbstractSpillAccessPlacement,
) -> Result<(), AbstractSpillAccessConstraintError> {
    let end = placement
        .spill_area_offset
        .checked_add(placement.size_bytes);
    if placement.size_bytes == 0
        || !placement.alignment_bytes.is_power_of_two()
        || placement.spill_area_offset % placement.alignment_bytes != 0
        || !matches!(end, Some(end) if end <= area)
    {
        return Err(AbstractSpillAccessConstraintError::InvalidGeometry {
            function,
            pseudo: placement.pseudo,
        });
    }
    Ok(())
}
