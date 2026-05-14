use crate::program::Lowerer;
use crate::statement::lower_statement_handle;
use crate::type_reference::lower_type_reference_handle;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::signature::{StateParameter, StateSignature};
use omega_resolved_trees::state::State;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_state_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    state: &syntax::item::StateNode,
) -> Result<State, Diagnostic> {
    lower_state_parts(
        lowerer,
        syntax_trees,
        &state.name,
        state.parameters,
        state.return_type,
        state.statements,
    )
}

fn lower_state_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &syntax::identifier::Identifier,
    parameters: omega_core::arena::HandleSpan<syntax::item::StateParameterHandle>,
    return_type_handle: syntax::types::TypeReferenceHandle,
    statements: omega_core::arena::HandleSpan<syntax::statement::StatementHandle>,
) -> Result<State, Diagnostic> {
    let parameters = syntax_trees
        .items
        .state_parameters(parameters)
        .iter()
        .map(|parameter| lower_state_parameter(lowerer, syntax_trees, *parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;
    let statements = syntax_trees
        .items
        .statements(statements)
        .iter()
        .map(|statement| lower_statement_handle(lowerer, syntax_trees, *statement))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(State {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(name),
        parameters,
        return_type,
        statements,
        statement_nodes: Default::default(),
    })
}

pub(crate) fn lower_state_signature_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    signature: &syntax::item::StateSignatureNode,
) -> Result<StateSignature, Diagnostic> {
    lower_state_signature_parts(
        lowerer,
        syntax_trees,
        &signature.name,
        signature.parameters,
        signature.return_type,
    )
}

fn lower_state_signature_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &syntax::identifier::Identifier,
    parameters: omega_core::arena::HandleSpan<syntax::item::StateParameterHandle>,
    return_type_handle: syntax::types::TypeReferenceHandle,
) -> Result<StateSignature, Diagnostic> {
    let parameters = syntax_trees
        .items
        .state_parameters(parameters)
        .iter()
        .map(|parameter| lower_state_parameter(lowerer, syntax_trees, *parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;

    Ok(StateSignature {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(name),
        parameters,
        return_type,
    })
}

fn lower_state_parameter(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    parameter: syntax::item::StateParameterHandle,
) -> Result<StateParameter, Diagnostic> {
    let parameter = syntax_trees.items.state_parameter(parameter);
    Ok(StateParameter {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&parameter.name),
        type_reference: lower_type_reference_handle(
            lowerer,
            syntax_trees,
            parameter.type_reference,
        )?,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    })
}
