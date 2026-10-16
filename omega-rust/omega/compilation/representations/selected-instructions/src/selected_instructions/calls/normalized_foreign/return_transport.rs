//! Locate a foreign result's ABI normalization through selected value copies.
//!
//! Runtime splitting preserves a fixed result in a fresh virtual register before
//! later instructions. This query follows those exact effect-free copies; it
//! does not infer another call, normalization, or floating-control operation.
//! An external invocation has no return transport. Its last body instruction
//! must instead be followed by the exact edge's uncharged containment trap.
use crate::{
    LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedInstruction,
    SelectedInstructionKind, SelectedNormalizedForeignCall, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use register_model::RegisterOperandAccess;

/// Positions borrow the already validated selected block; no new custody is minted.
pub struct ReturnTransport {
    pub call_position: usize,
    pub restore_position: Option<usize>,
    pub normalization_position: usize,
    pub result_register: Option<VirtualRegisterId>,
}

pub fn resolve(
    function: &SelectedFunction,
    block: &SelectedBlock,
    record: &SelectedNormalizedForeignCall,
) -> Option<ReturnTransport> {
    let mut calls = block
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| instruction.id == record.instruction);
    let (call_position, call) = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    if let semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(edge) =
        record.invocation_site
    {
        if !matches!(
            record.call.result,
            target_operations::TargetBoundaryResult::Unit
        ) || record
            .call
            .binding
            .boundary_entry_plan
            .call
            .result
            .is_some()
            || !record.call.callbacks.is_empty()
            || !matches!(&block.terminator,
                crate::SelectedTerminator::HostedExitProcess { instruction, completion_edge }
                    if *completion_edge == edge
                    && instruction.kind == SelectedInstructionKind::Crash
                    && instruction.provenance.edges.as_slice() == [edge]
                    && instruction.provenance.operations.is_empty()
                    && instruction.provenance.values.is_empty()
                    && instruction.provenance.obligations.is_empty()
                    && instruction.provenance.fuel.is_empty()
                    && instruction.operands.is_empty())
            || call_position.checked_add(1)? != block.instructions.len()
        {
            return None;
        }
        return Some(ReturnTransport {
            call_position,
            restore_position: None,
            normalization_position: call_position.checked_add(1)?,
            result_register: None,
        });
    }
    let operation = record.invocation_site.operation()?;
    let mut result_register = if let Some(home) = record.call.result.scalar_home() {
        let output = call.operands.last()?;
        let value = function
            .virtual_registers
            .get(output.virtual_register.0 as usize)?;
        if output.access != RegisterOperandAccess::Def
            || value.scalar_type != home.scalar_type
            || value.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: call.id,
                    source_value: home.source_value,
                })
        {
            return None;
        }
        Some(output.virtual_register)
    } else {
        None
    };
    let mut cursor = call_position.checked_add(1)?;
    let mut copies = |cursor: &mut usize| -> Option<()> {
        let Some(home) = record.call.result.scalar_home() else {
            return Some(());
        };
        while let Some(instruction) = block.instructions.get(*cursor) {
            let Some(next) =
                copy_result(function, instruction, result_register?, home.source_value)
            else {
                break;
            };
            result_register = Some(next);
            *cursor = cursor.checked_add(1)?;
        }
        Some(())
    };
    copies(&mut cursor)?;
    let restore_position = if record.call.binding.same_stack_contribution().is_some() {
        let restore = block.instructions.get(cursor)?;
        if restore.kind
            != (SelectedInstructionKind::RestoreFloatingControl {
                slot: LocalStorageSlotId::Boundary { operation },
            })
            || restore.provenance.operations.as_slice() != [operation]
        {
            return None;
        }
        let position = cursor;
        cursor = cursor.checked_add(1)?;
        copies(&mut cursor)?;
        Some(position)
    } else {
        None
    };
    Some(ReturnTransport {
        call_position,
        restore_position,
        normalization_position: cursor,
        result_register,
    })
}

fn copy_result(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    source: VirtualRegisterId,
    source_value: semantic_vocabulary::ValueId,
) -> Option<VirtualRegisterId> {
    let [input, output] = instruction.operands.as_slice() else {
        return None;
    };
    if instruction.kind != SelectedInstructionKind::CopyI64
        || input.virtual_register != source
        || input.access != RegisterOperandAccess::Use
        || output.access != RegisterOperandAccess::Def
        || !instruction.implicit_uses.is_empty()
        || !instruction.implicit_defs.is_empty()
        || !instruction.clobbers.is_empty()
        || !instruction.provenance.operations.is_empty()
        || instruction.provenance.values.as_slice() != [source_value]
        || !instruction.provenance.edges.is_empty()
        || !instruction.provenance.obligations.is_empty()
        || !instruction.provenance.fuel.is_empty()
        || instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return None;
    }
    let input_value = function
        .virtual_registers
        .get(input.virtual_register.0 as usize)?;
    let output_value = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)?;
    if input_value.scalar_type != output_value.scalar_type
        || input_value.class != output_value.class
        || input_value.definition_site != output_value.definition_site
        || output_value.origin
            != (VirtualRegisterOrigin::InstructionResult {
                instruction: instruction.id,
                source_value,
            })
    {
        return None;
    }
    Some(output.virtual_register)
}
