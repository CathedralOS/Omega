use crate::domain::lower_proof_facts;
use crate::lowerer::Lowerer;
use crate::statement::lower_statement_handle;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::signature::{
    SignatureContract, SignatureContractKind, StateParameter, StateSignature, StateSignatureStorage,
};
use omega_symbol_resolved_trees::state::{State, StateStorage};
use omega_symbol_resolved_trees::statement::Statement;
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
        signature.effects,
        signature.contracts,
    )
}

fn lower_state_signature_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &syntax::identifier::Identifier,
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
    return_type_handle: syntax::types::TypeReferenceHandle,
    effects: HandleSpan<syntax::identifier::Identifier>,
    contracts: HandleSpan<syntax::item::CapabilityContract>,
) -> Result<StateSignature, Diagnostic> {
    let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;
    let effects = lower_signature_effects(lowerer, syntax_trees, effects);
    let contracts = lower_signature_contracts(lowerer, syntax_trees, contracts)?;

    Ok(StateSignature {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(name),
        storage: StateSignatureStorage {
            parameters,
            return_type,
            effects,
            contracts,
        },
    })
}

pub(crate) fn lower_signature_effects(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    effects: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<omega_symbol_resolved_trees::name::DiagnosticName> {
    let mut span = HandleSpan::empty();

    for effect in syntax_trees.items.identifier_path_members(effects) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .signature_effects
            .append_to_span(&mut span, crate::name::lower_name(effect));
    }

    span
}

pub(crate) fn lower_signature_contracts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    contracts: HandleSpan<syntax::item::CapabilityContract>,
) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for contract in syntax_trees.items.capability_contracts(contracts) {
        let facts = lower_proof_facts(lowerer, syntax_trees, contract.facts)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .signature_contracts
            .append_to_span(
                &mut span,
                SignatureContract {
                    kind: match &contract.kind {
                        syntax::item::CapabilityContractKind::Requires => {
                            SignatureContractKind::Requires
                        }
                        syntax::item::CapabilityContractKind::Ensures => {
                            SignatureContractKind::Ensures
                        }
                        syntax::item::CapabilityContractKind::Boundary(_) => {
                            SignatureContractKind::Boundary
                        }
                    },
                    facts,
                    token_count: contract.token_count,
                },
            );
    }

    Ok(span)
}

fn lower_state_statements(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statements: HandleSpan<syntax::statement::StatementHandle>,
) -> Result<HandleSpan<Statement>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for statement in syntax_trees.items.statements(statements) {
        let statement = lower_statement_handle(lowerer, syntax_trees, *statement)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_statements
            .append_to_span(&mut span, statement);
    }

    Ok(span)
}

pub(crate) fn lower_state_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
) -> Result<HandleSpan<StateParameter>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for parameter in syntax_trees.items.state_parameters(parameters) {
        let parameter = lower_state_parameter(lowerer, syntax_trees, *parameter)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_parameters
            .append_to_span(&mut span, parameter);
    }

    Ok(span)
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
