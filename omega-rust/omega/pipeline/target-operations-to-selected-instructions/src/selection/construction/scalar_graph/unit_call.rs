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
    let key = crate::selection::scalar_call_abi::unit_key(call, environment).ok_or_else(invalid)?;
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
                    operands.push((argument_index, pointer));
                }
            }
            LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } => {
                if super::scalar_stack::argument(
                    builder,
                    operation,
                    argument_index,
                    *value,
                    placement,
                )? {
                    continue;
                }
                let (_, input, site, scalar_type) = builder.resolve(*value).ok_or_else(invalid)?;
                if crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                    != Some(placement.shape)
                {
                    return Err(invalid());
                }
                let output = if let Some((kind, key)) =
                    crate::selection::scalar_call_abi::outgoing_float_transfer(
                        scalar_type,
                        &builder.constraints.keys,
                    ) {
                    let output = builder.register(*value, site, scalar_type)?;
                    builder.registers[output.0 as usize].class = row(builder.catalog, key)?
                        .operands
                        .get(1)
                        .ok_or_else(invalid)?
                        .class;
                    builder.emit(
                        kind,
                        key,
                        &[input, output],
                        SelectedInstructionProvenance {
                            values: vec![*value],
                            ..Default::default()
                        },
                    )?;
                    output
                } else {
                    builder.copy(input, *value, site, scalar_type)?
                };
                operands.push((argument_index, output));
            }
        }
    }
    let order = crate::selection::scalar_call_abi::register_argument_order(call);
    let operands = order
        .iter()
        .map(|index| {
            operands
                .iter()
                .find(|(argument, _)| argument == index)
                .map(|(_, register)| *register)
                .ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
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
