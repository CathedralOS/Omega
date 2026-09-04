//! Architectural register-unit discovery, actions, and fragments.

use super::*;

pub(super) fn architectural_units(
    function_index: usize,
    liveness: &crate::FunctionLiveness,
) -> Result<Vec<ArchitecturalUnitLiveRange>, LiveRangeError> {
    let mut units = BTreeSet::new();
    for block in &liveness.blocks {
        units.extend(block.unit_live_in.iter().copied());
        units.extend(block.unit_live_out.iter().copied());
        for instruction in &block.instructions {
            units.extend(instruction.unit_uses.iter().copied());
            units.extend(instruction.unit_defs.iter().copied());
            units.extend(instruction.unit_clobbers.iter().copied());
            units.extend(instruction.unit_live_in.iter().copied());
            units.extend(instruction.unit_live_out.iter().copied());
        }
        for edge in &block.successors {
            units.extend(edge.unit_live.iter().copied());
        }
    }
    units
        .into_iter()
        .map(|unit| build_unit(function_index, liveness, unit))
        .collect()
}

pub(super) fn build_unit(
    function: usize,
    liveness: &crate::FunctionLiveness,
    unit: RegisterUnitId,
) -> Result<ArchitecturalUnitLiveRange, LiveRangeError> {
    let mut actions = Vec::new();
    let mut fragments = Vec::new();
    let mut edge_connectors = Vec::new();
    for block in &liveness.blocks {
        let mut points = BTreeSet::new();
        for instruction in &block.instructions {
            let before = before_point(function, instruction.position)?;
            let after = after_point(function, instruction.position)?;
            if instruction.unit_live_in.contains(&unit) {
                points.insert(before);
            }
            if instruction.unit_live_out.contains(&unit) {
                points.insert(after);
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
                if values.contains(&unit) {
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
        fragments.extend(fragments_from_points(block.block, points));
        edge_connectors.extend(
            block
                .successors
                .iter()
                .filter(|edge| edge.unit_live.contains(&unit))
                .map(|edge| connector(block.block, edge)),
        );
    }
    Ok(ArchitecturalUnitLiveRange {
        unit,
        actions,
        fragments,
        edge_connectors,
    })
}
