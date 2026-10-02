//! Independent replay of scalar-result calls and their argument transport.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::row;
use legalized_operations::LegalizedScalarInstruction;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::ValidatedRegisterConstraintCatalog;

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
    let result_shape =
        crate::selection::scalar_call_abi::scalar_shape(scalar_type).ok_or_else(invalid)?;
    if call
        .result_placement
        .as_ref()
        .map(|placement| placement.shape)
        != Some(result_shape)
    {
        return Err(invalid());
    }
    let key = crate::selection::scalar_call_abi::unit_key(call, environment).ok_or_else(invalid)?;
    if !replay.constraints.keys.call_scalar.contains(&key) {
        return Err(invalid());
    }
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(catalog, key)?,
        environment,
    )?;
    let mut operands =
        super::call_arguments::replay(source, operation, call, replay, environment, invalid)?;
    let class = row(catalog, key)?
        .operands
        .last()
        .ok_or_else(invalid)?
        .class;
    let short_result = replay.check_register_class(
        class,
        result.definition_site,
        scalar_type,
        VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            source_value: result.value,
        },
        None,
    )?;
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
            owner: target_operations::CallSiteOwner::Operation(operation.operation),
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    replay.check_instruction(
        SelectedInstructionKind::CallScalar {
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
    let output = replay.result_register(result.value, result.definition_site, scalar_type)?;
    let (kind, key) = crate::selection::scalar_call_abi::incoming_float_transfer(
        scalar_type,
        &replay.constraints.keys,
    )
    .unwrap_or((
        crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type),
        replay.constraints.keys.copy_i64,
    ));
    replay.check_instruction(
        kind,
        key,
        &[short_result, output],
        &SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
