use crate::program::Lowerer;
use crate::statement::lower_statement_handle;
use crate::type_reference::lower_type_reference;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees::{self as syntax, SyntaxTrees};
use omega_resolved_trees::signature::{StateParameter, StateSignature};
use omega_resolved_trees::state::State;

pub(crate) fn lower_state(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    state: &syntax::item::State,
) -> Result<State, Diagnostic> {
    let parameters = state
        .parameters
        .iter()
        .map(|parameter| lower_state_parameter(lowerer, parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = state
        .return_type
        .as_ref()
        .map(|type_reference| lower_type_reference(lowerer, type_reference))
        .transpose()?;
    let statements = syntax_trees
        .items
        .statements(state.statements)
        .iter()
        .map(|statement| lower_statement_handle(lowerer, syntax_trees, *statement))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(State {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&state.name),
        parameters,
        return_type,
        statements,
        statement_nodes: Default::default(),
    })
}

pub(crate) fn lower_state_signature(
    lowerer: &mut Lowerer,
    signature: &syntax::item::StateSignature,
) -> Result<StateSignature, Diagnostic> {
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| lower_state_parameter(lowerer, parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = signature
        .return_type
        .as_ref()
        .map(|type_reference| lower_type_reference(lowerer, type_reference))
        .transpose()?;

    Ok(StateSignature {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&signature.name),
        parameters,
        return_type,
    })
}

fn lower_state_parameter(
    lowerer: &mut Lowerer,
    parameter: &syntax::item::StateParameter,
) -> Result<StateParameter, Diagnostic> {
    Ok(StateParameter {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&parameter.name),
        type_reference: lower_type_reference(lowerer, &parameter.type_reference)?,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    })
}
