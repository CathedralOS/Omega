//! Boundary-settlement gap tables shared by the runtime recovery rewrites:
//! each rebuilds one block body, so inserted instructions move the positions
//! that block's settlements name.

use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
};

/// The boundary and instruction position tables of one rebuilt block. A block
/// that carries no settlement keeps both empty and records nothing.
pub(crate) fn gap_tables(carries: bool, block: &SelectedBlock) -> (Vec<u32>, Vec<u32>) {
    let capacity = usize::from(carries) * (block.instructions.len() + 1);
    (Vec::with_capacity(capacity), Vec::with_capacity(capacity))
}

/// Record one emitted position in a gap table, for a block that carries
/// settlements.
pub(crate) fn push_position(table: &mut Vec<u32>, carries: bool, at: usize) -> Option<()> {
    if carries {
        table.push(u32::try_from(at).ok()?);
    }
    Some(())
}

/// Move every settlement of `block` onto the position its rebuild gave it: an
/// eliminated completion keeps the boundary gap, a hosted settlement names the
/// surviving instruction. `None` when a settlement names a position the
/// rebuild never emitted.
pub(crate) fn remap(
    settlements: &mut [SelectedBoundarySettlement],
    boundaries: &[u32],
    positions: &[u32],
    block: SelectedBlockId,
) -> Option<()> {
    for settlement in settlements {
        if settlement.block != block {
            continue;
        }
        let table = match settlement.settlement {
            SelectedBoundarySettlementPayload::ClaimCompletion(_) => boundaries,
            _ => positions,
        };
        settlement.instruction_index = *table.get(settlement.instruction_index as usize)?;
    }
    Some(())
}
