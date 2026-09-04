//! Independent tied-operand and early-clobber constraint reconstruction.

use std::collections::BTreeSet;

use crate::{DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, LiveRangeError};
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::VirtualRegisterId;

use super::fragments::{checked_after, checked_before};

pub(super) fn reject_unsupported(
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

pub(super) fn derive_early_clobbers(
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
            let tied_components = merge_tied_components(&tied_edges);
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
