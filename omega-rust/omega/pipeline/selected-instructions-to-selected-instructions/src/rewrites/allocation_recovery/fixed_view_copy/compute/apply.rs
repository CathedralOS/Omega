//! Selected-CFG copy insertion and operand rewriting mechanics.

use super::*;

pub(super) fn apply_copy(
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
            ..
        } = destination.site
        else {
            return Err(FixedViewCopyError::UnsupportedTransitionSite {
                function: function_index,
                register: copy.source_virtual_register.0,
            });
        };
        let destination_block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == destination.block)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?;
        let SelectedTerminator::Return {
            instruction: return_instruction,
            ..
        } = &mut destination_block.terminator
        else {
            return Err(FixedViewCopyError::NonLeafDestination {
                function: function_index,
                instruction: instruction.0,
            });
        };
        return_instruction
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
    if terminator_instruction_id(&block.terminator) != copy.before_instruction {
        return Err(FixedViewCopyError::InvalidInsertionSite {
            function: function_index,
            instruction: copy.before_instruction.0,
        });
    }
    function.virtual_registers.push(VirtualRegister {
        id: copy.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: match source.origin {
            VirtualRegisterOrigin::LegalizationTemporary { temporary, .. } => {
                VirtualRegisterOrigin::LegalizationTemporary {
                    instruction: copy.copy_instruction,
                    temporary,
                    source_value: copy.source_value,
                }
            }
            _ => VirtualRegisterOrigin::InstructionResult {
                instruction: copy.copy_instruction,
                source_value: copy.source_value,
            },
        },
        definition_site: copy.source_definition_site,
        entry_fixed_view: None,
    });
    block.instructions.push(SelectedInstruction {
        id: copy.copy_instruction,
        kind: SelectedInstructionKind::CopyI64,
        constraint: copy.copy_constraint,
        operands: vec![
            selected_operand(&row.operands[0], copy.source_virtual_register),
            selected_operand(&row.operands[1], copy.result_virtual_register),
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
    });
    Ok(())
}

fn terminator_instruction_id(terminator: &SelectedTerminator) -> SelectedInstructionId {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction.id,
    }
}

fn selected_operand(
    constraint: &register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

pub(super) fn is_u64(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Integer(integer)
            if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64
    )
}
