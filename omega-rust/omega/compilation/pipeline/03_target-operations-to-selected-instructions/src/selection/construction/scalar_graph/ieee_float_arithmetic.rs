//! One selected compiler float call as one nearest-even IEEE form.
//! Operands (one or two, by the operation; IEEE payloads or integers) and the
//! result stay in general registers; the binary or unary constraint row gives
//! the target form its scratch floating registers.
use super::{
    Builder, LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use selected_instructions::IeeeFloatArithmeticForm;

pub(super) fn emit(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Builder<'_>,
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
    let form = IeeeFloatArithmeticForm {
        operation: arithmetic,
        format,
    };
    let scalar = ScalarType::IeeeFloat(format);
    let operand = arithmetic.operand_type(format);
    let values = std::iter::once(left).chain(right).collect::<Vec<_>>();
    if result.scalar_type != scalar || values.len() != form.operand_count() {
        return Err(SelectedInstructionError::custody());
    }
    let mut registers = Vec::with_capacity(values.len() + 1);
    for value in &values {
        let (_, register, _, value_type) = state
            .resolve(*value)
            .ok_or_else(SelectedInstructionError::custody)?;
        if value_type != operand {
            return Err(SelectedInstructionError::custody());
        }
        registers.push(register);
    }
    let keys = &state.constraints.keys;
    let key = if form.operand_count() == 2 {
        keys.ieee_float_arithmetic
    } else {
        keys.ieee_float_unary
    }
    .ok_or_else(SelectedInstructionError::custody)?;
    let output = state.register(result.value, result.definition_site, scalar)?;
    registers.push(output);
    state.emit(
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
        key,
        &registers,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: values.into_iter().chain([result.value]).collect(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
