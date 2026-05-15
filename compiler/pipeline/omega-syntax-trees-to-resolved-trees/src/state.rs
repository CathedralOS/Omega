use crate::program::Lowerer;
use crate::statement::lower_statement_handle;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::signature::{StateParameter, StateSignature, StateSignatureStorage};
use omega_resolved_trees::state::{State, StateStorage};
use omega_resolved_trees::statement::Statement;
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
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
    return_type_handle: syntax::types::TypeReferenceHandle,
    statements: omega_core::arena::HandleSpan<syntax::statement::StatementHandle>,
) -> Result<State, Diagnostic> {
    let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;
    let statements = lower_state_statements(lowerer, syntax_trees, statements)?;

    Ok(State {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(name),
        storage: StateStorage {
            parameters,
            return_type,
            statements,
            statement_nodes: Default::default(),
        },
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
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
    return_type_handle: syntax::types::TypeReferenceHandle,
) -> Result<StateSignature, Diagnostic> {
    let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;

    Ok(StateSignature {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(name),
        storage: StateSignatureStorage {
            parameters,
            return_type,
        },
    })
}

fn lower_state_statements(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statements: HandleSpan<syntax::statement::StatementHandle>,
) -> Result<HandleSpan<Statement>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for statement in syntax_trees.items.statements(statements) {
        let statement = lower_statement_handle(lowerer, syntax_trees, *statement)?;
        let statement = lowerer
            .program
            .tables
            .declarations
            .state_statements
            .append(statement);
        if count == 0 {
            start = statement;
        }
        count = count
            .checked_add(1)
            .expect("state statement span count overflow");
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
}

fn lower_state_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
) -> Result<HandleSpan<StateParameter>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for parameter in syntax_trees.items.state_parameters(parameters) {
        let parameter = lower_state_parameter(lowerer, syntax_trees, *parameter)?;
        let parameter = lowerer
            .program
            .tables
            .declarations
            .state_parameters
            .append(parameter);
        if count == 0 {
            start = parameter;
        }
        count = count
            .checked_add(1)
            .expect("state parameter span count overflow");
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
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
