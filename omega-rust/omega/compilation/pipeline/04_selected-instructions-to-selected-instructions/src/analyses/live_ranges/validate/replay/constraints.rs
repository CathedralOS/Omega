//! Independent tied-operand and early-clobber constraint reconstruction.

use std::collections::BTreeSet;

use crate::LiveRangeError;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, FunctionLiveness, OperandPosition,
    SelectedInstructionId, VirtualRegisterId,
};

use super::fragments::{checked_after, checked_before};

pub(super) fn reject_unsupported(
    function: usize,
    live: &selected_instructions::FunctionLiveness,
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

pub(super) fn derive_early_clobbers(
    function: usize,
    live: &selected_instructions::FunctionLiveness,
) -> Result<Vec<EarlyClobberConstraint>, LiveRangeError> {
    if !live
        .operand_positions
        .iter()
        .any(|operand| operand.early_clobber)
    {
        return Ok(Vec::new());
    }
    let grouped = InstructionOperands::new(live);
    let tied = TiedComponents::reconstruct(live, &grouped);
    let mut rows = Vec::new();
    for block in &live.blocks {
        for instruction in &block.instructions {
            let operands = grouped.for_instruction(instruction.instruction);
            let marked = operands
                .iter()
                .copied()
                .filter(|operand| operand.early_clobber)
                .collect::<Vec<_>>();
            let multiple_definitions = operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Def)
                .count()
                > 1;
            for definition in &marked {
                if (!multiple_definitions && marked.len() != 1)
                    || definition.access != RegisterOperandAccess::Def
                    || definition.position != instruction.position
                    || operands.len() < 2
                    || !operands
                        .iter()
                        .any(|operand| operand.access == RegisterOperandAccess::Use)
                    || multiple_definitions
                        && operands.iter().any(|operand| operand.tied_to.is_some())
                {
                    return Err(LiveRangeError::UnsupportedEarlyClobber {
                        function,
                        instruction: instruction.instruction.0,
                        operand: definition.operand,
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
                let mut ordinals = Vec::new();
                for operand in operands {
                    if seen.contains(&operand.virtual_register)
                        || ordinals.contains(&operand.operand)
                        || (operand.operand != definition.operand
                            && ((operand.access != RegisterOperandAccess::Use
                                && !(multiple_definitions
                                    && operand.access == RegisterOperandAccess::Def
                                    && operand.early_clobber))
                                || operand.tied_to.is_some()))
                    {
                        return Err(LiveRangeError::UnsupportedEarlyClobber {
                            function,
                            instruction: definition.instruction.0,
                            operand: operand.operand,
                        });
                    }
                    seen.push(operand.virtual_register);
                    ordinals.push(operand.operand);
                }
                let unrelated = operands
                    .iter()
                    .copied()
                    .filter(|operand| {
                        operand.operand != definition.operand
                            && tied_source.is_none_or(|source| source.operand != operand.operand)
                    })
                    .collect::<Vec<_>>();
                // Independently replay the one-early-definition-per-component rule.
                let tie_component_has_one_early_definition = match tied_source {
                    None => tied.component(definition.virtual_register).is_none(),
                    Some(source) => {
                        let Some(component) =
                            tied.component(source.virtual_register).filter(|component| {
                                Some(*component) == tied.component(definition.virtual_register)
                            })
                        else {
                            return Err(LiveRangeError::UnsupportedEarlyClobber {
                                function,
                                instruction: definition.instruction.0,
                                operand: definition.operand,
                            });
                        };
                        tied.edges
                            .binary_search(&(source.virtual_register, definition.virtual_register))
                            .is_ok()
                            && tied.early_definitions[component] == 1
                    }
                };
                if !tie_component_has_one_early_definition
                    || tied_source.is_some() && unrelated.is_empty()
                    || unrelated
                        .iter()
                        .any(|operand| tied.component(operand.virtual_register).is_some())
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
                        operand.access == RegisterOperandAccess::Use
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
    }
    let matched = rows
        .iter()
        .map(|row| (row.instruction, row.def_operand, row.def_virtual_register))
        .collect::<BTreeSet<_>>();
    if let Some(unmatched) = live.operand_positions.iter().find(|operand| {
        operand.early_clobber
            && !matched.contains(&(
                operand.instruction,
                operand.operand,
                operand.virtual_register,
            ))
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

/// Stable grouping keeps the original first-match and within-instruction order,
/// including noncontiguous operands in malformed-input controls. Neither sparse
/// instruction IDs nor virtual-register IDs determine scratch allocation size.
struct InstructionOperands<'input> {
    operands: Vec<&'input OperandPosition>,
}

impl<'input> InstructionOperands<'input> {
    fn new(live: &'input FunctionLiveness) -> Self {
        let mut operands = live.operand_positions.iter().collect::<Vec<_>>();
        operands.sort_by_key(|operand| operand.instruction);
        Self { operands }
    }

    fn for_instruction(&self, instruction: SelectedInstructionId) -> &[&'input OperandPosition] {
        let start = self
            .operands
            .partition_point(|operand| operand.instruction < instruction);
        let end = self
            .operands
            .partition_point(|operand| operand.instruction <= instruction);
        &self.operands[start..end]
    }
}

/// Preparation records relationships without admitting them or raising errors.
/// The original definition traversal still owns predicate and diagnostic order.
struct TiedComponents {
    edges: Vec<(VirtualRegisterId, VirtualRegisterId)>,
    members: Vec<(VirtualRegisterId, usize)>,
    early_definitions: Vec<usize>,
}

impl TiedComponents {
    fn reconstruct(live: &FunctionLiveness, grouped: &InstructionOperands<'_>) -> Self {
        let mut edges = live
            .operand_positions
            .iter()
            .filter(|operand| operand.tied_to.is_some())
            .filter_map(|definition| {
                grouped
                    .for_instruction(definition.instruction)
                    .iter()
                    .find(|operand| Some(operand.operand) == definition.tied_to)
                    .map(|source| (source.virtual_register, definition.virtual_register))
            })
            .collect::<Vec<_>>();
        let components = merge_tied_components(&edges);
        let mut members = components
            .iter()
            .enumerate()
            .flat_map(|(component, members)| {
                members.iter().map(move |register| (*register, component))
            })
            .collect::<Vec<_>>();
        members.sort_unstable_by_key(|(register, _)| *register);
        edges.sort_unstable();
        let mut tied = Self {
            edges,
            members,
            early_definitions: vec![0; components.len()],
        };
        for operand in &live.operand_positions {
            if operand.early_clobber
                && operand.tied_to.is_some()
                && let Some(component) = tied.component(operand.virtual_register)
            {
                tied.early_definitions[component] += 1;
            }
        }
        tied
    }

    fn component(&self, register: VirtualRegisterId) -> Option<usize> {
        self.members
            .binary_search_by_key(&register, |(member, _)| *member)
            .ok()
            .map(|position| self.members[position].1)
    }
}

fn merge_tied_components(
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

pub(super) fn derive_ties(
    function: usize,
    live: &selected_instructions::FunctionLiveness,
) -> Result<Vec<DistinctUseDefTie>, LiveRangeError> {
    if !live
        .operand_positions
        .iter()
        .any(|operand| operand.tied_to.is_some())
    {
        return Ok(Vec::new());
    }
    let grouped = InstructionOperands::new(live);
    let mut instruction_blocks = live
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(move |instruction| (instruction.instruction, block.block))
        })
        .collect::<Vec<_>>();
    // Stable sorting and deduplication preserve the original first-owner lookup.
    instruction_blocks.sort_by_key(|(instruction, _)| *instruction);
    instruction_blocks.dedup_by_key(|(instruction, _)| *instruction);
    let mut result = Vec::new();
    for definition in live
        .operand_positions
        .iter()
        .filter(|operand| operand.tied_to.is_some())
    {
        let matching = grouped.for_instruction(definition.instruction);
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
        let block = instruction_blocks
            .binary_search_by_key(&definition.instruction, |(instruction, _)| *instruction)
            .ok()
            .map(|position| instruction_blocks[position].1)
            .ok_or(LiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            })?;
        result.push(DistinctUseDefTie {
            block,
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
