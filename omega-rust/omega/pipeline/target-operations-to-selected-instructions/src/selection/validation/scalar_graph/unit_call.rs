//! Independently replay resultless argument transport and the exact Unit call row.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::SelectedCallContract;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(replay.invalid());
    };
    if operation.result.is_some() || call.result_placement.is_some() {
        return Err(replay.invalid());
    }
    call.validate_source(&operation.ownership)
        .map_err(|_| replay.invalid())?;
    let key = replay
        .constraints
        .keys
        .call_unit
        .get(crate::selection::scalar_call_abi::register_argument_count(
            call,
        ))
        .copied()
        .ok_or_else(|| replay.invalid())?;
    let constraint = environment
        .constraint(key)
        .ok_or_else(|| replay.invalid())?;
    crate::selection::scalar_call_abi::validate(
        replay.function,
        source,
        call,
        operation.operation,
        key,
        constraint,
        environment,
    )?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        match argument {
            LegalizedScalarArgument::Structural { semantic, target } => {
                if let Some(pointer) = super::scalar_call::argument_pointer(
                    replay,
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
                let (_, input, site, scalar_type) =
                    replay.resolve(*value).ok_or_else(|| replay.invalid())?;
                if crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                    != Some(placement.shape)
                {
                    return Err(replay.invalid());
                }
                operands.push(replay.check_copy(input, *value, site, scalar_type)?);
            }
        }
    }
    replay.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        operation: operation.operation,
        call: call.clone(),
        effect: operation.effect,
        ownership: operation.ownership.clone(),
    });
    replay.check_instruction(
        SelectedInstructionKind::CallUnit {
            callee: call.callee,
        },
        key,
        &operands,
        &SelectedInstructionProvenance {
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
