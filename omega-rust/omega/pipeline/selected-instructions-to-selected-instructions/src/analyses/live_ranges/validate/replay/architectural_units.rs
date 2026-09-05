//! Independent architectural-unit action and range reconstruction.

use std::collections::BTreeSet;

use crate::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    LiveRangeError,
};
use register_model::RegisterUnitId;

use super::fragments::{append_maximal, checked_after, checked_before, edge_row};

pub(super) fn replay_all(
    function: usize,
    live: &crate::FunctionLiveness,
) -> Result<Vec<ArchitecturalUnitLiveRange>, LiveRangeError> {
    let mut discovered_units = BTreeSet::new();
    for block in &live.blocks {
        discovered_units.extend(block.unit_live_in.iter().copied());
        discovered_units.extend(block.unit_live_out.iter().copied());
        for instruction in &block.instructions {
            discovered_units.extend(instruction.unit_uses.iter().copied());
            discovered_units.extend(instruction.unit_defs.iter().copied());
            discovered_units.extend(instruction.unit_clobbers.iter().copied());
            discovered_units.extend(instruction.unit_live_in.iter().copied());
            discovered_units.extend(instruction.unit_live_out.iter().copied());
        }
        for edge in &block.successors {
            discovered_units.extend(edge.unit_live.iter().copied());
        }
    }
    discovered_units
        .into_iter()
        .map(|unit| replay_unit(function, live, unit))
        .collect()
}

fn replay_unit(
    function: usize,
    live: &crate::FunctionLiveness,
    unit: RegisterUnitId,
) -> Result<ArchitecturalUnitLiveRange, LiveRangeError> {
    let mut actions = Vec::new();
    let mut fragments = Vec::new();
    let mut edge_connectors = Vec::new();
    for block in &live.blocks {
        let mut occupied = BTreeSet::new();
        for instruction in &block.instructions {
            let before = checked_before(function, instruction.position.0)?;
            let after = checked_after(function, instruction.position.0)?;
            if instruction.unit_live_in.binary_search(&unit).is_ok() {
                occupied.insert(before);
            }
            if instruction.unit_live_out.binary_search(&unit).is_ok() {
                occupied.insert(after);
            }
            for (kind, rows, point) in [
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
                if rows.binary_search(&unit).is_ok() {
                    actions.push(ArchitecturalUnitAction {
                        block: block.block,
                        position: instruction.position,
                        point,
                        instruction: instruction.instruction,
                        kind,
                    });
                }
            }
        }
        append_maximal(block.block, occupied, &mut fragments);
        edge_connectors.extend(
            block
                .successors
                .iter()
                .filter(|edge| edge.unit_live.binary_search(&unit).is_ok())
                .map(|edge| edge_row(block.block, edge)),
        );
    }
    Ok(ArchitecturalUnitLiveRange {
        unit,
        actions,
        fragments,
        edge_connectors,
    })
}
