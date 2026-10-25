//! Tied-operand and early-clobber constraint derivation.
use super::{
    BTreeSet, DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, RegisterOperandAccess,
    VirtualRegisterId,
};
use super::{LiveRangeError, after_point, before_point};

pub(crate) fn derive_early_clobbers(
    function: usize,
    liveness: &selected_instructions::FunctionLiveness,
) -> Result<Vec<EarlyClobberConstraint>, LiveRangeError> {
    if !liveness
        .operand_positions
        .storage_slice()
        .iter()
        .any(|operand| operand.early_clobber)
    {
        return Ok(Vec::new());
    }
    let instruction_operands = || {
        liveness.blocks.iter().flat_map(|block| {
            block.instructions.iter().map(move |instruction| {
                (
                    block,
                    liveness
                        .operand_positions
                        .span_or_empty(instruction.operands),
                )
            })
        })
    };
    let early_definitions = instruction_operands()
        .flat_map(|(block, operands)| {
            operands
                .iter()
                .filter(|operand| operand.early_clobber)
                .map(move |definition| (block, operands, definition))
        })
        .collect::<Vec<_>>();
    if early_definitions.is_empty() {
        return Ok(Vec::new());
    }
    let tied_edges = instruction_operands()
        .flat_map(|(_, operands)| {
            operands
                .iter()
                .filter(|operand| operand.tied_to.is_some())
                .filter_map(move |definition| {
                    operands
                        .iter()
                        .find(|operand| Some(operand.operand) == definition.tied_to)
                        .map(|source| (source.virtual_register, definition.virtual_register))
                })
        })
        .collect::<Vec<_>>();
    let mut tied_registers = BTreeSet::new();
    for &(left, right) in &tied_edges {
        tied_registers.insert(left);
        tied_registers.insert(right);
    }
    let mut rows = Vec::with_capacity(early_definitions.len());
    for (block, operands, definition) in &early_definitions {
        let tied_source = match definition.tied_to {
            None => None,
            Some(source_operand) => Some(
                operands
                    .iter()
                    .find(|operand| operand.operand == source_operand)
                    .ok_or(LiveRangeError::UnsupportedEarlyClobber {
                        function,
                        instruction: definition.instruction.0,
                        operand: definition.operand,
                    })?,
            ),
        };
        let multiple_definitions = operands
            .iter()
            .filter(|operand| operand.access == RegisterOperandAccess::Def)
            .count()
            > 1;
        let source_is_valid = tied_source.is_none_or(|source| {
            source.access == RegisterOperandAccess::Use
                && source.operand < definition.operand
                && source.virtual_register != definition.virtual_register
                && source.class == definition.class
                && source.tied_to.is_none()
        });
        let is_unrelated = |operand: &selected_instructions::OperandPosition| {
            operand.operand != definition.operand
                && tied_source.is_none_or(|source| source.operand != operand.operand)
        };
        // SingleEarlyDefTiedComponentAgainstUntiedUsesV1 keeps the source in
        // ordinary tie evidence and only unrelated Uses in this hazard row.
        let tie_component_has_one_early_definition = tied_source.is_none_or(|source| {
            let component = tied_component(source.virtual_register, &tied_edges);
            tied_edges.contains(&(source.virtual_register, definition.virtual_register))
                && component.contains(&definition.virtual_register)
                && early_definitions
                    .iter()
                    .filter(|(_, _, candidate)| {
                        candidate.tied_to.is_some()
                            && component.contains(&candidate.virtual_register)
                    })
                    .count()
                    == 1
        });
        let untied_definition_is_free =
            tied_source.is_some() || !tied_registers.contains(&definition.virtual_register);
        if definition.access != RegisterOperandAccess::Def
            || !multiple_definitions
                && operands
                    .iter()
                    .filter(|operand| operand.early_clobber)
                    .count()
                    != 1
            || operands.len() < 2
            || !operands
                .iter()
                .any(|operand| operand.access == RegisterOperandAccess::Use)
            || multiple_definitions && operands.iter().any(|operand| operand.tied_to.is_some())
            || !source_is_valid
            || tied_source.is_some() && !operands.iter().any(&is_unrelated)
            || operands.iter().enumerate().any(|(idx, operand)| {
                (operand.operand != definition.operand
                    && ((operand.access != RegisterOperandAccess::Use
                        && !(multiple_definitions
                            && operand.access == RegisterOperandAccess::Def
                            && operand.early_clobber))
                        || operand.tied_to.is_some()))
                    || operands[..idx].iter().any(|prior| {
                        prior.virtual_register == operand.virtual_register
                            || prior.operand == operand.operand
                    })
            })
            || !tie_component_has_one_early_definition
            || !untied_definition_is_free
            || operands
                .iter()
                .filter(|operand| is_unrelated(operand))
                .any(|operand| tied_registers.contains(&operand.virtual_register))
        {
            return Err(LiveRangeError::UnsupportedEarlyClobber {
                function,
                instruction: definition.instruction.0,
                operand: definition.operand,
            });
        }
        let uses = operands
            .iter()
            // Other definitions interfere at the ordinary after point, even
            // when dead. Only incoming Uses belong to early-clobber hazards.
            .filter(|operand| is_unrelated(operand) && operand.access == RegisterOperandAccess::Use)
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
    liveness: &selected_instructions::FunctionLiveness,
) -> Result<Vec<DistinctUseDefTie>, LiveRangeError> {
    if !liveness
        .operand_positions
        .storage_slice()
        .iter()
        .any(|operand| operand.tied_to.is_some())
    {
        return Ok(Vec::new());
    }
    let mut pairs = Vec::new();
    for block in &liveness.blocks {
        for instruction in &block.instructions {
            let operands = liveness
                .operand_positions
                .span_or_empty(instruction.operands);
            for definition in operands.iter().filter(|operand| operand.tied_to.is_some()) {
                let Some(use_operand) = operands
                    .iter()
                    .find(|candidate| Some(candidate.operand) == definition.tied_to)
                else {
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
        }
    }
    pairs.sort_unstable();
    Ok(pairs)
}

#[cfg(test)]
mod tests;
