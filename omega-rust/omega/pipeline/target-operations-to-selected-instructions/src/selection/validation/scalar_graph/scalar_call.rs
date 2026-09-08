//! Independent replay of scalar-result calls and their argument transport.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use register_environment::ValidatedTargetRegisterEnvironment;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let function = replay.function;
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(invalid());
    };
    let result = operation.result.ok_or_else(invalid)?;
    let scalar_type = result.scalar_type;
    let key = replay
        .constraints
        .keys
        .call_i64
        .get(call.arguments.len())
        .copied()
        .ok_or_else(invalid)?;
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(catalog, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for argument in &call.arguments {
        if let LegalizedScalarArgument::Structural { semantic, target } = argument {
            operands.push(structural::call_pointer(
                replay,
                operation,
                semantic.place,
                target.source_byte_offset,
            )?);
            continue;
        }
        let (_, input, site, argument_type) = replay
            .resolve(argument.scalar_source().ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let shape = match argument_type {
            ScalarType::Boolean => calling_conventions::ValueShape::integer(1, 1),
            ScalarType::Integer(integer) if integer.bits() == 64 => {
                calling_conventions::ValueShape::integer(8, 8)
            }
            _ => return Err(invalid()),
        };
        if argument.placement().shape != shape {
            return Err(invalid());
        }
        operands.push(replay.check_copy(
            input,
            argument.scalar_source().ok_or_else(invalid)?,
            site,
            argument_type,
        )?);
    }
    let short_result = replay.result_register(result.value, result.definition_site, scalar_type)?;
    operands.push(short_result);
    replay
        .transport
        .calls
        .push(selected_instructions::SelectedCallContract {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            operation: operation.operation,
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    replay.check_instruction(
        SelectedInstructionKind::CallI64 {
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
                .chain(std::iter::once(result.value))
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    replay.check_copy(
        short_result,
        result.value,
        result.definition_site,
        scalar_type,
    )
}
