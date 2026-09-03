//! Producer-local canonical block placement.

use crate::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessKind, AbstractSpillAccessPlacement,
    AbstractSpillMemoryEffect, FunctionAbstractSpillMemoryEffects,
};

pub(super) fn project(
    function: usize,
    source: &FunctionAbstractSpillMemoryEffects,
) -> Result<Vec<AbstractSpillAccessPlacement>, AbstractSpillAccessConstraintError> {
    let mut ordered = source.effects.to_vec();
    ordered.sort_by_key(|effect| {
        let (block, point) = position(*effect);
        (block, point, effect.pseudo())
    });
    let mut seen = Vec::new();
    let mut block_counts = Vec::new();
    let mut placements = Vec::with_capacity(ordered.len());
    for effect in ordered {
        if seen.contains(&effect.pseudo()) {
            return Err(AbstractSpillAccessConstraintError::DuplicateAccess {
                function,
                pseudo: effect.pseudo(),
            });
        }
        seen.push(effect.pseudo());
        let (block, _) = position(effect);
        let block_ordinal = next_block_ordinal(&mut block_counts, block)?;
        let placement = placement(effect, block_ordinal);
        validate_geometry(function, source.spill_area_bytes, placement)?;
        if placements
            .last()
            .is_some_and(|prior: &AbstractSpillAccessPlacement| {
                prior.block == placement.block
                    && (prior.point, prior.pseudo) >= (placement.point, placement.pseudo)
            })
        {
            return Err(AbstractSpillAccessConstraintError::InvalidAccessOrder { function });
        }
        placements.push(placement);
    }
    Ok(placements)
}

fn placement(
    effect: AbstractSpillMemoryEffect,
    block_ordinal: u32,
) -> AbstractSpillAccessPlacement {
    match effect {
        AbstractSpillMemoryEffect::Write {
            pseudo,
            block,
            point,
            before_instruction,
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
            ..
        } => AbstractSpillAccessPlacement {
            pseudo,
            block,
            block_ordinal,
            point,
            before_instruction,
            kind: AbstractSpillAccessKind::Write,
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
        },
        AbstractSpillMemoryEffect::Read {
            pseudo,
            block,
            point,
            before_instruction,
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
            ..
        } => AbstractSpillAccessPlacement {
            pseudo,
            block,
            block_ordinal,
            point,
            before_instruction,
            kind: AbstractSpillAccessKind::Read,
            storage,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
        },
    }
}

fn next_block_ordinal(
    counts: &mut Vec<(omega_selected_instructions::SelectedBlockId, u32)>,
    block: omega_selected_instructions::SelectedBlockId,
) -> Result<u32, AbstractSpillAccessConstraintError> {
    if let Some((_, count)) = counts.iter_mut().find(|(candidate, _)| *candidate == block) {
        let ordinal = *count;
        *count = count
            .checked_add(1)
            .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?;
        Ok(ordinal)
    } else {
        counts.push((block, 1));
        Ok(0)
    }
}

fn validate_geometry(
    function: usize,
    area: u64,
    placement: AbstractSpillAccessPlacement,
) -> Result<(), AbstractSpillAccessConstraintError> {
    if placement.size_bytes == 0
        || !placement.alignment_bytes.is_power_of_two()
        || !placement
            .spill_area_offset
            .is_multiple_of(placement.alignment_bytes)
        || placement
            .spill_area_offset
            .checked_add(placement.size_bytes)
            .is_none_or(|end| end > area)
    {
        return Err(AbstractSpillAccessConstraintError::InvalidGeometry {
            function,
            pseudo: placement.pseudo,
        });
    }
    Ok(())
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
