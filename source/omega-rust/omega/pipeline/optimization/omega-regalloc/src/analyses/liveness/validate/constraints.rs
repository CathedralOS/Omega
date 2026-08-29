//! Independent admission of supported operand-constraint shapes.

use super::shared::*;

pub(super) fn reject_v1_unsupported(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), LivenessError> {
    let mut tied_edges = Vec::new();
    let mut early_rows = Vec::new();
    for instruction in function.blocks.iter().flat_map(ordered_instructions) {
        for operand in &instruction.operands {
            if operand.access == RegisterOperandAccess::UseDef {
                return Err(LivenessError::UnsupportedUseDef {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: operand.operand,
                });
            }
        }
        let early = instruction
            .operands
            .iter()
            .filter(|operand| operand.early_clobber)
            .collect::<Vec<_>>();
        if let Some(definition) = early.first().copied() {
            let mut values = Vec::new();
            for operand in &instruction.operands {
                if values.contains(&operand.virtual_register) {
                    return Err(LivenessError::UnsupportedEarlyClobber {
                        function: function_index,
                        instruction: instruction.id.0,
                        operand: operand.operand,
                    });
                }
                values.push(operand.virtual_register);
            }
            let tied_source = definition.tied_to.and_then(|operand| {
                instruction
                    .operands
                    .iter()
                    .find(|candidate| candidate.operand == operand)
            });
            let source_is_valid = match tied_source {
                None => true,
                Some(source) => {
                    source.access == RegisterOperandAccess::Use
                        && source.operand < definition.operand
                        && source.virtual_register != definition.virtual_register
                        && source.class == definition.class
                        && source.tied_to.is_none()
                }
            };
            let unrelated = instruction
                .operands
                .iter()
                .filter(|operand| {
                    operand.operand != definition.operand
                        && tied_source.is_none_or(|source| source.operand != operand.operand)
                })
                .collect::<Vec<_>>();
            if early.len() != 1
                || definition.access != RegisterOperandAccess::Def
                || instruction.operands.len() < 2
                || !source_is_valid
                || tied_source.is_some() && unrelated.is_empty()
                || instruction.operands.iter().any(|operand| {
                    operand.operand != definition.operand
                        && (operand.access != RegisterOperandAccess::Use
                            || operand.tied_to.is_some())
                })
            {
                return Err(LivenessError::UnsupportedEarlyClobber {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: early.get(1).copied().unwrap_or(definition).operand,
                });
            }
            early_rows.push((
                instruction.id.0,
                definition.operand,
                definition.virtual_register,
                tied_source.map(|source| source.virtual_register),
                unrelated
                    .into_iter()
                    .map(|operand| (operand.virtual_register, operand.operand))
                    .collect::<Vec<_>>(),
            ));
        }
        let tied = instruction
            .operands
            .iter()
            .filter(|operand| operand.tied_to.is_some())
            .collect::<Vec<_>>();
        for definition in tied {
            let Some(use_operand) = instruction
                .operands
                .iter()
                .find(|operand| Some(operand.operand) == definition.tied_to)
            else {
                return Err(LivenessError::UnsupportedTiedOperand {
                    function: function_index,
                    instruction: instruction.id.0,
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
                return Err(LivenessError::UnsupportedTiedOperand {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: definition.operand,
                });
            }
            tied_edges.push((use_operand.virtual_register, definition.virtual_register));
        }
    }
    let components = independently_merge_tied_components(&tied_edges);
    // Independently replay SingleEarlyDefTiedComponentAgainstUntiedUsesV1.
    for (instruction, def_operand, definition, tied_source, unrelated) in &early_rows {
        let source_and_definition_share_one_early_component = match tied_source {
            None => tied_edges
                .iter()
                .all(|(left, right)| left != definition && right != definition),
            Some(source) => {
                let Some(component) = components
                    .iter()
                    .find(|component| component.contains(source) && component.contains(definition))
                else {
                    return Err(LivenessError::UnsupportedEarlyClobber {
                        function: function_index,
                        instruction: *instruction,
                        operand: *def_operand,
                    });
                };
                tied_edges.contains(&(*source, *definition))
                    && early_rows
                        .iter()
                        .filter(|(_, _, candidate, candidate_source, _)| {
                            candidate_source.is_some() && component.contains(candidate)
                        })
                        .count()
                        == 1
            }
        };
        let related_unrelated_operand = unrelated.iter().find(|(register, _)| {
            tied_edges
                .iter()
                .any(|(left, right)| left == register || right == register)
        });
        if !source_and_definition_share_one_early_component || related_unrelated_operand.is_some() {
            return Err(LivenessError::UnsupportedEarlyClobber {
                function: function_index,
                instruction: *instruction,
                operand: related_unrelated_operand.map_or(*def_operand, |(_, operand)| *operand),
            });
        }
    }
    Ok(())
}

fn independently_merge_tied_components(
    edges: &[(VirtualRegisterId, VirtualRegisterId)],
) -> Vec<BTreeSet<VirtualRegisterId>> {
    let mut components = Vec::<BTreeSet<_>>::new();
    for (left, right) in edges {
        let left_component = components
            .iter()
            .position(|component| component.contains(left));
        let right_component = components
            .iter()
            .position(|component| component.contains(right));
        match (left_component, right_component) {
            (None, None) => components.push(BTreeSet::from([*left, *right])),
            (Some(component), None) => {
                components[component].insert(*right);
            }
            (None, Some(component)) => {
                components[component].insert(*left);
            }
            (Some(left_component), Some(right_component)) if left_component != right_component => {
                let (keep, remove) = if left_component < right_component {
                    (left_component, right_component)
                } else {
                    (right_component, left_component)
                };
                let removed = components.remove(remove);
                components[keep].extend(removed);
            }
            (Some(_), Some(_)) => {}
        }
    }
    components
}
