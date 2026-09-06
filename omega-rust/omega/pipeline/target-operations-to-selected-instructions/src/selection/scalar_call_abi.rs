//! Input-only join from the admitted register CallPlan to target constraint rows.

use super::shared::*;
use legalized_operations::LegalizedScalarCallUnitCall;
use register_environment::ValidatedTargetRegisterEnvironment;

pub(super) fn validate(
    function: usize,
    call: &LegalizedScalarCallUnitCall,
    key: RegisterConstraintKey,
    row: &RegisterInstructionConstraint,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    call.validate_shape().map_err(|_| invalid())?;
    let count = call.arguments.len();
    if environment.selected_keys().call_i64.get(count) != Some(&key)
        || environment.constraint(key) != Some(row)
        || row.key != key
        || call.call_plan.parameters.len() != count
        || row.operands.len() != count + 1
    {
        return Err(invalid());
    }
    let result = call.call_plan.result.as_ref().ok_or_else(invalid)?;
    for (index, (placement, operand)) in call
        .call_plan
        .parameters
        .iter()
        .chain(std::iter::once(result))
        .zip(&row.operands)
        .enumerate()
    {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] = placement.locations.as_slice()
        else {
            return Err(invalid());
        };
        if operand.fixed_view.is_none()
            || operand.fixed_view != environment.fixed_register_view(*register)
            || operand.access
                != if index == count {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                }
        {
            return Err(invalid());
        }
        if let Some(argument) = call.arguments.get(index)
            && (argument.parameter_index as usize != index || argument.placement != *placement)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
