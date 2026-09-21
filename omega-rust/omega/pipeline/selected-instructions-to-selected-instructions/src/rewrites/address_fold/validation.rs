use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{AddressFoldError, AddressFoldReceipt, ValidatedAddressFold, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: the validator locates the one
/// changed instruction by comparing the proposal to the source — the claimed
/// `access` must name that position — then audits the change's legality on
/// its own: the source instruction must be a clean displaced consumer whose
/// operand-0 register is last defined in-block by a clean `AddressOffset`,
/// the producer's base must survive the open interval, and the proposal must
/// carry exactly the combined displacement and rebound base the audit
/// derives. Restoring the changed instruction must return the complete
/// source — every other instruction, register, roster row, call, settlement,
/// and function included. The producer's `admission::admit` is never
/// consulted, so a wrong legality decision fails here even when the proposal
/// matches the edit the producer emitted.
pub fn validate_address_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedAddressFold, AddressFoldError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(AddressFoldError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(AddressFoldError::SourceMismatch)?;
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(AddressFoldError::ReplayMismatch)?;
    if proposed_function.blocks.len() != function.blocks.len() {
        return Err(AddressFoldError::ReplayMismatch);
    }
    // Locate the changed instruction by content, not by the producer's
    // coordinates: exactly one position may differ, and its identity must be
    // the claimed access.
    let mut change = None;
    for (block_index, (block, proposed_block)) in function
        .blocks
        .iter()
        .zip(proposed_function.blocks.iter())
        .enumerate()
    {
        if proposed_block.instructions.len() != block.instructions.len() {
            return Err(AddressFoldError::ReplayMismatch);
        }
        for (consumer_index, (instruction, proposed_instruction)) in block
            .instructions
            .iter()
            .zip(proposed_block.instructions.iter())
            .enumerate()
        {
            if proposed_instruction != instruction {
                if change.is_some() {
                    return Err(AddressFoldError::ReplayMismatch);
                }
                change = Some((block_index, consumer_index));
            }
        }
    }
    let (block_index, consumer_index) = change.ok_or(AddressFoldError::ReplayMismatch)?;
    let block = &function.blocks[block_index];
    let consumer = &block.instructions[consumer_index];
    if consumer.id != access {
        return Err(AddressFoldError::ReplayMismatch);
    }

    // The validator's own legality audit: the changed instruction's source
    // form must be the emitted two-operand addressing shape — operand zero a
    // referent base pointer `Use`, operand one the result `Def` (loads and
    // address projections) or stored value `Use` (referent store), with no
    // implicit unit traffic, fixed views, ties, or early clobbers.
    let (scale, second_access) =
        admission::consumer_shape(consumer.kind).ok_or(AddressFoldError::UnsupportedInstruction)?;
    if consumer.operands.len() != 2
        || !consumer.implicit_uses.is_empty()
        || !consumer.implicit_defs.is_empty()
        || !consumer.clobbers.is_empty()
    {
        return Err(AddressFoldError::UnsupportedInstruction);
    }
    let pointer_operand = &consumer.operands[0];
    let second_operand = &consumer.operands[1];
    if pointer_operand.operand != 0
        || pointer_operand.access != RegisterOperandAccess::Use
        || second_operand.operand != 1
        || second_operand.access != second_access
        || consumer.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(AddressFoldError::UnsupportedInstruction);
    }
    let pointer = pointer_operand.virtual_register;
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    let pointer_register = find_register(pointer).ok_or(AddressFoldError::UnsupportedProducer)?;
    let second_register =
        find_register(second_operand.virtual_register).ok_or(AddressFoldError::UnsupportedUse)?;
    // The consumer's declared constraint row must publish the same operand
    // shape and classes its operands carry, and each operand's class must
    // equal its roster row's.
    let row = environment
        .constraint(consumer.constraint)
        .ok_or(AddressFoldError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != pointer_operand.class
        || pointer_operand.class != pointer_register.class
        || row.operands[1].operand != 1
        || row.operands[1].access != second_access
        || row.operands[1].class != second_operand.class
        || second_operand.class != second_register.class
    {
        return Err(AddressFoldError::ConstraintMismatch);
    }
    // The pointer must come from the block's last definition before the
    // changed instruction, and that instruction must be a clean
    // `AddressOffset` `[use base, def pointer]` — a projection that rewrote
    // its own base reads the projected value, not the base.
    let producer_index =
        admission::last_definition_before(&block.instructions, consumer_index, pointer)
            .ok_or(AddressFoldError::UnsupportedProducer)?;
    let producer = &block.instructions[producer_index];
    let SelectedInstructionKind::AddressOffset {
        byte_offset: producer_offset,
    } = producer.kind
    else {
        return Err(AddressFoldError::UnsupportedProducer);
    };
    if producer.operands.len() != 2
        || !producer.implicit_uses.is_empty()
        || !producer.implicit_defs.is_empty()
        || !producer.clobbers.is_empty()
    {
        return Err(AddressFoldError::UnsupportedProducer);
    }
    let base_operand = &producer.operands[0];
    let result_operand = &producer.operands[1];
    if base_operand.operand != 0
        || base_operand.access != RegisterOperandAccess::Use
        || result_operand.operand != 1
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.virtual_register != pointer
        || producer.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(AddressFoldError::UnsupportedProducer);
    }
    let base = base_operand.virtual_register;
    if base == pointer {
        return Err(AddressFoldError::UnsupportedProducer);
    }
    let base_register = find_register(base).ok_or(AddressFoldError::UnsupportedUse)?;
    if base_register.class != pointer_operand.class {
        return Err(AddressFoldError::UnsupportedUse);
    }
    let producer_row = environment
        .constraint(producer.constraint)
        .ok_or(AddressFoldError::ConstraintMismatch)?;
    if producer_row.operands.len() != 2
        || producer_row.operands[0].operand != 0
        || producer_row.operands[0].access != RegisterOperandAccess::Use
        || producer_row.operands[0].class != base_operand.class
        || base_operand.class != base_register.class
        || producer_row.operands[1].operand != 1
        || producer_row.operands[1].access != RegisterOperandAccess::Def
        || producer_row.operands[1].class != result_operand.class
        || result_operand.class != pointer_register.class
    {
        return Err(AddressFoldError::ConstraintMismatch);
    }
    // Nothing strictly between the producer and the changed instruction may
    // redefine the base; operand uses read pre-definition, so the consumer's
    // own definitions and edge bindings are safe and only the open interval
    // is scanned.
    for instruction in &block.instructions[producer_index + 1..consumer_index] {
        if instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == base
        }) {
            return Err(AddressFoldError::UnsupportedUse);
        }
    }
    let byte_offset = match consumer.kind {
        SelectedInstructionKind::Load8 { byte_offset }
        | SelectedInstructionKind::Load16 { byte_offset }
        | SelectedInstructionKind::Load32 { byte_offset }
        | SelectedInstructionKind::Load64 { byte_offset }
        | SelectedInstructionKind::Store { byte_offset, .. }
        | SelectedInstructionKind::AddressOffset { byte_offset } => byte_offset,
        _ => return Err(AddressFoldError::UnsupportedInstruction),
    };
    let combined = byte_offset
        .checked_add(producer_offset)
        .ok_or(AddressFoldError::UnsupportedOffset)?;
    if !admission::admitted_offset(combined, scale, plan.target.architecture) {
        return Err(AddressFoldError::UnsupportedOffset);
    }
    let kind = match consumer.kind {
        SelectedInstructionKind::Load8 { .. } => SelectedInstructionKind::Load8 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load16 { .. } => SelectedInstructionKind::Load16 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load32 { .. } => SelectedInstructionKind::Load32 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load64 { .. } => SelectedInstructionKind::Load64 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Store { byte_size, .. } => SelectedInstructionKind::Store {
            byte_offset: combined,
            byte_size,
        },
        SelectedInstructionKind::AddressOffset { .. } => SelectedInstructionKind::AddressOffset {
            byte_offset: combined,
        },
        _ => return Err(AddressFoldError::UnsupportedInstruction),
    };
    // The changed instruction must equal exactly the fold the audit derives:
    // identity, constraint row, remaining operands, implicit surfaces, and
    // provenance all retained — only the kind's displacement and operand
    // zero's register change.
    let mut expected = consumer.clone();
    expected.kind = kind;
    expected.operands[0].virtual_register = base;
    if proposed_function.blocks[block_index].instructions[consumer_index] != expected {
        return Err(AddressFoldError::ReplayMismatch);
    }
    // Restoring the changed instruction must return the complete source:
    // every other function, block, terminator, register, call, and
    // settlement included.
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[block_index].instructions[consumer_index] =
        consumer.clone();
    if restored != *plan {
        return Err(AddressFoldError::ReplayMismatch);
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            // A backward scan of this block locates the pointer's last
            // definition; the interval scan then audits the instructions
            // between it and the consumer for a base redefinition.
            total
                .checked_add(block.instructions.len())?
                .checked_add(block.instructions.len())
        })
        .ok_or(AddressFoldError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| AddressFoldError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(AddressFoldError::WorkBudgetExceeded);
    }
    Ok(ValidatedAddressFold {
        receipt: AddressFoldReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
