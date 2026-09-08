//! Genuine resultless calls share scalar argument copies and original reference transport.
use super::*;

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
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
        .get(call.arguments.len())
        .copied()
        .ok_or_else(invalid)?;
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        key,
        row_constraint(builder, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for argument in &call.arguments {
        match argument {
            LegalizedScalarArgument::Structural { semantic, .. } => {
                operands.push(call_pointer(builder, operation, semantic.place)?)
            }
            LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } => {
                let (_, input, site, scalar_type) = builder.resolve(*value).ok_or_else(invalid)?;
                if !matches!(
                    (scalar_type, placement.shape.byte_size),
                    (ScalarType::Boolean, 1) | (ScalarType::Integer(_), 8)
                ) {
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
