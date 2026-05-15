use crate::program::Lowerer;
use crate::statement::lower_statement;
use crate::type_reference::lower_type_reference;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_state(
    lowerer: &mut Lowerer,
    state: &resolved::state::State,
) -> Result<typed::state::State, Diagnostic> {
    let parameters = lowerer
        .source_program
        .state_parameters(state.parameters)
        .iter()
        .map(|parameter| lower_state_parameter(lowerer, parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = state
        .return_type
        .as_ref()
        .map(|type_reference| lower_type_reference(lowerer, type_reference))
        .transpose()?;
    let statements = state
        .statements
        .iter()
        .map(|statement| lower_statement(lowerer, statement))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(typed::state::State {
        symbol: state.symbol,
        name: crate::name::lower_name(&state.name),
        parameters,
        return_type,
        statements,
        statement_nodes: Default::default(),
    })
}

pub(crate) fn lower_state_signature(
    lowerer: &mut Lowerer,
    signature: &resolved::signature::StateSignature,
) -> Result<typed::signature::StateSignature, Diagnostic> {
    let parameters = lowerer
        .source_program
        .state_parameters(signature.parameters)
        .iter()
        .map(|parameter| lower_state_parameter(lowerer, parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = signature
        .return_type
        .as_ref()
        .map(|type_reference| lower_type_reference(lowerer, type_reference))
        .transpose()?;

    Ok(typed::signature::StateSignature {
        symbol: signature.symbol,
        name: crate::name::lower_name(&signature.name),
        parameters,
        return_type,
    })
}

fn lower_state_parameter(
    lowerer: &mut Lowerer,
    parameter: &resolved::signature::StateParameter,
) -> Result<typed::signature::StateParameter, Diagnostic> {
    Ok(typed::signature::StateParameter {
        symbol: parameter.symbol,
        name: crate::name::lower_name(&parameter.name),
        type_reference: lower_type_reference(lowerer, &parameter.type_reference)?,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    })
}
