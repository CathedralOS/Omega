//! Scalar-result calls copy typed arguments and retain their actual result home.
use super::*;
use legalized_operations::LegalizedScalarInstruction;

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(invalid());
    };
    let result = operation.result.ok_or_else(invalid)?;
    let scalar_type = result.scalar_type;
    let key = builder
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
        row(builder.catalog, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for argument in &call.arguments {
        if let legalized_operations::LegalizedScalarArgument::Structural { semantic, .. } = argument
        {
            operands.push(structural::call_pointer(
                builder,
                operation,
                semantic.place,
            )?);
            continue;
        }
        let (_, input, site, argument_type) = builder
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
        operands.push(builder.copy(
            input,
            argument.scalar_source().ok_or_else(invalid)?,
            site,
            argument_type,
        )?);
    }
    let short_result = builder.register(result.value, result.definition_site, scalar_type)?;
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
            operation: operation.operation,
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    builder.emit(
        SelectedInstructionKind::CallI64 {
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
    builder.copy(
        short_result,
        result.value,
        result.definition_site,
        scalar_type,
    )
}
