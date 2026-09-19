use super::leaf_destination::terminator;
use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand,
    SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};

use crate::{FixedViewCopy, FixedViewCopyError, VirtualFixedConstraintSite};

pub(super) fn replay_apply(
    function_index: usize,
    function: &mut selected_instructions::SelectedFunction,
    copy: &FixedViewCopy,
    row: &RegisterInstructionConstraint,
) -> Result<(), FixedViewCopyError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == copy.source_virtual_register)
        .cloned()
        .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: copy.source_virtual_register.0,
        })?;
    for destination in &copy.destinations {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access: RegisterOperandAccess::Use,
            ..
        } = destination.site
        else {
            return Err(FixedViewCopyError::UnsupportedTransitionSite {
                function: function_index,
                register: copy.source_virtual_register.0,
            });
        };
        let block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == destination.block)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?;
        let site_instruction = if terminator(&block.terminator).id == instruction {
            terminator_mut(&mut block.terminator)
        } else {
            block
                .instructions
                .iter_mut()
                .find(|candidate| candidate.id == instruction)
                .ok_or(FixedViewCopyError::MissingDestination {
                    function: function_index,
                    instruction: instruction.0,
                })?
        };
        site_instruction
            .operands
            .iter_mut()
            .find(|candidate| candidate.operand == operand)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?
            .virtual_register = copy.result_virtual_register;
    }
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == copy.insertion_block)
        .ok_or(FixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: copy.before_instruction.0,
        })?;
    // The copy sits immediately before the fixed-use instruction: at the end
    // of the block's instruction list for a terminator site, or at the site's
    // own position for an ordinary instruction.
    let insertion_index = if terminator(&block.terminator).id == copy.before_instruction {
        block.instructions.len()
    } else {
        block
            .instructions
            .iter()
            .position(|candidate| candidate.id == copy.before_instruction)
            .ok_or(FixedViewCopyError::InvalidInsertionSite {
                function: function_index,
                instruction: copy.before_instruction.0,
            })?
    };
    function.virtual_registers.push(VirtualRegister {
        id: copy.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: copy.copy_instruction,
            source_value: copy.source_value,
        },
        definition_site: Some(copy.source_definition_site),
        entry_fixed_view: None,
    });
    let insertion_index =
        u32::try_from(insertion_index).map_err(|_| FixedViewCopyError::WorkOverflow)?;
    for settlement in &mut function.boundary_settlements {
        if settlement.block == block.id && settlement.instruction_index >= insertion_index {
            settlement.instruction_index = settlement
                .instruction_index
                .checked_add(1)
                .ok_or(FixedViewCopyError::WorkOverflow)?;
        }
    }
    block.instructions.insert(
        usize::try_from(insertion_index).map_err(|_| FixedViewCopyError::WorkOverflow)?,
        SelectedInstruction {
            id: copy.copy_instruction,
            kind: SelectedInstructionKind::CopyI64,
            constraint: copy.copy_constraint,
            operands: vec![
                replay_operand(&row.operands[0], copy.source_virtual_register),
                replay_operand(&row.operands[1], copy.result_virtual_register),
            ],
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: SelectedInstructionProvenance {
                operations: Vec::new(),
                values: vec![copy.source_value],
                edges: Vec::new(),
                obligations: Vec::new(),
                fuel: Vec::new(),
            },
        },
    );
    Ok(())
}

fn terminator_mut(terminator: &mut SelectedTerminator) -> &mut SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
    }
}

fn replay_operand(
    constraint: &RegisterOperandConstraint,
    virtual_register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}
