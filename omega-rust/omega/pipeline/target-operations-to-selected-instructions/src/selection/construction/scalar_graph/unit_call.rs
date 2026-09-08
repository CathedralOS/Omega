//! Resultless calls copy scalar arguments and transport original borrowed pointers.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::SelectedCallContract;

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(invalid());
    };
    if operation.result.is_some() || call.result_placement.is_some() {
        return Err(invalid());
    }
    call.validate_source(&operation.ownership)
        .map_err(|_| invalid())?;
    let key = builder
        .constraints
        .keys
        .call_unit
        .get(crate::selection::scalar_call_abi::register_argument_count(
            call,
        ))
        .copied()
        .ok_or_else(invalid)?;
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(builder.catalog, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        match argument {
            LegalizedScalarArgument::Structural { semantic, target } => {
                if let Some(pointer) = super::scalar_call::argument_pointer(
                    builder,
                    operation,
                    argument_index,
                    semantic,
                    target,
                )? {
                    operands.push(pointer);
                }
            }
            LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } => {
                let (_, input, site, scalar_type) = builder.resolve(*value).ok_or_else(invalid)?;
                if crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                    != Some(placement.shape)
                {
                    return Err(invalid());
                }
                operands.push(builder.copy(input, *value, site, scalar_type)?);
            }
        }
    }
    builder.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        ),
        operation: operation.operation,
        call: call.clone(),
        effect: operation.effect,
        ownership: operation.ownership.clone(),
    });
    builder.emit(
        SelectedInstructionKind::CallUnit {
            callee: call.callee,
        },
        key,
        &operands,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .arguments
                .iter()
                .filter_map(|argument| argument.scalar_source())
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
