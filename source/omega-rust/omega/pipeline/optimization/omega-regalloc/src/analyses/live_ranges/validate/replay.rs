use std::collections::BTreeSet;

use crate::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    BlockPointDomain, DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse,
    FunctionLiveRanges, LiveRangeEdgeConnector, LiveRangeError, LiveRangeFragment, LiveRangePlan,
    LiveRangePoint, ValidatedLiveRanges, ValidatedLiveness, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualInterference, VirtualLiveRange, VirtualOccurrence,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_selected_instructions::{SelectedBlockId, VirtualRegisterId};

pub(super) fn replay_live_ranges(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
    plan: LiveRangePlan,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    if plan.selected != selected.selected_identity()
        || plan.liveness != liveness.receipt().identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || plan.target != selected.selected_plan().target
        || plan.functions.len() != selected.selected_plan().functions.len()
        || plan.structural_unit_functions.len()
            != selected.selected_plan().structural_unit_functions.len()
        || plan.structural_unit_functions.len() != liveness.plan().structural_unit_functions.len()
    {
        return Err(LiveRangeError::RootMismatch);
    }
    let mut machines = BTreeSet::new();
    for (function_index, function) in plan
        .functions
        .iter()
        .chain(&plan.structural_unit_functions)
        .enumerate()
    {
        if !machines.insert(function.machine) {
            return Err(LiveRangeError::FunctionMismatch {
                function: function_index,
            });
        }
    }
    for (function_index, ((selected_function, live_function), actual)) in selected
        .selected_plan()
        .structural_unit_functions
        .iter()
        .zip(&liveness.plan().structural_unit_functions)
        .zip(&plan.structural_unit_functions)
        .enumerate()
    {
        let expected = independently_replay_structural_function(
            function_index,
            selected_function.machine,
            live_function,
        )?;
        validate_canonical(function_index, actual)?;
        if actual != &expected {
            return Err(LiveRangeError::FunctionMismatch {
                function: function_index,
            });
        }
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
            return Err(LiveRangeError::FunctionMismatch {
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
            return Err(LiveRangeError::BlockDomainMismatch {
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
            return Err(LiveRangeError::VirtualRegisterMismatch {
                function: function_index,
                register,
            });
        }
        if actual.tied_pairs != expected.tied_pairs {
            return Err(LiveRangeError::TiedPairMismatch {
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
            return Err(LiveRangeError::ArchitecturalUnitMismatch {
                function: function_index,
                unit,
            });
        }
        if actual.interference != expected.interference {
            return Err(LiveRangeError::InterferenceMismatch {
                function: function_index,
            });
        }
    }

    let receipt = super::receipt::build_receipt(&plan);
    Ok(ValidatedLiveRanges { plan, receipt })
}

fn independently_replay_structural_function(
    function: usize,
    machine: psi_core::MachineId,
    live: &crate::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    if live.machine != machine
        || !live.entry_definitions.is_empty()
        || !live.operand_positions.is_empty()
    {
        return Err(LiveRangeError::FunctionMismatch { function });
    }
    let mut block_domains = Vec::new();
    for block in &live.blocks {
        let first = block
            .instructions
            .first()
            .ok_or(LiveRangeError::BlockDomainMismatch {
                function,
                block: block.block.0,
            })?;
        let last = block
            .instructions
            .last()
            .expect("first established nonempty");
        block_domains.push(BlockPointDomain {
            block: block.block,
            source_block: block.source_block,
            start: checked_before(function, first.position.0)?,
            end: LiveRangePoint(
                checked_after(function, last.position.0)?
                    .0
                    .checked_add(1)
                    .ok_or(LiveRangeError::PointOverflow { function })?,
            ),
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
    let architectural_units = discovered_units
        .into_iter()
        .map(|unit| independently_replay_unit(function, live, unit))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FunctionLiveRanges {
        machine,
        block_domains,
        virtual_registers: Vec::new(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units,
        interference: Vec::new(),
    })
}

fn require_early_clobber_rows(
    function: usize,
    actual: &[EarlyClobberConstraint],
    expected: &[EarlyClobberConstraint],
) -> Result<(), LiveRangeError> {
    if actual != expected {
        return Err(LiveRangeError::EarlyClobberMismatch { function });
    }
    Ok(())
}

fn independently_replay_function(
    function: usize,
    selected: &omega_selected_instructions::SelectedFunction,
    live: &crate::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    reject_constraints(function, live)?;
    let tied_pairs = independently_derive_ties(function, live)?;
    let early_clobbers = independently_derive_early_clobbers(function, live)?;
    let mut block_domains = Vec::new();
    for block in &live.blocks {
        let first = block
            .instructions
            .first()
            .ok_or(LiveRangeError::BlockDomainMismatch {
                function,
                block: block.block.0,
            })?;
        let last = block
            .instructions
            .last()
            .expect("first established nonempty");
        block_domains.push(BlockPointDomain {
            block: block.block,
            source_block: block.source_block,
            start: checked_before(function, first.position.0)?,
            end: LiveRangePoint(
                checked_after(function, last.position.0)?
                    .0
                    .checked_add(1)
                    .ok_or(LiveRangeError::PointOverflow { function })?,
            ),
        });
    }

    let mut virtual_registers = Vec::new();
    for register in &selected.virtual_registers {
        let mut occurrences = Vec::new();
        let mut fixed_constraints = Vec::new();
        if let Some(view) = register.entry_fixed_view {
            fixed_constraints.push(VirtualFixedConstraint {
                site: VirtualFixedConstraintSite::Entry,
                view,
            });
        }
        for operand in &live.operand_positions {
            if operand.virtual_register != register.id {
                continue;
            }
            let point = independently_operand_point(function, operand)?;
            occurrences.push(VirtualOccurrence {
                position: operand.position,
                point,
                instruction: operand.instruction,
                operand: operand.operand,
                access: operand.access,
            });
            if let Some(view) = operand.fixed_view {
                fixed_constraints.push(VirtualFixedConstraint {
                    site: VirtualFixedConstraintSite::Operand {
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
        virtual_registers.push(VirtualLiveRange {
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
                interference.push(VirtualInterference {
                    lower: left.virtual_register,
                    higher: right.virtual_register,
                });
            }
        }
    }
    Ok(FunctionLiveRanges {
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
    live: &crate::FunctionLiveness,
) -> Result<Vec<EarlyClobberConstraint>, LiveRangeError> {
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
                return Err(LiveRangeError::UnsupportedEarlyClobber {
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
                        .ok_or(LiveRangeError::UnsupportedEarlyClobber {
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
                return Err(LiveRangeError::UnsupportedEarlyClobber {
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
                    return Err(LiveRangeError::UnsupportedEarlyClobber {
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
                        return Err(LiveRangeError::UnsupportedEarlyClobber {
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
                return Err(LiveRangeError::UnsupportedEarlyClobber {
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
                .map(|operand| EarlyClobberUse {
                    operand: operand.operand,
                    virtual_register: operand.virtual_register,
                    class: operand.class,
                })
                .collect();
            rows.push(EarlyClobberConstraint {
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
        return Err(LiveRangeError::UnsupportedEarlyClobber {
            function,
            instruction: unmatched.instruction.0,
            operand: unmatched.operand,
        });
    }
    if rows.windows(2).any(|pair| pair[0] >= pair[1])
        && let Some(row) = rows.get(1)
    {
        return Err(LiveRangeError::UnsupportedEarlyClobber {
            function,
            instruction: row.instruction.0,
            operand: row.def_operand,
        });
    }
    Ok(rows)
}

fn independently_merge_tied_components(
    edges: &[(VirtualRegisterId, VirtualRegisterId)],
) -> Vec<BTreeSet<VirtualRegisterId>> {
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
    live: &crate::FunctionLiveness,
) -> Result<Vec<DistinctUseDefTie>, LiveRangeError> {
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
            return Err(LiveRangeError::UnsupportedTiedOperand {
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
            return Err(LiveRangeError::UnsupportedTiedOperand {
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
            .ok_or(LiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            })?;
        result.push(DistinctUseDefTie {
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
        append_maximal_fragments(block.block, occupied, &mut fragments);
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

fn validate_canonical(function: usize, actual: &FunctionLiveRanges) -> Result<(), LiveRangeError> {
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
        return Err(LiveRangeError::NonCanonicalRows { function });
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
    fragments: &[LiveRangeFragment],
) -> Result<(), LiveRangeError> {
    if fragments.iter().any(|row| row.start >= row.end)
        || fragments.windows(2).any(|rows| {
            rows[0].block > rows[1].block
                || (rows[0].block == rows[1].block && rows[0].end >= rows[1].start)
        })
    {
        return Err(LiveRangeError::NonCanonicalRows { function });
    }
    Ok(())
}

fn require_ordered_connectors(
    function: usize,
    connectors: &[LiveRangeEdgeConnector],
) -> Result<(), LiveRangeError> {
    if connectors.windows(2).any(|rows| {
        (rows[0].source, rows[0].polarity_ordinal) >= (rows[1].source, rows[1].polarity_ordinal)
    }) {
        return Err(LiveRangeError::NonCanonicalRows { function });
    }
    Ok(())
}

fn independently_operand_point(
    function: usize,
    operand: &crate::OperandPosition,
) -> Result<LiveRangePoint, LiveRangeError> {
    match operand.access {
        RegisterOperandAccess::Use => checked_before(function, operand.position.0),
        RegisterOperandAccess::Def => checked_after(function, operand.position.0),
        RegisterOperandAccess::UseDef => Err(LiveRangeError::UnsupportedUseDef {
            function,
            instruction: operand.instruction.0,
            operand: operand.operand,
        }),
    }
}

fn reject_constraints(
    function: usize,
    live: &crate::FunctionLiveness,
) -> Result<(), LiveRangeError> {
    for operand in &live.operand_positions {
        if operand.access == RegisterOperandAccess::UseDef {
            return Err(LiveRangeError::UnsupportedUseDef {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            });
        }
    }
    Ok(())
}

fn checked_before(function: usize, position: u32) -> Result<LiveRangePoint, LiveRangeError> {
    position
        .checked_mul(2)
        .map(LiveRangePoint)
        .ok_or(LiveRangeError::PointOverflow { function })
}

fn checked_after(function: usize, position: u32) -> Result<LiveRangePoint, LiveRangeError> {
    position
        .checked_mul(2)
        .and_then(|point| point.checked_add(1))
        .map(LiveRangePoint)
        .ok_or(LiveRangeError::PointOverflow { function })
}

fn append_maximal_fragments(
    block: SelectedBlockId,
    occupied: BTreeSet<LiveRangePoint>,
    output: &mut Vec<LiveRangeFragment>,
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
        output.push(LiveRangeFragment {
            block,
            start,
            end: LiveRangePoint(previous.0 + 1),
        });
        start = current;
        previous = current;
    }
    output.push(LiveRangeFragment {
        block,
        start,
        end: LiveRangePoint(previous.0 + 1),
    });
}

fn edge_row(source: SelectedBlockId, edge: &crate::SuccessorLiveness) -> LiveRangeEdgeConnector {
    LiveRangeEdgeConnector {
        source,
        terminator: edge.terminator,
        polarity_ordinal: edge.polarity_ordinal,
        psi_edge: edge.psi_edge,
        target: edge.target,
    }
}

fn independently_overlaps(left: &[LiveRangeFragment], right: &[LiveRangeFragment]) -> bool {
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
#[path = "tests.rs"]
mod tests;
