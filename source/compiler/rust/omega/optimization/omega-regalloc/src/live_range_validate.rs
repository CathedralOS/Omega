use std::collections::BTreeSet;

use crate::{
    TerminalArchitecturalUnitAction, TerminalArchitecturalUnitActionKind,
    TerminalArchitecturalUnitLiveRange, TerminalBlockPointDomain, TerminalFunctionLiveRanges,
    TerminalLiveRangeEdgeConnector, TerminalLiveRangeError, TerminalLiveRangeFragment,
    TerminalLiveRangePlan, TerminalLiveRangePoint, TerminalLiveRangeValidationReceipt,
    TerminalVirtualFixedConstraint, TerminalVirtualFixedConstraintSite,
    TerminalVirtualInterference, TerminalVirtualLiveRange, TerminalVirtualOccurrence,
    ValidatedTerminalLiveRanges, ValidatedTerminalLiveness, terminal_live_range_identity,
    validate_terminal_liveness,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_terminal_selected_instructions::TerminalSelectedBlockId;

pub fn validate_terminal_live_ranges(
    selected: &impl crate::ValidatedTerminalSelectedAnalysis,
    liveness: &ValidatedTerminalLiveness,
    plan: TerminalLiveRangePlan,
) -> Result<ValidatedTerminalLiveRanges, TerminalLiveRangeError> {
    revalidate_liveness_custody(selected, liveness)?;
    if plan.selected != selected.selected_identity()
        || plan.liveness != liveness.receipt().identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || plan.target != selected.selected_plan().target
        || plan.functions.len() != selected.selected_plan().functions.len()
    {
        return Err(TerminalLiveRangeError::RootMismatch);
    }
    for (function_index, ((selected_function, live_function), actual)) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&liveness.plan().functions)
        .zip(&plan.functions)
        .enumerate()
    {
        let expected =
            independently_replay_function(function_index, selected_function, live_function)?;
        validate_canonical(function_index, actual)?;
        if actual.machine != expected.machine {
            return Err(TerminalLiveRangeError::FunctionMismatch {
                function: function_index,
            });
        }
        if actual.block_domains != expected.block_domains {
            let block = expected
                .block_domains
                .iter()
                .zip(&actual.block_domains)
                .find(|(expected, actual)| expected != actual)
                .map_or(0, |(expected, _)| expected.block.0);
            return Err(TerminalLiveRangeError::BlockDomainMismatch {
                function: function_index,
                block,
            });
        }
        if actual.virtual_registers != expected.virtual_registers {
            let register = expected
                .virtual_registers
                .iter()
                .zip(&actual.virtual_registers)
                .find(|(expected, actual)| expected != actual)
                .map_or(0, |(expected, _)| expected.virtual_register.0);
            return Err(TerminalLiveRangeError::VirtualRegisterMismatch {
                function: function_index,
                register,
            });
        }
        if actual.architectural_units != expected.architectural_units {
            let unit = expected
                .architectural_units
                .iter()
                .zip(&actual.architectural_units)
                .find(|(expected, actual)| expected != actual)
                .map_or(0, |(expected, _)| expected.unit.0);
            return Err(TerminalLiveRangeError::ArchitecturalUnitMismatch {
                function: function_index,
                unit,
            });
        }
        if actual.interference != expected.interference {
            return Err(TerminalLiveRangeError::InterferenceMismatch {
                function: function_index,
            });
        }
    }

    let receipt = TerminalLiveRangeValidationReceipt {
        identity: terminal_live_range_identity(&plan),
        selected: plan.selected,
        liveness: plan.liveness,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        function_count: plan.functions.len(),
        block_count: plan
            .functions
            .iter()
            .map(|row| row.block_domains.len())
            .sum(),
        virtual_register_count: plan
            .functions
            .iter()
            .map(|row| row.virtual_registers.len())
            .sum(),
        virtual_occurrence_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|row| row.occurrences.len())
            .sum(),
        fixed_constraint_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|row| row.fixed_constraints.len())
            .sum(),
        virtual_fragment_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|row| row.fragments.len())
            .sum(),
        architectural_unit_count: plan
            .functions
            .iter()
            .map(|row| row.architectural_units.len())
            .sum(),
        architectural_action_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.architectural_units)
            .map(|row| row.actions.len())
            .sum(),
        architectural_fragment_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.architectural_units)
            .map(|row| row.fragments.len())
            .sum(),
        virtual_edge_connector_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|range| range.edge_connectors.len())
            .sum(),
        architectural_edge_connector_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.architectural_units)
            .map(|range| range.edge_connectors.len())
            .sum(),
        interference_count: plan
            .functions
            .iter()
            .map(|row| row.interference.len())
            .sum(),
    };
    Ok(ValidatedTerminalLiveRanges { plan, receipt })
}

pub(crate) fn revalidate_liveness_custody(
    selected: &impl crate::ValidatedTerminalSelectedAnalysis,
    liveness: &ValidatedTerminalLiveness,
) -> Result<(), TerminalLiveRangeError> {
    let replayed = validate_terminal_liveness(selected, liveness.plan().clone())
        .map_err(TerminalLiveRangeError::LivenessRevalidation)?;
    if replayed.receipt() != liveness.receipt() {
        return Err(TerminalLiveRangeError::LivenessReceiptMismatch);
    }
    Ok(())
}

fn independently_replay_function(
    function: usize,
    selected: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    live: &crate::TerminalFunctionLiveness,
) -> Result<TerminalFunctionLiveRanges, TerminalLiveRangeError> {
    reject_constraints(function, live)?;
    let mut block_domains = Vec::new();
    for block in &live.blocks {
        let first =
            block
                .instructions
                .first()
                .ok_or(TerminalLiveRangeError::BlockDomainMismatch {
                    function,
                    block: block.block.0,
                })?;
        let last = block
            .instructions
            .last()
            .expect("first established nonempty");
        block_domains.push(TerminalBlockPointDomain {
            block: block.block,
            source_block: block.source_block,
            start: checked_before(function, first.position.0)?,
            end: TerminalLiveRangePoint(
                checked_after(function, last.position.0)?
                    .0
                    .checked_add(1)
                    .ok_or(TerminalLiveRangeError::PointOverflow { function })?,
            ),
        });
    }

    let mut virtual_registers = Vec::new();
    for register in &selected.virtual_registers {
        let mut occurrences = Vec::new();
        let mut fixed_constraints = Vec::new();
        if let Some(view) = register.entry_fixed_view {
            fixed_constraints.push(TerminalVirtualFixedConstraint {
                site: TerminalVirtualFixedConstraintSite::Entry,
                view,
            });
        }
        for operand in &live.operand_positions {
            if operand.virtual_register != register.id {
                continue;
            }
            let point = independently_operand_point(function, operand)?;
            occurrences.push(TerminalVirtualOccurrence {
                position: operand.position,
                point,
                instruction: operand.instruction,
                operand: operand.operand,
                access: operand.access,
            });
            if let Some(view) = operand.fixed_view {
                fixed_constraints.push(TerminalVirtualFixedConstraint {
                    site: TerminalVirtualFixedConstraintSite::Operand {
                        position: operand.position,
                        point,
                        instruction: operand.instruction,
                        operand: operand.operand,
                        access: operand.access,
                    },
                    view,
                });
            }
        }
        let mut fragments = Vec::new();
        let mut edge_connectors = Vec::new();
        for block in &live.blocks {
            let mut occupied = BTreeSet::new();
            for instruction in &block.instructions {
                if instruction
                    .virtual_live_in
                    .binary_search(&register.id)
                    .is_ok()
                    || instruction.virtual_uses.binary_search(&register.id).is_ok()
                {
                    occupied.insert(checked_before(function, instruction.position.0)?);
                }
                if instruction
                    .virtual_live_out
                    .binary_search(&register.id)
                    .is_ok()
                    || instruction.virtual_defs.binary_search(&register.id).is_ok()
                {
                    occupied.insert(checked_after(function, instruction.position.0)?);
                }
            }
            append_maximal_fragments(block.block, occupied, &mut fragments);
            edge_connectors.extend(
                block
                    .successors
                    .iter()
                    .filter(|edge| edge.virtual_live.binary_search(&register.id).is_ok())
                    .map(|edge| edge_row(block.block, edge)),
            );
        }
        virtual_registers.push(TerminalVirtualLiveRange {
            virtual_register: register.id,
            class: register.class,
            occurrences,
            fixed_constraints,
            fragments,
            edge_connectors,
        });
    }

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
    let mut architectural_units = Vec::new();
    for unit in discovered_units {
        architectural_units.push(independently_replay_unit(function, live, unit)?);
    }

    let mut interference = Vec::new();
    for left_index in 0..virtual_registers.len() {
        for right_index in (left_index + 1)..virtual_registers.len() {
            let left = &virtual_registers[left_index];
            let right = &virtual_registers[right_index];
            if independently_overlaps(&left.fragments, &right.fragments) {
                interference.push(TerminalVirtualInterference {
                    lower: left.virtual_register,
                    higher: right.virtual_register,
                });
            }
        }
    }
    Ok(TerminalFunctionLiveRanges {
        machine: selected.machine,
        block_domains,
        virtual_registers,
        architectural_units,
        interference,
    })
}

fn independently_replay_unit(
    function: usize,
    live: &crate::TerminalFunctionLiveness,
    unit: RegisterUnitId,
) -> Result<TerminalArchitecturalUnitLiveRange, TerminalLiveRangeError> {
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
                if rows.binary_search(&unit).is_ok() {
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
        append_maximal_fragments(block.block, occupied, &mut fragments);
        edge_connectors.extend(
            block
                .successors
                .iter()
                .filter(|edge| edge.unit_live.binary_search(&unit).is_ok())
                .map(|edge| edge_row(block.block, edge)),
        );
    }
    Ok(TerminalArchitecturalUnitLiveRange {
        unit,
        actions,
        fragments,
        edge_connectors,
    })
}

fn validate_canonical(
    function: usize,
    actual: &TerminalFunctionLiveRanges,
) -> Result<(), TerminalLiveRangeError> {
    if actual
        .block_domains
        .windows(2)
        .any(|rows| rows[0].block >= rows[1].block)
        || actual
            .virtual_registers
            .windows(2)
            .any(|rows| rows[0].virtual_register >= rows[1].virtual_register)
        || actual
            .architectural_units
            .windows(2)
            .any(|rows| rows[0].unit >= rows[1].unit)
        || actual
            .interference
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
        || actual
            .interference
            .iter()
            .any(|pair| pair.lower >= pair.higher)
    {
        return Err(TerminalLiveRangeError::NonCanonicalRows { function });
    }
    for range in &actual.virtual_registers {
        require_maximal_fragments(function, &range.fragments)?;
        require_ordered_connectors(function, &range.edge_connectors)?;
    }
    for range in &actual.architectural_units {
        require_maximal_fragments(function, &range.fragments)?;
        require_ordered_connectors(function, &range.edge_connectors)?;
    }
    Ok(())
}

fn require_maximal_fragments(
    function: usize,
    fragments: &[TerminalLiveRangeFragment],
) -> Result<(), TerminalLiveRangeError> {
    if fragments.iter().any(|row| row.start >= row.end)
        || fragments.windows(2).any(|rows| {
            rows[0].block > rows[1].block
                || (rows[0].block == rows[1].block && rows[0].end >= rows[1].start)
        })
    {
        return Err(TerminalLiveRangeError::NonCanonicalRows { function });
    }
    Ok(())
}

fn require_ordered_connectors(
    function: usize,
    connectors: &[TerminalLiveRangeEdgeConnector],
) -> Result<(), TerminalLiveRangeError> {
    if connectors.windows(2).any(|rows| {
        (rows[0].source, rows[0].polarity_ordinal) >= (rows[1].source, rows[1].polarity_ordinal)
    }) {
        return Err(TerminalLiveRangeError::NonCanonicalRows { function });
    }
    Ok(())
}

fn independently_operand_point(
    function: usize,
    operand: &crate::TerminalOperandPosition,
) -> Result<TerminalLiveRangePoint, TerminalLiveRangeError> {
    match operand.access {
        RegisterOperandAccess::Use => checked_before(function, operand.position.0),
        RegisterOperandAccess::Def => checked_after(function, operand.position.0),
        RegisterOperandAccess::UseDef => Err(TerminalLiveRangeError::UnsupportedUseDef {
            function,
            instruction: operand.instruction.0,
            operand: operand.operand,
        }),
    }
}

fn reject_constraints(
    function: usize,
    live: &crate::TerminalFunctionLiveness,
) -> Result<(), TerminalLiveRangeError> {
    for operand in &live.operand_positions {
        if operand.access == RegisterOperandAccess::UseDef {
            return Err(TerminalLiveRangeError::UnsupportedUseDef {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            });
        }
        if operand.tied_to.is_some() {
            return Err(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            });
        }
        if operand.early_clobber {
            return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            });
        }
    }
    Ok(())
}

fn checked_before(
    function: usize,
    position: u32,
) -> Result<TerminalLiveRangePoint, TerminalLiveRangeError> {
    position
        .checked_mul(2)
        .map(TerminalLiveRangePoint)
        .ok_or(TerminalLiveRangeError::PointOverflow { function })
}

fn checked_after(
    function: usize,
    position: u32,
) -> Result<TerminalLiveRangePoint, TerminalLiveRangeError> {
    position
        .checked_mul(2)
        .and_then(|point| point.checked_add(1))
        .map(TerminalLiveRangePoint)
        .ok_or(TerminalLiveRangeError::PointOverflow { function })
}

fn append_maximal_fragments(
    block: TerminalSelectedBlockId,
    occupied: BTreeSet<TerminalLiveRangePoint>,
    output: &mut Vec<TerminalLiveRangeFragment>,
) {
    let mut points = occupied.into_iter();
    let Some(first) = points.next() else { return };
    let mut start = first;
    let mut previous = first;
    for current in points {
        if previous.0.checked_add(1) == Some(current.0) {
            previous = current;
            continue;
        }
        output.push(TerminalLiveRangeFragment {
            block,
            start,
            end: TerminalLiveRangePoint(previous.0 + 1),
        });
        start = current;
        previous = current;
    }
    output.push(TerminalLiveRangeFragment {
        block,
        start,
        end: TerminalLiveRangePoint(previous.0 + 1),
    });
}

fn edge_row(
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

fn independently_overlaps(
    left: &[TerminalLiveRangeFragment],
    right: &[TerminalLiveRangeFragment],
) -> bool {
    for first in left {
        for second in right {
            if first.block == second.block
                && first.start.0 < second.end.0
                && second.start.0 < first.end.0
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use omega_register_model::{RegisterClassId, RegisterUnitId};
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalVirtualRegisterId,
    };
    use psi_core::MachineId;

    use super::validate_canonical;
    use crate::{
        TerminalArchitecturalUnitLiveRange, TerminalFunctionLiveRanges, TerminalLiveRangeError,
        TerminalLiveRangeFragment, TerminalLiveRangePoint, TerminalVirtualInterference,
        TerminalVirtualLiveRange,
    };

    fn function() -> TerminalFunctionLiveRanges {
        TerminalFunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: vec![TerminalVirtualLiveRange {
                virtual_register: TerminalVirtualRegisterId(0),
                class: RegisterClassId(0),
                occurrences: Vec::new(),
                fixed_constraints: Vec::new(),
                fragments: vec![TerminalLiveRangeFragment {
                    block: TerminalSelectedBlockId(0),
                    start: TerminalLiveRangePoint(0),
                    end: TerminalLiveRangePoint(1),
                }],
                edge_connectors: Vec::new(),
            }],
            architectural_units: vec![TerminalArchitecturalUnitLiveRange {
                unit: RegisterUnitId(0),
                actions: Vec::new(),
                fragments: Vec::new(),
                edge_connectors: Vec::new(),
            }],
            interference: Vec::new(),
        }
    }

    #[test]
    fn canonical_validation_rejects_nonmaximal_fragments_and_reversed_pairs() {
        let mut adjacent = function();
        adjacent.virtual_registers[0]
            .fragments
            .push(TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(0),
                start: TerminalLiveRangePoint(1),
                end: TerminalLiveRangePoint(2),
            });
        assert!(matches!(
            validate_canonical(0, &adjacent),
            Err(TerminalLiveRangeError::NonCanonicalRows { .. })
        ));

        let mut reversed = function();
        reversed.interference.push(TerminalVirtualInterference {
            lower: TerminalVirtualRegisterId(2),
            higher: TerminalVirtualRegisterId(1),
        });
        assert!(matches!(
            validate_canonical(0, &reversed),
            Err(TerminalLiveRangeError::NonCanonicalRows { .. })
        ));
    }
}
