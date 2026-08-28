use std::collections::BTreeSet;

use crate::{
    TerminalArchitecturalUnitAction, TerminalArchitecturalUnitActionKind,
    TerminalArchitecturalUnitLiveRange, TerminalBlockLiveness, TerminalBlockPointDomain,
    TerminalDistinctUseDefTie, TerminalEarlyClobberConstraint, TerminalEarlyClobberUse,
    TerminalFunctionLiveRanges, TerminalLiveRangeEdgeConnector, TerminalLiveRangeError,
    TerminalLiveRangeFragment, TerminalLiveRangePlan, TerminalLiveRangePoint,
    TerminalLivenessPosition, TerminalVirtualFixedConstraint, TerminalVirtualFixedConstraintSite,
    TerminalVirtualInterference, TerminalVirtualLiveRange, TerminalVirtualOccurrence,
    ValidatedTerminalLiveness,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_terminal_selected_instructions::{TerminalSelectedBlockId, TerminalVirtualRegisterId};

pub(crate) fn compute_terminal_live_ranges(
    selected: &impl crate::ValidatedTerminalSelectedAnalysis,
    liveness: &ValidatedTerminalLiveness,
) -> Result<TerminalLiveRangePlan, TerminalLiveRangeError> {
    let functions = selected
        .selected_plan()
        .functions
        .iter()
        .zip(&liveness.plan().functions)
        .enumerate()
        .map(|(index, (selected, live))| compute_function(index, selected, live))
        .collect::<Result<Vec<_>, _>>()?;
    let structural_unit_functions = selected
        .selected_plan()
        .structural_unit_functions
        .iter()
        .zip(&liveness.plan().structural_unit_functions)
        .enumerate()
        .map(|(index, (selected, live))| compute_structural_function(index, selected.machine, live))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalLiveRangePlan {
        selected: selected.selected_identity(),
        liveness: liveness.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: selected.selected_plan().target,
        functions,
        structural_unit_functions,
    })
}

fn compute_structural_function(
    function_index: usize,
    machine: psi_core::MachineId,
    liveness: &crate::TerminalFunctionLiveness,
) -> Result<TerminalFunctionLiveRanges, TerminalLiveRangeError> {
    if liveness.machine != machine
        || !liveness.entry_definitions.is_empty()
        || !liveness.operand_positions.is_empty()
    {
        return Err(TerminalLiveRangeError::FunctionMismatch {
            function: function_index,
        });
    }
    let block_domains = liveness
        .blocks
        .iter()
        .map(|block| block_domain(function_index, block))
        .collect::<Result<Vec<_>, _>>()?;
    let architectural_units = architectural_units(function_index, liveness)?;
    Ok(TerminalFunctionLiveRanges {
        machine,
        block_domains,
        virtual_registers: Vec::new(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units,
        interference: Vec::new(),
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
    let tied_pairs = derive_tied_pairs(function_index, liveness)?;
    let early_clobbers = derive_early_clobbers(function_index, liveness)?;

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

    let architectural_units = architectural_units(function_index, liveness)?;

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
        tied_pairs,
        early_clobbers,
        architectural_units,
        interference: interference.into_iter().collect(),
    })
}

fn architectural_units(
    function_index: usize,
    liveness: &crate::TerminalFunctionLiveness,
) -> Result<Vec<TerminalArchitecturalUnitLiveRange>, TerminalLiveRangeError> {
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

pub(crate) fn derive_early_clobbers(
    function: usize,
    liveness: &crate::TerminalFunctionLiveness,
) -> Result<Vec<TerminalEarlyClobberConstraint>, TerminalLiveRangeError> {
    let early_definitions = liveness
        .operand_positions
        .iter()
        .filter(|operand| operand.early_clobber)
        .collect::<Vec<_>>();
    let tied_edges = liveness
        .operand_positions
        .iter()
        .filter(|operand| operand.tied_to.is_some())
        .filter_map(|tied_definition| {
            liveness
                .operand_positions
                .iter()
                .find(|operand| {
                    operand.instruction == tied_definition.instruction
                        && Some(operand.operand) == tied_definition.tied_to
                })
                .map(|tied_use| (tied_use.virtual_register, tied_definition.virtual_register))
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(early_definitions.len());
    for definition in &early_definitions {
        let operands = liveness
            .operand_positions
            .iter()
            .filter(|operand| operand.instruction == definition.instruction)
            .collect::<Vec<_>>();
        let tied_source = match definition.tied_to {
            None => None,
            Some(source_operand) => Some(
                operands
                    .iter()
                    .copied()
                    .find(|operand| operand.operand == source_operand)
                    .ok_or(TerminalLiveRangeError::UnsupportedEarlyClobber {
                        function,
                        instruction: definition.instruction.0,
                        operand: definition.operand,
                    })?,
            ),
        };
        let mut participants = BTreeSet::new();
        let source_is_valid = tied_source.is_none_or(|source| {
            source.access == RegisterOperandAccess::Use
                && source.operand < definition.operand
                && source.virtual_register != definition.virtual_register
                && source.class == definition.class
                && source.tied_to.is_none()
        });
        let unrelated = operands
            .iter()
            .copied()
            .filter(|operand| {
                operand.operand != definition.operand
                    && tied_source.is_none_or(|source| source.operand != operand.operand)
            })
            .collect::<Vec<_>>();
        // SingleEarlyDefTiedComponentAgainstUntiedUsesV1 keeps the source in
        // ordinary tie evidence and only unrelated Uses in this hazard row.
        let tie_component_has_one_early_definition = tied_source.is_none_or(|source| {
            let component = tied_component(source.virtual_register, &tied_edges);
            tied_edges.contains(&(source.virtual_register, definition.virtual_register))
                && component.contains(&definition.virtual_register)
                && early_definitions
                    .iter()
                    .filter(|candidate| {
                        candidate.tied_to.is_some()
                            && component.contains(&candidate.virtual_register)
                    })
                    .count()
                    == 1
        });
        let untied_definition_is_free = tied_source.is_some()
            || tied_edges.iter().all(|(left, right)| {
                *left != definition.virtual_register && *right != definition.virtual_register
            });
        if definition.access != RegisterOperandAccess::Def
            || operands
                .iter()
                .filter(|operand| operand.early_clobber)
                .count()
                != 1
            || operands.len() < 2
            || !source_is_valid
            || tied_source.is_some() && unrelated.is_empty()
            || operands.iter().any(|operand| {
                (operand.operand != definition.operand
                    && (operand.access != RegisterOperandAccess::Use || operand.tied_to.is_some()))
                    || !participants.insert(operand.virtual_register)
            })
            || !tie_component_has_one_early_definition
            || !untied_definition_is_free
            || unrelated.iter().any(|operand| {
                tied_edges.iter().any(|(left, right)| {
                    *left == operand.virtual_register || *right == operand.virtual_register
                })
            })
        {
            return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            });
        }
        let block = liveness
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| instruction.instruction == definition.instruction)
            })
            .ok_or(TerminalLiveRangeError::UnsupportedEarlyClobber {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            })?;
        let uses = unrelated
            .into_iter()
            .map(|operand| TerminalEarlyClobberUse {
                operand: operand.operand,
                virtual_register: operand.virtual_register,
                class: operand.class,
            })
            .collect();
        rows.push(TerminalEarlyClobberConstraint {
            block: block.block,
            position: definition.position,
            instruction: definition.instruction,
            early_point: before_point(function, definition.position)?,
            def_operand: definition.operand,
            def_virtual_register: definition.virtual_register,
            def_class: definition.class,
            def_point: after_point(function, definition.position)?,
            uses,
        });
    }
    Ok(rows)
}

fn tied_component(
    seed: TerminalVirtualRegisterId,
    edges: &[(TerminalVirtualRegisterId, TerminalVirtualRegisterId)],
) -> BTreeSet<TerminalVirtualRegisterId> {
    let mut component = BTreeSet::from([seed]);
    loop {
        let previous_len = component.len();
        for (left, right) in edges {
            if component.contains(left) || component.contains(right) {
                component.extend([*left, *right]);
            }
        }
        if component.len() == previous_len {
            return component;
        }
    }
}

pub(crate) fn derive_tied_pairs(
    function: usize,
    liveness: &crate::TerminalFunctionLiveness,
) -> Result<Vec<TerminalDistinctUseDefTie>, TerminalLiveRangeError> {
    let mut pairs = Vec::new();
    for definition in liveness
        .operand_positions
        .iter()
        .filter(|operand| operand.tied_to.is_some())
    {
        let Some(use_operand) = liveness.operand_positions.iter().find(|candidate| {
            candidate.instruction == definition.instruction
                && Some(candidate.operand) == definition.tied_to
        }) else {
            return Err(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            });
        };
        if definition.access != RegisterOperandAccess::Def
            || use_operand.access != RegisterOperandAccess::Use
            || definition.operand <= use_operand.operand
            || definition.virtual_register == use_operand.virtual_register
            || definition.class != use_operand.class
            || use_operand.tied_to.is_some()
        {
            return Err(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            });
        }
        let block = liveness
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| instruction.instruction == definition.instruction)
            })
            .ok_or(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            })?;
        pairs.push(TerminalDistinctUseDefTie {
            block: block.block,
            position: definition.position,
            instruction: definition.instruction,
            use_operand: use_operand.operand,
            use_virtual_register: use_operand.virtual_register,
            use_point: before_point(function, definition.position)?,
            def_operand: definition.operand,
            def_virtual_register: definition.virtual_register,
            def_point: after_point(function, definition.position)?,
            class: definition.class,
        });
    }
    pairs.sort_unstable();
    Ok(pairs)
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
    use omega_register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalVirtualRegisterId,
    };
    use psi_core::{BlockId, MachineId};

    use super::{
        block_domain, build_unit, compute_structural_function, derive_early_clobbers,
        derive_tied_pairs, fragments_overlap, virtual_fragments,
    };
    use crate::{
        TerminalBlockLiveness, TerminalFunctionLiveness, TerminalInstructionLiveness,
        TerminalLiveRangeFragment, TerminalLiveRangePoint, TerminalLivenessPosition,
        TerminalOperandPosition,
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
    fn structural_unit_ranges_retain_architecture_without_inventing_virtuals() {
        let mut call = instruction(0, &[], &[], &[], &[]);
        call.unit_uses = vec![RegisterUnitId(1)];
        call.unit_defs = vec![RegisterUnitId(2)];
        call.unit_clobbers = vec![RegisterUnitId(3)];
        call.unit_live_in = vec![RegisterUnitId(1)];
        call.unit_live_out = vec![RegisterUnitId(2)];
        let mut returned = instruction(1, &[], &[], &[], &[]);
        returned.unit_uses = vec![RegisterUnitId(2)];
        returned.unit_live_in = vec![RegisterUnitId(2)];
        let live = TerminalFunctionLiveness {
            machine: MachineId::new(9).unwrap(),
            entry_definitions: Vec::new(),
            operand_positions: Vec::new(),
            blocks: vec![block(0, vec![call, returned])],
        };
        let ranges = compute_structural_function(0, live.machine, &live).unwrap();
        assert_eq!(ranges.machine, live.machine);
        assert!(ranges.virtual_registers.is_empty());
        assert!(ranges.tied_pairs.is_empty());
        assert!(ranges.early_clobbers.is_empty());
        assert!(ranges.interference.is_empty());
        assert_eq!(ranges.block_domains.len(), 1);
        assert_eq!(ranges.architectural_units.len(), 3);
        assert_eq!(ranges.architectural_units[0].actions.len(), 1);
        assert_eq!(ranges.architectural_units[1].actions.len(), 2);
        assert_eq!(ranges.architectural_units[2].actions.len(), 1);
    }

    #[test]
    fn distinct_use_def_tie_has_exact_before_and_after_points() {
        let live = TerminalFunctionLiveness {
            machine: MachineId::new(1).unwrap(),
            entry_definitions: Vec::new(),
            operand_positions: vec![
                TerminalOperandPosition {
                    position: TerminalLivenessPosition(1),
                    instruction: TerminalSelectedInstructionId(1),
                    operand: 0,
                    virtual_register: TerminalVirtualRegisterId(0),
                    access: RegisterOperandAccess::Use,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                },
                TerminalOperandPosition {
                    position: TerminalLivenessPosition(1),
                    instruction: TerminalSelectedInstructionId(1),
                    operand: 1,
                    virtual_register: TerminalVirtualRegisterId(1),
                    access: RegisterOperandAccess::Def,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: Some(0),
                    early_clobber: false,
                },
            ],
            blocks: vec![block(0, vec![instruction(1, &[0], &[1], &[0], &[1])])],
        };
        let ties = derive_tied_pairs(0, &live).unwrap();
        assert_eq!(ties.len(), 1);
        assert_eq!(ties[0].use_point, TerminalLiveRangePoint(2));
        assert_eq!(ties[0].def_point, TerminalLiveRangePoint(3));
        assert_eq!(ties[0].use_virtual_register, TerminalVirtualRegisterId(0));
        assert_eq!(ties[0].def_virtual_register, TerminalVirtualRegisterId(1));

        let mut chained = live;
        chained.operand_positions.extend([
            TerminalOperandPosition {
                position: TerminalLivenessPosition(2),
                instruction: TerminalSelectedInstructionId(2),
                operand: 0,
                virtual_register: TerminalVirtualRegisterId(1),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            TerminalOperandPosition {
                position: TerminalLivenessPosition(2),
                instruction: TerminalSelectedInstructionId(2),
                operand: 1,
                virtual_register: TerminalVirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: false,
            },
        ]);
        chained.blocks[0]
            .instructions
            .push(instruction(2, &[1], &[2], &[1], &[2]));
        let ties = derive_tied_pairs(0, &chained).unwrap();
        assert_eq!(ties.len(), 2);
        assert_eq!(ties[1].use_virtual_register, TerminalVirtualRegisterId(1));
        assert_eq!(ties[1].def_virtual_register, TerminalVirtualRegisterId(2));
        assert_eq!(ties[1].use_point, TerminalLiveRangePoint(4));
        assert_eq!(ties[1].def_point, TerminalLiveRangePoint(5));
    }

    #[test]
    fn early_clobber_retains_before_phase_without_extending_definition_liveness() {
        let live = TerminalFunctionLiveness {
            machine: MachineId::new(1).unwrap(),
            entry_definitions: Vec::new(),
            operand_positions: vec![
                TerminalOperandPosition {
                    position: TerminalLivenessPosition(1),
                    instruction: TerminalSelectedInstructionId(1),
                    operand: 0,
                    virtual_register: TerminalVirtualRegisterId(0),
                    access: RegisterOperandAccess::Use,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                },
                TerminalOperandPosition {
                    position: TerminalLivenessPosition(1),
                    instruction: TerminalSelectedInstructionId(1),
                    operand: 1,
                    virtual_register: TerminalVirtualRegisterId(1),
                    access: RegisterOperandAccess::Def,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: true,
                },
            ],
            blocks: vec![block(0, vec![instruction(1, &[0], &[1], &[0], &[1])])],
        };
        let rows = derive_early_clobbers(0, &live).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].early_point, TerminalLiveRangePoint(2));
        assert_eq!(rows[0].def_point, TerminalLiveRangePoint(3));
        assert_eq!(
            rows[0].uses[0].virtual_register,
            TerminalVirtualRegisterId(0)
        );
        assert_eq!(rows[0].def_virtual_register, TerminalVirtualRegisterId(1));
        assert_eq!(
            virtual_fragments(0, &live.blocks[0], TerminalVirtualRegisterId(1)).unwrap(),
            vec![TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(0),
                start: TerminalLiveRangePoint(3),
                end: TerminalLiveRangePoint(4),
            }]
        );

        let mut multiple = live;
        multiple.operand_positions.extend([
            TerminalOperandPosition {
                position: TerminalLivenessPosition(2),
                instruction: TerminalSelectedInstructionId(2),
                operand: 0,
                virtual_register: TerminalVirtualRegisterId(1),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            TerminalOperandPosition {
                position: TerminalLivenessPosition(2),
                instruction: TerminalSelectedInstructionId(2),
                operand: 1,
                virtual_register: TerminalVirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: true,
            },
        ]);
        multiple.blocks[0]
            .instructions
            .push(instruction(2, &[1], &[2], &[1], &[2]));
        let rows = derive_early_clobbers(0, &multiple).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].early_point, TerminalLiveRangePoint(4));
        assert_eq!(rows[1].def_point, TerminalLiveRangePoint(5));
        assert_eq!(
            rows[1].uses[0].virtual_register,
            TerminalVirtualRegisterId(1)
        );
        assert_eq!(rows[1].def_virtual_register, TerminalVirtualRegisterId(2));
    }

    #[test]
    fn isolated_tied_early_clobber_separates_tie_from_unrelated_hazard_uses() {
        let selected = crate::compute::tests::supported_isolated_tied_early_clobber_function();
        let live = crate::compute::compute_function(0, &selected).unwrap();
        let ties = derive_tied_pairs(0, &live).unwrap();
        let rows = derive_early_clobbers(0, &live).unwrap();

        assert_eq!(ties.len(), 1);
        assert_eq!(ties[0].use_virtual_register, TerminalVirtualRegisterId(0));
        assert_eq!(ties[0].def_virtual_register, TerminalVirtualRegisterId(2));
        assert_eq!(ties[0].use_point, TerminalLiveRangePoint(0));
        assert_eq!(ties[0].def_point, TerminalLiveRangePoint(1));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].early_point, TerminalLiveRangePoint(0));
        assert_eq!(rows[0].def_point, TerminalLiveRangePoint(1));
        assert_eq!(
            rows[0]
                .uses
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            vec![TerminalVirtualRegisterId(1)]
        );
        assert_eq!(
            virtual_fragments(0, &live.blocks[0], TerminalVirtualRegisterId(2)).unwrap(),
            vec![TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(0),
                start: TerminalLiveRangePoint(1),
                end: TerminalLiveRangePoint(2),
            }]
        );

        let selected =
            crate::compute::tests::supported_multiple_isolated_tied_early_clobber_function();
        let live = crate::compute::compute_function(0, &selected).unwrap();
        assert_eq!(derive_tied_pairs(0, &live).unwrap().len(), 2);
        assert_eq!(derive_early_clobbers(0, &live).unwrap().len(), 2);
    }

    #[test]
    fn component_tied_early_clobber_keeps_transitive_ties_and_only_unrelated_hazards() {
        let selected = crate::compute::tests::supported_component_tied_early_clobber_function();
        let live = crate::compute::compute_function(0, &selected).unwrap();
        let ties = derive_tied_pairs(0, &live).unwrap();
        let rows = derive_early_clobbers(0, &live).unwrap();

        assert_eq!(ties.len(), 2);
        assert_eq!(
            ties.iter()
                .map(|tie| (tie.use_virtual_register, tie.def_virtual_register))
                .collect::<Vec<_>>(),
            vec![
                (TerminalVirtualRegisterId(0), TerminalVirtualRegisterId(1)),
                (TerminalVirtualRegisterId(1), TerminalVirtualRegisterId(3)),
            ]
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].def_virtual_register, TerminalVirtualRegisterId(3));
        assert_eq!(rows[0].early_point, TerminalLiveRangePoint(2));
        assert_eq!(rows[0].def_point, TerminalLiveRangePoint(3));
        assert_eq!(
            rows[0]
                .uses
                .iter()
                .map(|used| used.virtual_register)
                .collect::<Vec<_>>(),
            vec![TerminalVirtualRegisterId(2)]
        );

        let multiple =
            crate::compute::tests::supported_multiple_component_tied_early_clobber_function();
        let live = crate::compute::compute_function(0, &multiple).unwrap();
        assert_eq!(derive_tied_pairs(0, &live).unwrap().len(), 4);
        assert_eq!(derive_early_clobbers(0, &live).unwrap().len(), 2);
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
