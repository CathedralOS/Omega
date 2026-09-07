//! Selected SSA definitions, dominance, and explicit parallel edge transport.
use super::integrity::terminator_instruction;
use crate::selection::constraints::row;
use crate::selection::shared::*;
use selected_instructions::SelectedValueTransport;

pub(super) fn validate_def_use(
    function_index: usize,
    function: &SelectedFunction,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::NonCanonicalVirtualRegisters {
        function: function_index,
    };
    let count = function.blocks.len();
    let entry = function
        .blocks
        .iter()
        .position(|block| block.id == function.entry_block)
        .ok_or_else(invalid)?;
    let mut predecessors = vec![Vec::new(); count];
    for (source, block) in function.blocks.iter().enumerate() {
        for successor in successors(&block.terminator) {
            let destination = function
                .blocks
                .iter()
                .position(|block| block.id == successor.block)
                .ok_or_else(invalid)?;
            if function.blocks[destination].source_block != successor.source_target {
                return Err(invalid());
            }
            predecessors[destination].push(source);
        }
    }
    let all = (0..count).collect::<BTreeSet<_>>();
    let mut dominators = vec![all; count];
    dominators[entry] = BTreeSet::from([entry]);
    loop {
        let previous = dominators.clone();
        for block in 0..count {
            if block == entry {
                continue;
            }
            let Some(first) = predecessors[block].first() else {
                return Err(invalid());
            };
            let mut incoming = previous[*first].clone();
            for predecessor in &predecessors[block][1..] {
                incoming.retain(|candidate| previous[*predecessor].contains(candidate));
            }
            incoming.insert(block);
            dominators[block] = incoming;
        }
        if dominators == previous {
            break;
        }
    }
    // None is an ABI entry; a block parameter is defined before instruction zero.
    let mut definitions: Vec<Option<(Option<usize>, Option<usize>)>> =
        vec![None; function.virtual_registers.len()];
    for (position, register) in function.virtual_registers.iter().enumerate() {
        if register.id.0 as usize != position {
            return Err(invalid());
        }
        match register.origin {
            VirtualRegisterOrigin::EntryParameter { .. } => {
                definitions[position] = Some((None, None))
            }
            VirtualRegisterOrigin::BlockParameter { block, .. } => {
                let block = function
                    .blocks
                    .iter()
                    .position(|candidate| candidate.id == block)
                    .ok_or_else(invalid)?;
                definitions[position] = Some((Some(block), None));
            }
            _ => {}
        }
    }
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (position, instruction) in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
            .enumerate()
        {
            let constraint = row(catalog, instruction.constraint)?;
            for (operand, expected) in instruction.operands.iter().zip(&constraint.operands) {
                if matches!(
                    expected.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                ) {
                    let definition = definitions
                        .get_mut(operand.virtual_register.0 as usize)
                        .ok_or_else(invalid)?;
                    if definition
                        .replace((Some(block_index), Some(position)))
                        .is_some()
                    {
                        return Err(SelectedInstructionError::MultipleDefinitions {
                            function: function_index,
                            register: operand.virtual_register.0,
                        });
                    }
                }
            }
        }
    }
    let available = |register: VirtualRegisterId, block: usize, position: usize| match definitions
        .get(register.0 as usize)
        .copied()
        .flatten()
    {
        Some((None, _)) => true,
        Some((Some(owner), site)) if owner == block => {
            site.is_none_or(|defined| defined < position)
        }
        Some((Some(owner), _)) => dominators[block].contains(&owner),
        None => false,
    };
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (position, instruction) in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
            .enumerate()
        {
            let constraint = row(catalog, instruction.constraint)?;
            for (operand, expected) in instruction.operands.iter().zip(&constraint.operands) {
                if matches!(
                    expected.access,
                    RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                ) && !available(operand.virtual_register, block_index, position)
                {
                    return Err(SelectedInstructionError::UseBeforeDefinition {
                        function: function_index,
                        instruction: instruction.id.0,
                        register: operand.virtual_register.0,
                    });
                }
            }
        }
        for successor in successors(&block.terminator) {
            for binding in &successor.bindings {
                let semantic = binding.semantic;
                let destination = function.virtual_registers.iter().find(|register| {
                    matches!(register.origin,
                    VirtualRegisterOrigin::BlockParameter {source_value,block,..}
                        if source_value==semantic.parameter && block==successor.block)
                });
                match (binding.transport, destination) {
                    (SelectedValueTransport::Unused, None) => {}
                    (
                        SelectedValueTransport::Registers {
                            argument,
                            parameter,
                        },
                        Some(destination),
                    ) => {
                        let argument_row = function
                            .virtual_registers
                            .get(argument.0 as usize)
                            .ok_or_else(invalid)?;
                        if destination.id != parameter
                            || destination.scalar_type != semantic.scalar_type
                            || argument_row.scalar_type != semantic.scalar_type
                            || source_value(argument_row) != semantic.argument
                            || !available(argument, block_index, block.instructions.len() + 1)
                        {
                            return Err(invalid());
                        }
                    }
                    _ => return Err(invalid()),
                }
            }
        }
    }
    if let Some(position) = definitions.iter().position(Option::is_none) {
        return Err(SelectedInstructionError::MultipleDefinitions {
            function: function_index,
            register: position as u32,
        });
    }
    Ok(())
}

fn source_value(register: &VirtualRegister) -> ValueId {
    match register.origin {
        VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::InstructionResult { source_value, .. }
        | VirtualRegisterOrigin::LegalizationTemporary { source_value, .. }
        | VirtualRegisterOrigin::BlockParameter { source_value, .. } => source_value,
    }
}

fn successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::Return { .. } => Vec::new(),
    }
}
