use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionKind, SelectedInstructionProvenance,
    SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};

use crate::{
    PressureRematerializationAction, PressureRematerializationError,
    PressureRematerializationRewrite,
};

use super::selected_structure;

pub(super) fn replay(
    index: usize,
    function: &mut SelectedFunction,
    action: &PressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), PressureRematerializationError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == action.victim)
        .cloned()
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    function.virtual_registers.push(VirtualRegister {
        id: action.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: action.fresh_materialize,
            source_value: action.source_value,
        },
        definition_site: source.definition_site,
        entry_fixed_view: None,
    });
    let inserted = SelectedInstruction {
        id: action.fresh_materialize,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: action.value,
        },
        constraint: row.key,
        operands: vec![selected_structure::operand(
            &row.operands[0],
            action.result_virtual_register,
        )],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            values: vec![action.source_value],
            ..Default::default()
        },
    };
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    for rewrite_row in &action.rewrites {
        let mut matched = 0usize;
        for instruction in &mut block.instructions {
            if instruction.id == rewrite_row.instruction {
                rewrite(
                    index,
                    instruction,
                    action.victim,
                    action.result_virtual_register,
                    *rewrite_row,
                )?;
                matched += 1;
            }
        }
        let terminator = match &mut block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        };
        if terminator.id == rewrite_row.instruction {
            rewrite(
                index,
                terminator,
                action.victim,
                action.result_virtual_register,
                *rewrite_row,
            )?;
            matched += 1;
        }
        if matched != 1 {
            return Err(PressureRematerializationError::DecisionMismatch { function: index });
        }
    }
    let first = action
        .rewrites
        .first()
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    if let Some(position) = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == first.instruction)
    {
        block.instructions.insert(position, inserted);
    } else {
        let terminator = match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        };
        if terminator.id != first.instruction {
            return Err(PressureRematerializationError::DecisionMismatch { function: index });
        }
        block.instructions.push(inserted);
    }
    Ok(())
}

fn rewrite(
    index: usize,
    instruction: &mut SelectedInstruction,
    victim: VirtualRegisterId,
    result: VirtualRegisterId,
    rewrite: PressureRematerializationRewrite,
) -> Result<(), PressureRematerializationError> {
    let operand = instruction
        .operands
        .iter_mut()
        .find(|operand| {
            operand.operand == rewrite.operand
                && operand.virtual_register == victim
                && operand.access == RegisterOperandAccess::Use
                && operand.fixed_view.is_none()
        })
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    operand.virtual_register = result;
    Ok(())
}
