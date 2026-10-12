//! Spill accesses may clobber machine state without changing the stored value.
//! Admission preserves implicit state at the insertion boundary, not by
//! requiring every target's frame operations to be globally clobber-free.
//! Source unit liveness includes successor demand; a flags consumer in another
//! block therefore protects the same state as a local consumer. Allocatable
//! virtual payloads remain the subsequent allocator's responsibility.

use register_model::RegisterInstructionConstraint;
use selected_instructions::SelectedFunction;

use super::{RuntimeSpillError, StorageDefinition, StoragePosition, slot::BlockUsePositions};

pub(super) fn preserve_live_units(
    function_index: usize,
    function: &SelectedFunction,
    definitions: &[StorageDefinition],
    uses: &[BlockUsePositions],
    address: &RegisterInstructionConstraint,
    load: &RegisterInstructionConstraint,
    store: &RegisterInstructionConstraint,
) -> Result<(), RuntimeSpillError> {
    let reload_clobbers: Vec<_> = address
        .clobbers
        .iter()
        .chain(&load.clobbers)
        .copied()
        .collect();
    let all_clobbers: Vec<_> = reload_clobbers
        .iter()
        .chain(&store.clobbers)
        .copied()
        .collect();
    // A unit never implicitly read by source cannot carry source state through
    // an inserted access. Avoid rebuilding liveness for that common case.
    if !function.blocks.iter().any(|block| {
        block
            .instructions
            .iter()
            .chain(std::iter::once(super::control(&block.terminator).0))
            .any(|instruction| {
                instruction
                    .implicit_uses
                    .iter()
                    .any(|unit| all_clobbers.contains(unit))
            })
    }) {
        return Ok(());
    }
    let liveness =
        crate::analyses::liveness::compute::compute_function_units(function_index, function)
            .map_err(|_| RuntimeSpillError::SourceMismatch)?;
    for definition in definitions {
        let block = liveness
            .blocks
            .get(definition.block_index)
            .ok_or(RuntimeSpillError::SourceMismatch)?;
        let live = match definition.position {
            StoragePosition::BlockStart => &block.unit_live_in,
            StoragePosition::AfterInstruction(instruction)
            | StoragePosition::AfterUseDef { instruction, .. } => {
                &block
                    .instructions
                    .iter()
                    .find(|row| row.instruction == instruction)
                    .ok_or(RuntimeSpillError::SourceMismatch)?
                    .unit_live_out
            }
        };
        if store.clobbers.iter().any(|unit| live.contains(unit)) {
            return Err(RuntimeSpillError::UnsupportedUse);
        }
    }
    for (block_index, positions) in uses.iter().enumerate() {
        let block = liveness
            .blocks
            .get(block_index)
            .ok_or(RuntimeSpillError::SourceMismatch)?;
        for position in positions
            .unpinned
            .iter()
            .chain(&positions.pinned)
            .chain(&positions.dedicated)
        {
            let instruction = block
                .instructions
                .get(*position)
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            if reload_clobbers
                .iter()
                .any(|unit| instruction.unit_live_in.contains(unit))
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
        }
        if positions.end_of_block {
            // Terminator operands reload before the terminator; edge bindings
            // reload afterward. This position inventory can request either,
            // so both sides retain their source implicit state.
            let terminator = block
                .instructions
                .last()
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            if reload_clobbers.iter().any(|unit| {
                terminator.unit_live_in.contains(unit) || terminator.unit_live_out.contains(unit)
            }) {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
        }
    }
    Ok(())
}
