//! Independent replay of one float-arithmetic form: rebuild the exact form,
//! operand registers, result register and provenance from the legal
//! instruction; never invoke the producer.
use super::{
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use selected_instructions::IeeeFloatArithmeticForm;

pub(super) fn validate(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Replay<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(SelectedInstructionError::custody)?;
    let LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
        operation: arithmetic,
        format,
        left,
        right,
        ..
    } = operation.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    let (_, left_register, _, left_type) = state
        .resolve(left)
        .ok_or_else(SelectedInstructionError::custody)?;
    let (_, right_register, _, right_type) = state
        .resolve(right)
        .ok_or_else(SelectedInstructionError::custody)?;
    let scalar = ScalarType::IeeeFloat(format);
    if result.scalar_type != scalar || left_type != scalar || right_type != scalar {
        return Err(SelectedInstructionError::custody());
    }
    let key = state
        .constraints
        .keys
        .ieee_float_arithmetic
        .ok_or_else(SelectedInstructionError::custody)?;
    let output = state.result_register(result.value, result.definition_site, scalar)?;
    state.check_instruction(
        SelectedInstructionKind::NearestIeeeFloatArithmetic {
            form: IeeeFloatArithmeticForm {
                operation: arithmetic,
                format,
            },
        },
        key,
        &[left_register, right_register, output],
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![left, right, result.value],
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
