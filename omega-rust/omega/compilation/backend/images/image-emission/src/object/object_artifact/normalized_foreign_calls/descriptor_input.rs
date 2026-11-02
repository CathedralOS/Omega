//! Independently require the authenticated descriptor input's exact prefix
//! guard before its foreign instruction. Generic physical replay separately
//! binds these selected arithmetic instructions to their emitted trap bytes.
use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstructionKind as Kind,
    SelectedNormalizedForeignCall, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ScalarType};

pub(super) fn validate(
    function: &SelectedFunction,
    block: &SelectedBlock,
    record: &SelectedNormalizedForeignCall,
) -> Option<()> {
    let types = function
        .structural
        .as_ref()
        .map_or(&[][..], |contract| contract.structural_types.as_slice());
    let guard = target_operations_to_selected_instructions::descriptor_input_guard_for_call(
        &record.call.binding,
        types,
        &record.call.scalar_arguments,
        &record.call.structural_arguments,
        &record.call.result,
    )
    .ok()?;
    let Some(guard) = guard else {
        return Some(());
    };
    let operation = record.invocation_site.operation()?;
    let count = record
        .call
        .scalar_arguments
        .get(guard.count_scalar_argument)?
        .source
        .source_value();
    let call_position = block.instruction_index(record.instruction)?;
    let form = legalized_operations::TrappingForm {
        operation: legalized_operations::TrappingOperation::Subtract,
        carrier: legalized_operations::SaturatingCarrier::from_integer(guard.count_type)?,
    };
    let mut guards = block.instructions[..call_position]
        .iter()
        .enumerate()
        .filter(|(_, instruction)| {
            instruction.kind == (Kind::TrappingInteger { form })
                && instruction.provenance.operations.as_slice() == [operation]
                && instruction.provenance.values.as_slice() == [count]
        });
    let (guard_position, instruction) = guards.next()?;
    if guards.next().is_some() || !instruction.provenance.fuel.is_empty() {
        return None;
    }
    let [bound, input, output, scratch] = instruction.operands.as_slice() else {
        return None;
    };
    let input = function
        .virtual_registers
        .get(input.virtual_register.0 as usize)?;
    if input.scalar_type != ScalarType::Integer(guard.count_type)
        || !matches!(input.origin,
            VirtualRegisterOrigin::EntryParameter { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. }
            | VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == count)
    {
        return None;
    }
    let bound_register = function
        .virtual_registers
        .get(bound.virtual_register.0 as usize)?;
    let VirtualRegisterOrigin::InstructionScratch {
        instruction: materialize,
        operand: 0,
    } = bound_register.origin
    else {
        return None;
    };
    let materialize_position = block.instruction_index(materialize)?;
    let materialize = block.instructions.get(materialize_position)?;
    if materialize_position >= guard_position
        || materialize.kind
            != (Kind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(guard.max_bytes)),
            })
        || materialize.provenance.operations.as_slice() != [operation]
        || materialize.provenance.values.as_slice() != [count]
        || !materialize.provenance.fuel.is_empty()
        || materialize.operands.as_slice().first()?.virtual_register != bound.virtual_register
        || materialize.operands.len() != 1
        || bound_register.scalar_type != ScalarType::Integer(guard.count_type)
    {
        return None;
    }
    for (operand, expected) in [(output, 2), (scratch, 3)] {
        if function
            .virtual_registers
            .get(operand.virtual_register.0 as usize)?
            .origin
            != (VirtualRegisterOrigin::InstructionScratch {
                instruction: instruction.id,
                operand: expected,
            })
        {
            return None;
        }
    }
    Some(())
}
