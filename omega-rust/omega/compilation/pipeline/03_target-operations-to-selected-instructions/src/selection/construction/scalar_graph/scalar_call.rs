//! Scalar-result calls marshal their arguments and retain their actual result home.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
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
    // A scalar-shaped result placement can only come from the scalar-call key
    // family: `unit_key` searches that family exactly when the call carries no
    // structural result. A call with both would previously have been refused
    // as a key outside the family; refuse the shape it depends on instead.
    if call.structural_result.is_some() {
        return Err(invalid());
    }
    let key = super::call_arguments::validated_call_key(
        function,
        source,
        call,
        target_operations::CallSiteOwner::Operation(operation.operation),
        builder,
        environment,
        invalid,
    )?;
    let mut operands = super::call_arguments::marshal(source, operation, call, builder, invalid)?;
    if let Some(selector) = call.target.selector() {
        let (_, input, site, scalar_type) = builder.resolve(selector).ok_or_else(invalid)?;
        if scalar_type != target_operations::selector_carrier_type() {
            return Err(invalid());
        }
        operands.push(builder.copy(input, selector, site, scalar_type)?);
    }
    let short_result = builder.register(result.value, result.definition_site, scalar_type)?;
    builder.registers[short_result.0 as usize].class = row(builder.catalog, key)?
        .operands
        .last()
        .ok_or_else(invalid)?
        .class;
    operands.push(short_result);
    super::call_arguments::emit_call(
        builder,
        target_operations::CallSiteOwner::Operation(operation.operation),
        call,
        operation.effect,
        operation.ownership.clone(),
        SelectedInstructionKind::CallScalar {
            target: selected_instructions::SelectedCallTarget::from_native(call.target),
        },
        key,
        &operands,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .arguments
                .iter()
                .filter_map(|argument| argument.scalar_source())
                .chain(call.target.selector())
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
