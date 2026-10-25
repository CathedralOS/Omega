//! Architectural register-unit discovery, actions, and fragments.
use super::fragments::append_point;
use super::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    RegisterUnitId,
};
use super::{LiveRangeError, after_point, before_point, connector};

pub(super) fn architectural_units(
    function_index: usize,
    liveness: &selected_instructions::FunctionLiveness,
) -> Result<Vec<ArchitecturalUnitLiveRange>, LiveRangeError> {
    let mut unit_indices = Vec::<u32>::new();
    let mut rows = Vec::<ArchitecturalUnitLiveRange>::new();
    let ensure_unit =
        |unit: RegisterUnitId, unit_indices: &mut Vec<u32>, rows: &mut Vec<ArchitecturalUnitLiveRange>| -> usize {
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
                new_slot
            } else {
                *slot as usize
            }
        };

    for block in &liveness.blocks {
        for &unit in block.unit_live_in.iter().chain(&block.unit_live_out) {
            ensure_unit(unit, &mut unit_indices, &mut rows);
        }
        for instruction in &block.instructions {
            let before = before_point(function_index, instruction.position)?;
            let after = after_point(function_index, instruction.position)?;
            for &unit in &instruction.unit_live_in {
                let slot = ensure_unit(unit, &mut unit_indices, &mut rows);
                append_point(function_index, &mut rows[slot].fragments, block.block, before)?;
            }
            for &unit in &instruction.unit_live_out {
                let slot = ensure_unit(unit, &mut unit_indices, &mut rows);
                append_point(function_index, &mut rows[slot].fragments, block.block, after)?;
            }
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
                    let slot = ensure_unit(unit, &mut unit_indices, &mut rows);
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
        for edge in &block.successors {
            for (idx, &unit) in edge.unit_live.iter().enumerate() {
                if edge.unit_live[..idx].contains(&unit) {
                    continue;
                }
                let slot = ensure_unit(unit, &mut unit_indices, &mut rows);
                rows[slot].edge_connectors.push(connector(block.block, edge));
            }
        }
    }
    rows.sort_by_key(|row| row.unit);
    Ok(rows)
}
