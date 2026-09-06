//! Selected definitions, block parameters, and successor value uses.

use super::integrity::terminator_instruction;
use crate::selection::constraints::row;
use crate::selection::shared::*;

pub(super) fn validate_def_use(
    function_index: usize,
    function: &SelectedFunction,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let mut definitions = vec![0_u8; function.virtual_registers.len()];
    let entry_registers = function
        .virtual_registers
        .iter()
        .filter_map(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::EntryParameter { .. }
            )
            .then_some(register.id)
        })
        .collect::<BTreeSet<_>>();
    for register in &entry_registers {
        definitions[register.0 as usize] = 1;
    }
    for register in &function.virtual_registers {
        if let VirtualRegisterOrigin::BlockParameter { block, .. } = register.origin {
            if !function
                .blocks
                .iter()
                .any(|candidate| candidate.id == block)
            {
                return Err(SelectedInstructionError::NonCanonicalVirtualRegisters {
                    function: function_index,
                });
            }
            definitions[register.id.0 as usize] = 1;
        }
    }
    for block in &function.blocks {
        let mut available = entry_registers.clone();
        available.extend(function.virtual_registers.iter().filter_map(|register| {
            matches!(register.origin, VirtualRegisterOrigin::BlockParameter { block: owner, .. } if owner == block.id).then_some(register.id)
        }));
        for instruction in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
        {
            let row = row(catalog, instruction.constraint)?;
            for (operand, constraint) in instruction.operands.iter().zip(&row.operands) {
                let index = operand.virtual_register.0 as usize;
                if matches!(
                    constraint.access,
                    RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                ) && !available.contains(&operand.virtual_register)
                {
                    return Err(SelectedInstructionError::UseBeforeDefinition {
                        function: function_index,
                        instruction: instruction.id.0,
                        register: operand.virtual_register.0,
                    });
                }
                if matches!(
                    constraint.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                ) {
                    definitions[index] += 1;
                    if definitions[index] != 1 {
                        return Err(SelectedInstructionError::MultipleDefinitions {
                            function: function_index,
                            register: operand.virtual_register.0,
                        });
                    }
                    available.insert(operand.virtual_register);
                }
            }
        }
        let successors: Vec<_> = match &block.terminator {
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::Return { .. } => Vec::new(),
        };
        for successor in successors {
            for binding in &successor.bindings {
                let argument = function
                    .virtual_registers
                    .iter()
                    .find(|register| match register.origin {
                        VirtualRegisterOrigin::EntryParameter { source_value, .. }
                        | VirtualRegisterOrigin::InstructionResult { source_value, .. }
                        | VirtualRegisterOrigin::LegalizationTemporary { source_value, .. }
                        | VirtualRegisterOrigin::BlockParameter { source_value, .. } => {
                            source_value == binding.argument
                        }
                    })
                    .ok_or(SelectedInstructionError::NonCanonicalVirtualRegisters {
                        function: function_index,
                    })?;
                if !available.contains(&argument.id) || argument.scalar_type != binding.scalar_type
                {
                    return Err(SelectedInstructionError::UseBeforeDefinition {
                        function: function_index,
                        instruction: terminator_instruction(&block.terminator).id.0,
                        register: argument.id.0,
                    });
                }
            }
        }
    }
    if definitions.iter().any(|count| *count != 1) {
        return Err(SelectedInstructionError::MultipleDefinitions {
            function: function_index,
            register: definitions.iter().position(|count| *count != 1).unwrap() as u32,
        });
    }
    Ok(())
}
