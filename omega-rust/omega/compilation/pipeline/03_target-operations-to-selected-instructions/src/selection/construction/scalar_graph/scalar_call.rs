//! Scalar-result calls marshal their arguments and retain their actual result home.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::row;
use legalized_operations::LegalizedScalarInstruction;

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
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
    if !builder.constraints.keys.call_scalar.contains(&key) {
        return Err(invalid());
    }
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(builder.catalog, key)?,
        environment,
    )?;
    let mut operands = super::call_arguments::marshal(source, operation, call, builder, invalid)?;
    let short_result = builder.register(result.value, result.definition_site, scalar_type)?;
    builder.registers[short_result.0 as usize].class = row(builder.catalog, key)?
        .operands
        .last()
        .ok_or_else(invalid)?
        .class;
    operands.push(short_result);
    builder
        .transport
        .calls
        .push(selected_instructions::SelectedCallContract {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            owner: target_operations::CallSiteOwner::Operation(operation.operation),
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    builder.emit(
        SelectedInstructionKind::CallScalar {
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
                .chain(std::iter::once(result.value))
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    let output = builder.register(result.value, result.definition_site, scalar_type)?;
    let (kind, key) = crate::selection::scalar_call_abi::incoming_float_transfer(
        scalar_type,
        &builder.constraints.keys,
    )
    .unwrap_or((
        crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type),
        builder.constraints.keys.copy_i64,
    ));
    builder.emit(
        kind,
        key,
        &[short_result, output],
        SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
