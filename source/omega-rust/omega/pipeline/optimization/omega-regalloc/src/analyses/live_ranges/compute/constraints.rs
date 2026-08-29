//! Tied-operand and early-clobber constraint derivation.

use super::*;

pub(crate) fn derive_early_clobbers(
    function: usize,
    liveness: &crate::FunctionLiveness,
) -> Result<Vec<EarlyClobberConstraint>, LiveRangeError> {
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
                    .ok_or(LiveRangeError::UnsupportedEarlyClobber {
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
            return Err(LiveRangeError::UnsupportedEarlyClobber {
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
            .ok_or(LiveRangeError::UnsupportedEarlyClobber {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            })?;
        let uses = unrelated
            .into_iter()
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
    seed: VirtualRegisterId,
    edges: &[(VirtualRegisterId, VirtualRegisterId)],
) -> BTreeSet<VirtualRegisterId> {
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
    liveness: &crate::FunctionLiveness,
) -> Result<Vec<DistinctUseDefTie>, LiveRangeError> {
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
            return Err(LiveRangeError::UnsupportedTiedOperand {
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
            return Err(LiveRangeError::UnsupportedTiedOperand {
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
            .ok_or(LiveRangeError::UnsupportedTiedOperand {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            })?;
        pairs.push(DistinctUseDefTie {
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
