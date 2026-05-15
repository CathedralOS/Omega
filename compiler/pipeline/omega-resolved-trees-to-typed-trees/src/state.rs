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
    let mut typed_state = typed::state::State {
        symbol: state.symbol,
        name: crate::name::lower_name(&state.name),
        parameters: Default::default(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference(lowerer, type_reference))
            .transpose()?,
        statements: Vec::new(),
        statement_nodes: Default::default(),
    };

    for parameter in lowerer.source_program.state_parameters(state.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_state_parameter(&mut typed_state, parameter);
    }

    for statement in lowerer.source_program.state_statements(state.statements) {
        typed_state
            .statements
            .push(lower_statement(lowerer, statement)?);
    }

    Ok(typed_state)
}

pub(crate) fn lower_state_signature(
    lowerer: &mut Lowerer,
    signature: &resolved::signature::StateSignature,
) -> Result<typed::signature::StateSignature, Diagnostic> {
    let mut typed_signature = typed::signature::StateSignature {
        symbol: signature.symbol,
        name: crate::name::lower_name(&signature.name),
        parameters: Vec::new(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference(lowerer, type_reference))
            .transpose()?,
    };

    for parameter in lowerer.source_program.state_parameters(signature.parameters) {
        typed_signature
            .parameters
            .push(lower_state_parameter(lowerer, parameter)?);
    }

    Ok(typed_signature)
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
