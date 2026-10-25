//! Architectural register-unit discovery, actions, and fragments.
use super::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    LiveRangeFragment, LiveRangePoint, RegisterUnitId,
};
use super::{LiveRangeError, after_point, before_point, connector};

pub(super) fn architectural_units(
    function_index: usize,
    liveness: &selected_instructions::FunctionLiveness,
) -> Result<Vec<ArchitecturalUnitLiveRange>, LiveRangeError> {
    let mut unit_indices = Vec::<u32>::new();
    let mut rows = Vec::<ArchitecturalUnitLiveRange>::new();
    let mut open_unit_end = Vec::<u32>::new();
    let mut touched_units = Vec::<(usize, usize)>::new();
    let ensure_unit =
        |unit: RegisterUnitId,
         unit_indices: &mut Vec<u32>,
         rows: &mut Vec<ArchitecturalUnitLiveRange>,
         open_unit_end: &mut Vec<u32>|
         -> usize {
            let idx = usize::from(unit.0);
            if idx >= unit_indices.len() {
                unit_indices.resize(idx + 1, u32::MAX);
            }
            let slot = &mut unit_indices[idx];
            if *slot == u32::MAX {
                let new_slot = rows.len();
                *slot = new_slot as u32;
                rows.push(ArchitecturalUnitLiveRange {
                    unit,
                    actions: Vec::new(),
                    fragments: Vec::new(),
                    edge_connectors: Vec::new(),
                });
                open_unit_end.push(u32::MAX);
                new_slot
            } else {
                *slot as usize
            }
        };

    for block in &liveness.blocks {
        for &unit in block.unit_live_in.iter().chain(&block.unit_live_out) {
            ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
        }
        if let (Some(first), Some(last)) = (block.instructions.first(), block.instructions.last()) {
            let block_start = before_point(function_index, first.position)?.0;
            let block_end = after_point(function_index, last.position)?
                .0
                .checked_add(1)
                .ok_or(LiveRangeError::PointOverflow {
                    function: function_index,
                })?;
            for &unit in &block.unit_live_out {
                let slot = ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                open_unit_end[slot] = block_end;
            }
            for instruction in block.instructions.iter().rev() {
                if instruction.unit_defs.is_empty()
                    && instruction.unit_clobbers.is_empty()
                    && instruction.unit_uses.is_empty()
                {
                    continue;
                }
                let _before = before_point(function_index, instruction.position)?.0;
                let after = after_point(function_index, instruction.position)?.0;
                for &killed in instruction
                    .unit_defs
                    .iter()
                    .chain(&instruction.unit_clobbers)
                {
                    if !instruction.unit_uses.contains(&killed) {
                        let slot =
                            ensure_unit(killed, &mut unit_indices, &mut rows, &mut open_unit_end);
                        if open_unit_end[slot] != u32::MAX {
                            let frags = &mut rows[slot].fragments;
                            if frags.last().map(|f| f.block) != Some(block.block) {
                                touched_units.push((slot, frags.len()));
                            }
                            frags.push(LiveRangeFragment {
                                block: block.block,
                                start: LiveRangePoint(after),
                                end: LiveRangePoint(open_unit_end[slot]),
                            });
                            open_unit_end[slot] = u32::MAX;
                        }
                    }
                }
                for &used in &instruction.unit_uses {
                    let slot = ensure_unit(used, &mut unit_indices, &mut rows, &mut open_unit_end);
                    if open_unit_end[slot] == u32::MAX {
                        open_unit_end[slot] = after;
                    }
                }
            }
            for &unit in &block.unit_live_in {
                let slot = ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                if open_unit_end[slot] != u32::MAX {
                    let frags = &mut rows[slot].fragments;
                    if frags.last().map(|f| f.block) != Some(block.block) {
                        touched_units.push((slot, frags.len()));
                    }
                    frags.push(LiveRangeFragment {
                        block: block.block,
                        start: LiveRangePoint(block_start),
                        end: LiveRangePoint(open_unit_end[slot]),
                    });
                    open_unit_end[slot] = u32::MAX;
                }
            }
            for (slot, first_frag) in touched_units.drain(..) {
                rows[slot].fragments[first_frag..].reverse();
            }
            for instruction in &block.instructions {
                if instruction.unit_uses.is_empty()
                    && instruction.unit_defs.is_empty()
                    && instruction.unit_clobbers.is_empty()
                {
                    continue;
                }
                let before = before_point(function_index, instruction.position)?;
                let after = after_point(function_index, instruction.position)?;
                for (kind, values, point) in [
                    (
                        ArchitecturalUnitActionKind::Use,
                        &instruction.unit_uses,
                        before,
                    ),
                    (
                        ArchitecturalUnitActionKind::Def,
                        &instruction.unit_defs,
                        after,
                    ),
                    (
                        ArchitecturalUnitActionKind::Clobber,
                        &instruction.unit_clobbers,
                        after,
                    ),
                ] {
                    for (idx, &unit) in values.iter().enumerate() {
                        if values[..idx].contains(&unit) {
                            continue;
                        }
                        let slot =
                            ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                        rows[slot].actions.push(ArchitecturalUnitAction {
                            block: block.block,
                            position: instruction.position,
                            point,
                            instruction: instruction.instruction,
                            kind,
                        });
                    }
                }
            }
        }
        for edge in &block.successors {
            for (idx, &unit) in edge.unit_live.iter().enumerate() {
                if edge.unit_live[..idx].contains(&unit) {
                    continue;
                }
                let slot = ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                rows[slot].edge_connectors.push(connector(block.block, edge));
            }
        }
    }
    rows.sort_by_key(|row| row.unit);
    Ok(rows)
}
