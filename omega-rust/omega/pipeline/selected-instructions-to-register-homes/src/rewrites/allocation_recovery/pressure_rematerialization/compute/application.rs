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

pub(super) fn apply(
    function_index: usize,
    function: &mut SelectedFunction,
    action: &PressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), PressureRematerializationError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == action.victim)
        .cloned()
        .ok_or(PressureRematerializationError::DecisionMismatch {
            function: function_index,
        })?;
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
    let new_instruction = SelectedInstruction {
        id: action.fresh_materialize,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: action.value,
        },
        constraint: action.materialize_constraint,
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
        .ok_or(PressureRematerializationError::DecisionMismatch {
            function: function_index,
        })?;
    for rewrite in &action.rewrites {
        rewrite_operand(
            function_index,
            block,
            action.victim,
            action.result_virtual_register,
            *rewrite,
        )?;
    }
    let first =
        action
            .rewrites
            .first()
            .ok_or(PressureRematerializationError::DecisionMismatch {
                function: function_index,
            })?;
    if let Some(index) = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == first.instruction)
    {
        block.instructions.insert(index, new_instruction);
    } else {
        let terminator_id = match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction.id,
        };
        if terminator_id != first.instruction {
            return Err(PressureRematerializationError::DecisionMismatch {
                function: function_index,
            });
        }
        block.instructions.push(new_instruction);
    }
    Ok(())
}

fn rewrite_operand(
    function: usize,
    block: &mut selected_instructions::SelectedBlock,
    victim: VirtualRegisterId,
    result: VirtualRegisterId,
    rewrite: PressureRematerializationRewrite,
) -> Result<(), PressureRematerializationError> {
    let instruction = block
        .instructions
        .iter_mut()
        .find(|instruction| instruction.id == rewrite.instruction)
        .or_else(|| {
            let terminator = match &mut block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                | SelectedTerminator::Return { instruction, .. } => instruction,
            };
            (terminator.id == rewrite.instruction).then_some(terminator)
        })
        .ok_or(PressureRematerializationError::DecisionMismatch { function })?;
    let operand = instruction
        .operands
        .iter_mut()
        .find(|operand| {
            operand.operand == rewrite.operand
                && operand.virtual_register == victim
                && operand.access == RegisterOperandAccess::Use
                && operand.fixed_view.is_none()
        })
        .ok_or(PressureRematerializationError::DecisionMismatch { function })?;
    operand.virtual_register = result;
    Ok(())
}
