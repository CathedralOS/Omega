use std::collections::BTreeSet;

use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_terminal_selected_instructions::{TerminalSelectedBlockId, TerminalVirtualRegisterId};
use omega_terminal_target_operations_to_selected_instructions::ValidatedTerminalSelectedInstructions;

use crate::{
    TerminalArchitecturalUnitAction, TerminalArchitecturalUnitActionKind,
    TerminalArchitecturalUnitLiveRange, TerminalBlockLiveness, TerminalBlockPointDomain,
    TerminalFunctionLiveRanges, TerminalLiveRangeEdgeConnector, TerminalLiveRangeError,
    TerminalLiveRangeFragment, TerminalLiveRangePlan, TerminalLiveRangePoint,
    TerminalLivenessPosition, TerminalVirtualFixedConstraint, TerminalVirtualFixedConstraintSite,
    TerminalVirtualInterference, TerminalVirtualLiveRange, TerminalVirtualOccurrence,
    ValidatedTerminalLiveness,
};

pub(crate) fn compute_terminal_live_ranges(
    selected: &ValidatedTerminalSelectedInstructions,
    liveness: &ValidatedTerminalLiveness,
) -> Result<TerminalLiveRangePlan, TerminalLiveRangeError> {
    let functions = selected
        .plan()
        .functions
        .iter()
        .zip(&liveness.plan().functions)
        .enumerate()
        .map(|(index, (selected, live))| compute_function(index, selected, live))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalLiveRangePlan {
        selected: selected.receipt().identity(),
        liveness: liveness.receipt().identity(),
        optimization_unit: selected.receipt().optimization_unit(),
        fuel_schedule: selected.receipt().fuel_schedule(),
        target: selected.plan().target,
        functions,
    })
}

fn compute_function(
    function_index: usize,
    selected: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    liveness: &crate::TerminalFunctionLiveness,
) -> Result<TerminalFunctionLiveRanges, TerminalLiveRangeError> {
    reject_unsupported(function_index, liveness)?;
    let block_domains = liveness
        .blocks
        .iter()
        .map(|block| block_domain(function_index, block))
        .collect::<Result<Vec<_>, _>>()?;

    let mut virtual_rows = Vec::with_capacity(selected.virtual_registers.len());
    for register in &selected.virtual_registers {
        let occurrences = liveness
            .operand_positions
            .iter()
            .filter(|row| row.virtual_register == register.id)
            .map(|row| {
                Ok(TerminalVirtualOccurrence {
                    position: row.position,
                    point: operand_point(function_index, row.position, row.access)?,
                    instruction: row.instruction,
                    operand: row.operand,
                    access: row.access,
                })
            })
            .collect::<Result<Vec<_>, TerminalLiveRangeError>>()?;
        let mut fixed_constraints = Vec::new();
        if let Some(view) = register.entry_fixed_view {
            fixed_constraints.push(TerminalVirtualFixedConstraint {
                site: TerminalVirtualFixedConstraintSite::Entry,
                view,
            });
        }
        for row in liveness
            .operand_positions
            .iter()
            .filter(|row| row.virtual_register == register.id)
        {
            if let Some(view) = row.fixed_view {
                fixed_constraints.push(TerminalVirtualFixedConstraint {
                    site: TerminalVirtualFixedConstraintSite::Operand {
                        position: row.position,
                        point: operand_point(function_index, row.position, row.access)?,
                        instruction: row.instruction,
                        operand: row.operand,
                        access: row.access,
                    },
                    view,
                });
            }
        }
        let fragments = liveness
            .blocks
            .iter()
            .map(|block| virtual_fragments(function_index, block, register.id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        let edge_connectors = liveness
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .successors
                    .iter()
                    .filter(move |edge| edge.virtual_live.contains(&register.id))
                    .map(move |edge| connector(block.block, edge))
            })
            .collect();
        virtual_rows.push(TerminalVirtualLiveRange {
            virtual_register: register.id,
            class: register.class,
            occurrences,
            fixed_constraints,
            fragments,
            edge_connectors,
        });
    }

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
    let architectural_units = units
        .into_iter()
        .map(|unit| build_unit(function_index, liveness, unit))
        .collect::<Result<Vec<_>, _>>()?;

    let mut interference = BTreeSet::new();
    for (left_index, left) in virtual_rows.iter().enumerate() {
        for right in virtual_rows.iter().skip(left_index + 1) {
            if fragments_overlap(&left.fragments, &right.fragments) {
                interference.insert(TerminalVirtualInterference {
                    lower: left.virtual_register,
                    higher: right.virtual_register,
                });
            }
        }
    }
    Ok(TerminalFunctionLiveRanges {
        machine: selected.machine,
        block_domains,
        virtual_registers: virtual_rows,
        architectural_units,
        interference: interference.into_iter().collect(),
    })
}

fn block_domain(
    function: usize,
    block: &TerminalBlockLiveness,
) -> Result<TerminalBlockPointDomain, TerminalLiveRangeError> {
    let first = block
        .instructions
        .first()
        .ok_or(TerminalLiveRangeError::BlockDomainMismatch {
            function,
            block: block.block.0,
        })?;
    let last = block
        .instructions
        .last()
        .expect("nonempty block established above");
    Ok(TerminalBlockPointDomain {
        block: block.block,
        source_block: block.source_block,
        start: before_point(function, first.position)?,
        end: after_point(function, last.position)?
            .0
            .checked_add(1)
            .map(TerminalLiveRangePoint)
            .ok_or(TerminalLiveRangeError::PointOverflow { function })?,
    })
}

fn virtual_fragments(
    function: usize,
    block: &TerminalBlockLiveness,
    register: TerminalVirtualRegisterId,
) -> Result<Vec<TerminalLiveRangeFragment>, TerminalLiveRangeError> {
    let mut points = BTreeSet::new();
    for instruction in &block.instructions {
        if instruction.virtual_live_in.contains(&register)
            || instruction.virtual_uses.contains(&register)
        {
            points.insert(before_point(function, instruction.position)?);
        }
        if instruction.virtual_live_out.contains(&register)
            || instruction.virtual_defs.contains(&register)
        {
            points.insert(after_point(function, instruction.position)?);
        }
    }
    Ok(fragments_from_points(block.block, points))
}

fn build_unit(
    function: usize,
    liveness: &crate::TerminalFunctionLiveness,
    unit: RegisterUnitId,
) -> Result<TerminalArchitecturalUnitLiveRange, TerminalLiveRangeError> {
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
                    TerminalArchitecturalUnitActionKind::Use,
                    &instruction.unit_uses,
                    before,
                ),
                (
                    TerminalArchitecturalUnitActionKind::Def,
                    &instruction.unit_defs,
                    after,
                ),
                (
                    TerminalArchitecturalUnitActionKind::Clobber,
                    &instruction.unit_clobbers,
                    after,
                ),
            ] {
                if values.contains(&unit) {
                    actions.push(TerminalArchitecturalUnitAction {
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
    Ok(TerminalArchitecturalUnitLiveRange {
        unit,
        actions,
        fragments,
        edge_connectors,
    })
}

fn fragments_from_points(
    block: TerminalSelectedBlockId,
    points: BTreeSet<TerminalLiveRangePoint>,
) -> Vec<TerminalLiveRangeFragment> {
    let mut fragments = Vec::new();
    let mut iterator = points.into_iter();
    let Some(first) = iterator.next() else {
        return fragments;
    };
    let mut start = first;
    let mut last = first;
    for point in iterator {
        if last.0.checked_add(1) == Some(point.0) {
            last = point;
        } else {
            fragments.push(TerminalLiveRangeFragment {
                block,
                start,
                end: TerminalLiveRangePoint(last.0 + 1),
            });
            start = point;
            last = point;
        }
    }
    fragments.push(TerminalLiveRangeFragment {
        block,
        start,
        end: TerminalLiveRangePoint(last.0 + 1),
    });
    fragments
}

fn fragments_overlap(
    left: &[TerminalLiveRangeFragment],
    right: &[TerminalLiveRangeFragment],
) -> bool {
    left.iter().any(|left| {
        right.iter().any(|right| {
            left.block == right.block && left.start < right.end && right.start < left.end
        })
    })
}

fn connector(
    source: TerminalSelectedBlockId,
    edge: &crate::TerminalSuccessorLiveness,
) -> TerminalLiveRangeEdgeConnector {
    TerminalLiveRangeEdgeConnector {
        source,
        terminator: edge.terminator,
        polarity_ordinal: edge.polarity_ordinal,
        psi_edge: edge.psi_edge,
        target: edge.target,
    }
}

fn operand_point(
    function: usize,
    position: TerminalLivenessPosition,
    access: RegisterOperandAccess,
) -> Result<TerminalLiveRangePoint, TerminalLiveRangeError> {
    match access {
        RegisterOperandAccess::Use => before_point(function, position),
        RegisterOperandAccess::Def => after_point(function, position),
        RegisterOperandAccess::UseDef => Err(TerminalLiveRangeError::UnsupportedUseDef {
            function,
            instruction: 0,
            operand: 0,
        }),
    }
}

fn before_point(
    function: usize,
    position: TerminalLivenessPosition,
) -> Result<TerminalLiveRangePoint, TerminalLiveRangeError> {
    position
        .0
        .checked_mul(2)
        .map(TerminalLiveRangePoint)
        .ok_or(TerminalLiveRangeError::PointOverflow { function })
}

fn after_point(
    function: usize,
    position: TerminalLivenessPosition,
) -> Result<TerminalLiveRangePoint, TerminalLiveRangeError> {
    position
        .0
        .checked_mul(2)
        .and_then(|point| point.checked_add(1))
        .map(TerminalLiveRangePoint)
        .ok_or(TerminalLiveRangeError::PointOverflow { function })
}

fn reject_unsupported(
    function: usize,
    liveness: &crate::TerminalFunctionLiveness,
) -> Result<(), TerminalLiveRangeError> {
    for operand in &liveness.operand_positions {
        let error = if operand.access == RegisterOperandAccess::UseDef {
            Some(TerminalLiveRangeError::UnsupportedUseDef {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            })
        } else if operand.tied_to.is_some() {
            Some(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            })
        } else if operand.early_clobber {
            Some(TerminalLiveRangeError::UnsupportedEarlyClobber {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            })
        } else {
            None
        };
        if let Some(error) = error {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use omega_register_model::RegisterUnitId;
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalVirtualRegisterId,
    };
    use psi_core::{BlockId, MachineId};

    use super::{block_domain, build_unit, fragments_overlap, virtual_fragments};
    use crate::{
        TerminalBlockLiveness, TerminalFunctionLiveness, TerminalInstructionLiveness,
        TerminalLiveRangeFragment, TerminalLiveRangePoint, TerminalLivenessPosition,
    };

    fn instruction(
        position: u32,
        uses: &[u32],
        defs: &[u32],
        live_in: &[u32],
        live_out: &[u32],
    ) -> TerminalInstructionLiveness {
        TerminalInstructionLiveness {
            position: TerminalLivenessPosition(position),
            instruction: TerminalSelectedInstructionId(position),
            virtual_uses: uses
                .iter()
                .copied()
                .map(TerminalVirtualRegisterId)
                .collect(),
            virtual_defs: defs
                .iter()
                .copied()
                .map(TerminalVirtualRegisterId)
                .collect(),
            virtual_live_in: live_in
                .iter()
                .copied()
                .map(TerminalVirtualRegisterId)
                .collect(),
            virtual_live_out: live_out
                .iter()
                .copied()
                .map(TerminalVirtualRegisterId)
                .collect(),
            unit_uses: Vec::new(),
            unit_defs: Vec::new(),
            unit_clobbers: Vec::new(),
            unit_live_in: Vec::new(),
            unit_live_out: Vec::new(),
        }
    }

    fn block(id: u32, instructions: Vec<TerminalInstructionLiveness>) -> TerminalBlockLiveness {
        TerminalBlockLiveness {
            block: TerminalSelectedBlockId(id),
            source_block: BlockId::new(u64::from(id) + 1).unwrap(),
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_live_in: Vec::new(),
            unit_live_out: Vec::new(),
            instructions,
            successors: Vec::new(),
        }
    }

    #[test]
    fn conditional_fixture_has_exact_block_domains_and_virtual_fragments() {
        let blocks = [
            block(
                0,
                vec![
                    instruction(0, &[0], &[], &[0], &[]),
                    instruction(1, &[], &[], &[], &[]),
                ],
            ),
            block(
                1,
                vec![
                    instruction(2, &[], &[1], &[], &[1]),
                    instruction(3, &[1], &[], &[1], &[]),
                ],
            ),
            block(
                2,
                vec![
                    instruction(4, &[], &[2], &[], &[2]),
                    instruction(5, &[2], &[], &[2], &[]),
                ],
            ),
        ];
        assert_eq!(
            blocks
                .iter()
                .map(|row| {
                    let row = block_domain(0, row).unwrap();
                    (row.start.0, row.end.0)
                })
                .collect::<Vec<_>>(),
            vec![(0, 4), (4, 8), (8, 12)]
        );
        assert_eq!(
            virtual_fragments(0, &blocks[0], TerminalVirtualRegisterId(0)).unwrap(),
            vec![TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(0),
                start: TerminalLiveRangePoint(0),
                end: TerminalLiveRangePoint(1),
            }]
        );
        assert_eq!(
            virtual_fragments(0, &blocks[1], TerminalVirtualRegisterId(1)).unwrap(),
            vec![TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(1),
                start: TerminalLiveRangePoint(5),
                end: TerminalLiveRangePoint(7),
            }]
        );
        assert_eq!(
            virtual_fragments(0, &blocks[2], TerminalVirtualRegisterId(2)).unwrap(),
            vec![TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(2),
                start: TerminalLiveRangePoint(9),
                end: TerminalLiveRangePoint(11),
            }]
        );
        let v0 = virtual_fragments(0, &blocks[0], TerminalVirtualRegisterId(0)).unwrap();
        let v1 = virtual_fragments(0, &blocks[1], TerminalVirtualRegisterId(1)).unwrap();
        let v2 = virtual_fragments(0, &blocks[2], TerminalVirtualRegisterId(2)).unwrap();
        assert!(!fragments_overlap(&v0, &v1));
        assert!(!fragments_overlap(&v0, &v2));
        assert!(!fragments_overlap(&v1, &v2));
    }

    #[test]
    fn architectural_actions_do_not_turn_dead_machine_writes_into_live_state() {
        let unit = RegisterUnitId(7);
        let mut first = instruction(0, &[], &[], &[], &[]);
        first.unit_uses = vec![unit];
        first.unit_defs = vec![unit];
        first.unit_live_in = vec![unit];
        first.unit_live_out = vec![unit];
        let mut second = instruction(1, &[], &[], &[], &[]);
        second.unit_uses = vec![unit];
        second.unit_defs = vec![unit];
        second.unit_live_in = vec![unit];
        let function = TerminalFunctionLiveness {
            machine: MachineId::new(1).unwrap(),
            entry_definitions: Vec::new(),
            operand_positions: Vec::new(),
            blocks: vec![block(0, vec![first, second])],
        };
        let row = build_unit(0, &function, unit).unwrap();
        assert_eq!(row.actions.len(), 4);
        assert_eq!(row.actions[0].point, TerminalLiveRangePoint(0));
        assert_eq!(row.actions[1].point, TerminalLiveRangePoint(1));
        assert_eq!(row.actions[2].point, TerminalLiveRangePoint(2));
        assert_eq!(row.actions[3].point, TerminalLiveRangePoint(3));
        assert_eq!(
            row.fragments,
            vec![TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(0),
                start: TerminalLiveRangePoint(0),
                end: TerminalLiveRangePoint(3),
            }]
        );
        assert_eq!(row.actions[3].point, row.fragments[0].end);
    }
}
