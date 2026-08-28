use std::collections::BTreeSet;

use crate::{
    TerminalArchitecturalUnitAction, TerminalArchitecturalUnitActionKind,
    TerminalArchitecturalUnitLiveRange, TerminalBlockPointDomain, TerminalDistinctUseDefTie,
    TerminalEarlyClobberConstraint, TerminalEarlyClobberUse, TerminalFunctionLiveRanges,
    TerminalLiveRangeEdgeConnector, TerminalLiveRangeError, TerminalLiveRangeFragment,
    TerminalLiveRangePlan, TerminalLiveRangePoint, TerminalLiveRangeValidationReceipt,
    TerminalVirtualFixedConstraint, TerminalVirtualFixedConstraintSite,
    TerminalVirtualInterference, TerminalVirtualLiveRange, TerminalVirtualOccurrence,
    ValidatedTerminalLiveRanges, ValidatedTerminalLiveness, terminal_live_range_identity,
    validate_terminal_liveness,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_terminal_selected_instructions::{TerminalSelectedBlockId, TerminalVirtualRegisterId};

pub fn validate_terminal_live_ranges(
    selected: &impl crate::ValidatedTerminalSelectedAnalysis,
    liveness: &ValidatedTerminalLiveness,
    plan: TerminalLiveRangePlan,
) -> Result<ValidatedTerminalLiveRanges, TerminalLiveRangeError> {
    revalidate_liveness_custody(selected, liveness)?;
    if !liveness.plan().structural_unit_functions.is_empty() {
        return Err(TerminalLiveRangeError::UnsupportedStructuralUnitFunctions {
            count: liveness.plan().structural_unit_functions.len(),
        });
    }
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
        if actual.tied_pairs != expected.tied_pairs {
            return Err(TerminalLiveRangeError::TiedPairMismatch {
                function: function_index,
            });
        }
        require_early_clobber_rows(
            function_index,
            &actual.early_clobbers,
            &expected.early_clobbers,
        )?;
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
        tied_pair_count: plan.functions.iter().map(|row| row.tied_pairs.len()).sum(),
        tied_component_count: plan
            .functions
            .iter()
            .map(|row| tied_component_count(&row.tied_pairs))
            .sum(),
        early_clobber_count: plan
            .functions
            .iter()
            .map(|row| row.early_clobbers.len())
            .sum(),
        early_clobber_use_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.early_clobbers)
            .map(|row| row.uses.len())
            .sum(),
    };
    Ok(ValidatedTerminalLiveRanges { plan, receipt })
}

fn require_early_clobber_rows(
    function: usize,
    actual: &[TerminalEarlyClobberConstraint],
    expected: &[TerminalEarlyClobberConstraint],
) -> Result<(), TerminalLiveRangeError> {
    if actual != expected {
        return Err(TerminalLiveRangeError::EarlyClobberMismatch { function });
    }
    Ok(())
}

fn tied_component_count(ties: &[TerminalDistinctUseDefTie]) -> usize {
    let mut components =
        Vec::<BTreeSet<omega_terminal_selected_instructions::TerminalVirtualRegisterId>>::new();
    for tie in ties {
        let use_component = components
            .iter()
            .position(|component| component.contains(&tie.use_virtual_register));
        let def_component = components
            .iter()
            .position(|component| component.contains(&tie.def_virtual_register));
        match (use_component, def_component) {
            (None, None) => components.push(BTreeSet::from([
                tie.use_virtual_register,
                tie.def_virtual_register,
            ])),
            (Some(component), None) => {
                components[component].insert(tie.def_virtual_register);
            }
            (None, Some(component)) => {
                components[component].insert(tie.use_virtual_register);
            }
            (Some(left), Some(right)) if left != right => {
                let (keep, remove) = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                let removed = components.remove(remove);
                components[keep].extend(removed);
            }
            (Some(_), Some(_)) => {}
        }
    }
    components.len()
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
    let tied_pairs = independently_derive_ties(function, live)?;
    let early_clobbers = independently_derive_early_clobbers(function, live)?;
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
        tied_pairs,
        early_clobbers,
        architectural_units,
        interference,
    })
}

fn independently_derive_early_clobbers(
    function: usize,
    live: &crate::TerminalFunctionLiveness,
) -> Result<Vec<TerminalEarlyClobberConstraint>, TerminalLiveRangeError> {
    let mut rows = Vec::new();
    for block in &live.blocks {
        for instruction in &block.instructions {
            let operands = live
                .operand_positions
                .iter()
                .filter(|operand| operand.instruction == instruction.instruction)
                .collect::<Vec<_>>();
            let marked = operands
                .iter()
                .copied()
                .filter(|operand| operand.early_clobber)
                .collect::<Vec<_>>();
            let Some(definition) = marked.first().copied() else {
                continue;
            };
            if marked.len() != 1
                || definition.access != RegisterOperandAccess::Def
                || definition.position != instruction.position
                || operands.len() < 2
            {
                return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
                    function,
                    instruction: instruction.instruction.0,
                    operand: marked.get(1).copied().unwrap_or(definition).operand,
                });
            }
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
            if let Some(source) = tied_source
                && (source.access != RegisterOperandAccess::Use
                    || source.operand >= definition.operand
                    || source.virtual_register == definition.virtual_register
                    || source.class != definition.class
                    || source.tied_to.is_some())
            {
                return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
                    function,
                    instruction: definition.instruction.0,
                    operand: definition.operand,
                });
            }
            let mut seen = Vec::new();
            for operand in &operands {
                if seen.contains(&operand.virtual_register)
                    || (operand.operand != definition.operand
                        && (operand.access != RegisterOperandAccess::Use
                            || operand.tied_to.is_some()))
                {
                    return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
                        function,
                        instruction: definition.instruction.0,
                        operand: operand.operand,
                    });
                }
                seen.push(operand.virtual_register);
            }
            let tied_edges = live
                .operand_positions
                .iter()
                .filter(|operand| operand.tied_to.is_some())
                .filter_map(|tied_definition| {
                    live.operand_positions
                        .iter()
                        .find(|operand| {
                            operand.instruction == tied_definition.instruction
                                && Some(operand.operand) == tied_definition.tied_to
                        })
                        .map(|source| (source.virtual_register, tied_definition.virtual_register))
                })
                .collect::<Vec<_>>();
            let unrelated = operands
                .iter()
                .copied()
                .filter(|operand| {
                    operand.operand != definition.operand
                        && tied_source.is_none_or(|source| source.operand != operand.operand)
                })
                .collect::<Vec<_>>();
            let tied_components = independently_merge_tied_components(&tied_edges);
            // Independently replay the one-early-definition-per-component rule.
            let tie_component_has_one_early_definition = match tied_source {
                None => tied_edges.iter().all(|(left, right)| {
                    *left != definition.virtual_register && *right != definition.virtual_register
                }),
                Some(source) => {
                    let Some(component) = tied_components.iter().find(|component| {
                        component.contains(&source.virtual_register)
                            && component.contains(&definition.virtual_register)
                    }) else {
                        return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
                            function,
                            instruction: definition.instruction.0,
                            operand: definition.operand,
                        });
                    };
                    tied_edges.contains(&(source.virtual_register, definition.virtual_register))
                        && live
                            .operand_positions
                            .iter()
                            .filter(|candidate| {
                                candidate.early_clobber
                                    && candidate.tied_to.is_some()
                                    && component.contains(&candidate.virtual_register)
                            })
                            .count()
                            == 1
                }
            };
            if !tie_component_has_one_early_definition
                || tied_source.is_some() && unrelated.is_empty()
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
            let uses = operands
                .iter()
                .filter(|operand| {
                    operand.operand != definition.operand
                        && tied_source.is_none_or(|source| source.operand != operand.operand)
                })
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
                early_point: checked_before(function, definition.position.0)?,
                def_operand: definition.operand,
                def_virtual_register: definition.virtual_register,
                def_class: definition.class,
                def_point: checked_after(function, definition.position.0)?,
                uses,
            });
        }
    }
    if let Some(unmatched) = live.operand_positions.iter().find(|operand| {
        operand.early_clobber
            && !rows
                .iter()
                .any(|row| row.instruction == operand.instruction)
    }) {
        return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
            function,
            instruction: unmatched.instruction.0,
            operand: unmatched.operand,
        });
    }
    if rows.windows(2).any(|pair| pair[0] >= pair[1])
        && let Some(row) = rows.get(1)
    {
        return Err(TerminalLiveRangeError::UnsupportedEarlyClobber {
            function,
            instruction: row.instruction.0,
            operand: row.def_operand,
        });
    }
    Ok(rows)
}

fn independently_merge_tied_components(
    edges: &[(TerminalVirtualRegisterId, TerminalVirtualRegisterId)],
) -> Vec<BTreeSet<TerminalVirtualRegisterId>> {
    let mut components = Vec::<BTreeSet<_>>::new();
    for &(left, right) in edges {
        let mut joined = BTreeSet::from([left, right]);
        let mut retained = Vec::new();
        for component in components {
            if component.contains(&left) || component.contains(&right) {
                joined.extend(component);
            } else {
                retained.push(component);
            }
        }
        retained.push(joined);
        components = retained;
    }
    components
}

fn independently_derive_ties(
    function: usize,
    live: &crate::TerminalFunctionLiveness,
) -> Result<Vec<TerminalDistinctUseDefTie>, TerminalLiveRangeError> {
    let mut result = Vec::new();
    for definition in live
        .operand_positions
        .iter()
        .filter(|operand| operand.tied_to.is_some())
    {
        let matching = live
            .operand_positions
            .iter()
            .filter(|operand| operand.instruction == definition.instruction)
            .collect::<Vec<_>>();
        let Some(source) = matching
            .iter()
            .copied()
            .find(|operand| Some(operand.operand) == definition.tied_to)
        else {
            return Err(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            });
        };
        if source.access != RegisterOperandAccess::Use
            || definition.access != RegisterOperandAccess::Def
            || source.operand >= definition.operand
            || source.virtual_register == definition.virtual_register
            || source.class != definition.class
            || source.tied_to.is_some()
        {
            return Err(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            });
        }
        let block = live
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|row| row.instruction == definition.instruction)
            })
            .ok_or(TerminalLiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            })?;
        result.push(TerminalDistinctUseDefTie {
            block: block.block,
            position: definition.position,
            instruction: definition.instruction,
            use_operand: source.operand,
            use_virtual_register: source.virtual_register,
            use_point: checked_before(function, definition.position.0)?,
            def_operand: definition.operand,
            def_virtual_register: definition.virtual_register,
            def_point: checked_after(function, definition.position.0)?,
            class: definition.class,
        });
    }
    result.sort_unstable();
    Ok(result)
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
        || actual.tied_pairs.windows(2).any(|rows| rows[0] >= rows[1])
        || actual
            .early_clobbers
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
        || actual
            .early_clobbers
            .iter()
            .any(|row| row.uses.is_empty() || row.uses.windows(2).any(|uses| uses[0] >= uses[1]))
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
    use omega_register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalVirtualRegisterId,
    };
    use psi_core::{BlockId, MachineId};

    use super::{
        independently_derive_early_clobbers, require_early_clobber_rows, tied_component_count,
        validate_canonical,
    };
    use crate::{
        TerminalArchitecturalUnitLiveRange, TerminalBlockLiveness, TerminalFunctionLiveRanges,
        TerminalFunctionLiveness, TerminalInstructionLiveness, TerminalLiveRangeError,
        TerminalLiveRangeFragment, TerminalLiveRangePoint, TerminalLivenessPosition,
        TerminalOperandPosition, TerminalVirtualInterference, TerminalVirtualLiveRange,
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
            tied_pairs: Vec::new(),
            early_clobbers: Vec::new(),
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

    #[test]
    fn independent_tie_derivation_matches_production() {
        let instruction = TerminalInstructionLiveness {
            position: TerminalLivenessPosition(1),
            instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId(1),
            virtual_uses: vec![TerminalVirtualRegisterId(0)],
            virtual_defs: vec![TerminalVirtualRegisterId(1)],
            virtual_live_in: vec![TerminalVirtualRegisterId(0)],
            virtual_live_out: vec![TerminalVirtualRegisterId(1)],
            unit_uses: Vec::new(),
            unit_defs: Vec::new(),
            unit_clobbers: Vec::new(),
            unit_live_in: Vec::new(),
            unit_live_out: Vec::new(),
        };
        let live = TerminalFunctionLiveness {
            machine: MachineId::new(1).unwrap(),
            entry_definitions: Vec::new(),
            operand_positions: vec![
                TerminalOperandPosition {
                    position: TerminalLivenessPosition(1),
                    instruction: instruction.instruction,
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
                    instruction: instruction.instruction,
                    operand: 1,
                    virtual_register: TerminalVirtualRegisterId(1),
                    access: RegisterOperandAccess::Def,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: Some(0),
                    early_clobber: false,
                },
            ],
            blocks: vec![TerminalBlockLiveness {
                block: TerminalSelectedBlockId(0),
                source_block: BlockId::new(1).unwrap(),
                virtual_live_in: Vec::new(),
                virtual_live_out: Vec::new(),
                unit_live_in: Vec::new(),
                unit_live_out: Vec::new(),
                instructions: vec![instruction],
                successors: Vec::new(),
            }],
        };
        assert_eq!(
            super::independently_derive_ties(0, &live).unwrap(),
            crate::live_range_compute::derive_tied_pairs(0, &live).unwrap()
        );
    }

    #[test]
    fn multiple_early_clobber_rows_replay_and_reject_individual_corruption() {
        let selected = crate::compute::tests::supported_multiple_early_clobber_function();
        let live = crate::compute::compute_function(0, &selected).unwrap();
        let expected = crate::live_range_compute::derive_early_clobbers(0, &live).unwrap();
        let replayed = independently_derive_early_clobbers(0, &live).unwrap();
        assert_eq!(expected, replayed);
        assert_eq!(expected.len(), 2);

        let mut removed = expected.clone();
        removed.pop();
        assert_eq!(
            require_early_clobber_rows(0, &removed, &expected),
            Err(TerminalLiveRangeError::EarlyClobberMismatch { function: 0 })
        );

        let mut reordered = expected.clone();
        reordered.swap(0, 1);
        assert_eq!(
            require_early_clobber_rows(0, &reordered, &expected),
            Err(TerminalLiveRangeError::EarlyClobberMismatch { function: 0 })
        );

        let mut corrupt_point = expected.clone();
        corrupt_point[1].early_point = TerminalLiveRangePoint(99);
        assert_eq!(
            require_early_clobber_rows(0, &corrupt_point, &expected),
            Err(TerminalLiveRangeError::EarlyClobberMismatch { function: 0 })
        );
    }

    #[test]
    fn isolated_tied_early_clobber_replay_rejects_malformed_and_corrupt_rows() {
        let selected = crate::compute::tests::supported_isolated_tied_early_clobber_function();
        let live = crate::compute::compute_function(0, &selected).unwrap();
        let expected = crate::live_range_compute::derive_early_clobbers(0, &live).unwrap();
        let replayed = independently_derive_early_clobbers(0, &live).unwrap();
        assert_eq!(expected, replayed);
        assert_eq!(
            super::independently_derive_ties(0, &live).unwrap(),
            crate::live_range_compute::derive_tied_pairs(0, &live).unwrap()
        );
        assert_eq!(expected[0].uses.len(), 1);
        assert_eq!(
            expected[0].uses[0].virtual_register,
            TerminalVirtualRegisterId(1)
        );

        let mut tied_source_duplicated_as_hazard = expected.clone();
        tied_source_duplicated_as_hazard[0]
            .uses
            .push(crate::TerminalEarlyClobberUse {
                operand: 0,
                virtual_register: TerminalVirtualRegisterId(0),
                class: RegisterClassId(0),
            });
        assert_eq!(
            require_early_clobber_rows(0, &tied_source_duplicated_as_hazard, &expected),
            Err(TerminalLiveRangeError::EarlyClobberMismatch { function: 0 })
        );

        let mut no_unrelated = live.clone();
        no_unrelated
            .operand_positions
            .retain(|operand| operand.virtual_register != TerminalVirtualRegisterId(1));
        assert!(matches!(
            independently_derive_early_clobbers(0, &no_unrelated),
            Err(TerminalLiveRangeError::UnsupportedEarlyClobber { .. })
        ));

        let mut tied_unrelated = live;
        tied_unrelated
            .operand_positions
            .iter_mut()
            .find(|operand| operand.virtual_register == TerminalVirtualRegisterId(1))
            .unwrap()
            .tied_to = Some(0);
        assert!(matches!(
            independently_derive_early_clobbers(0, &tied_unrelated),
            Err(TerminalLiveRangeError::UnsupportedEarlyClobber { .. })
        ));
    }

    #[test]
    fn component_tied_early_clobber_replay_matches_and_rejects_a_second_early_member() {
        let selected = crate::compute::tests::supported_component_tied_early_clobber_function();
        let live = crate::compute::compute_function(0, &selected).unwrap();
        assert_eq!(
            independently_derive_early_clobbers(0, &live).unwrap(),
            crate::live_range_compute::derive_early_clobbers(0, &live).unwrap()
        );
        assert_eq!(
            super::independently_derive_ties(0, &live).unwrap(),
            crate::live_range_compute::derive_tied_pairs(0, &live).unwrap()
        );

        let mut two_early = live;
        two_early
            .operand_positions
            .iter_mut()
            .find(|operand| operand.virtual_register == TerminalVirtualRegisterId(1))
            .unwrap()
            .early_clobber = true;
        two_early.operand_positions.push(TerminalOperandPosition {
            position: TerminalLivenessPosition(0),
            instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId(0),
            operand: 2,
            virtual_register: TerminalVirtualRegisterId(4),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        assert!(matches!(
            independently_derive_early_clobbers(0, &two_early),
            Err(TerminalLiveRangeError::UnsupportedEarlyClobber { .. })
        ));
        assert!(matches!(
            crate::live_range_compute::derive_early_clobbers(0, &two_early),
            Err(TerminalLiveRangeError::UnsupportedEarlyClobber { .. })
        ));
    }

    #[test]
    fn tied_component_receipt_count_uses_transitive_closure() {
        let edge = |use_register, def_register, instruction| crate::TerminalDistinctUseDefTie {
            block: TerminalSelectedBlockId(0),
            position: TerminalLivenessPosition(instruction),
            instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId(
                instruction,
            ),
            use_operand: 0,
            use_virtual_register: TerminalVirtualRegisterId(use_register),
            use_point: TerminalLiveRangePoint(instruction * 2),
            def_operand: 1,
            def_virtual_register: TerminalVirtualRegisterId(def_register),
            def_point: TerminalLiveRangePoint(instruction * 2 + 1),
            class: RegisterClassId(0),
        };
        assert_eq!(
            tied_component_count(&[edge(0, 1, 0), edge(1, 2, 1), edge(3, 4, 2)]),
            2
        );
    }
}
