//! Native pointer preservation is a checked pre-call count bound. Unsigned
//! trapping subtraction reuses the target's existing arithmetic semantics:
//! `maximum - count` completes exactly when the prefix remains authorized.
use super::super::integer_arithmetic::instruction_scratch;
use super::super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
use crate::SelectedInstructionError;
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

pub(super) fn emit(
    call: &legalized_operations::LegalizedNormalizedForeignCall,
    operation: semantic_vocabulary::OperationId,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let Some(guard) = call.descriptor_input else {
        return Ok(());
    };
    let count = call
        .scalar_arguments
        .get(guard.count_scalar_argument)
        .ok_or_else(SelectedInstructionError::custody)?
        .source
        .source_value();
    let (_, count_register, _, scalar) = builder
        .resolve(count)
        .ok_or_else(SelectedInstructionError::custody)?;
    if scalar != ScalarType::Integer(guard.count_type) {
        return Err(SelectedInstructionError::custody());
    }
    let provenance = || SelectedInstructionProvenance {
        operations: vec![operation],
        values: vec![count],
        ..Default::default()
    };
    let bound = instruction_scratch(builder, 0, IntegerSign::Unsigned)?;
    builder.registers[bound.0 as usize].scalar_type = scalar;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(guard.max_bytes)),
        bound,
        provenance(),
    )?;
    let output = instruction_scratch(builder, 2, IntegerSign::Unsigned)?;
    builder.registers[output.0 as usize].scalar_type = scalar;
    let scratch = instruction_scratch(builder, 3, IntegerSign::Unsigned)?;
    let form = legalized_operations::TrappingForm {
        operation: legalized_operations::TrappingOperation::Subtract,
        carrier: legalized_operations::SaturatingCarrier::from_integer(guard.count_type)
            .ok_or_else(SelectedInstructionError::custody)?,
    };
    let key = builder
        .constraints
        .keys
        .for_semantic(selected_instructions::MachineSemanticKind::TrappingInteger(
            form,
        ))
        .ok_or_else(SelectedInstructionError::custody)?;
    builder.emit_integer(
        SelectedInstructionKind::TrappingInteger { form },
        key,
        &[bound, count_register, output, scratch],
        provenance(),
    )
}
