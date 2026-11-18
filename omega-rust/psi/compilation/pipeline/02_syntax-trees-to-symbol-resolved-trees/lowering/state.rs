/*
    Machine states: signatures, parameters, contracts, and bodies.

    Signature lowering covers invokes, service-reach names, and contracts,
    including outcome-specific contracts over a declared result sum whose
    symbols finalize after assignment. Bodies lower statement by statement
    into pending form. The guarded-arm rewrites in `statement` mint their
    continuation states here.
*/

mod lexical_inputs {
    /*
        One-shot lexical preparation for type operands and selected call results.

        These are declaration construction records, not executable statements. They
        are never installed into a state's body, and the machine stays Deferred.
        Ordinary body lowering shares the local declaration constructor; symbol
        assignment shares its preceding-local scope and append-only allocation.
    */

    use crate::declarations::generic_data::statement_call_inputs;
    use crate::declarations::generic_data::statement_type_inputs;
    use crate::lowering::expression::lower_private_expression_into_table;
    use crate::lowering::lowerer::Lowerer;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use arena::{Handle, HandleSpan};
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::{
        state::State,
        statement::{LocalData, LocalDataStorage, Statement},
        types::TypeReference,
    };
    use symbols::SymbolHandle;
    use syntax_trees::{self as syntax, SyntaxTrees};

    #[derive(Clone, Copy)]
    pub(crate) enum LexicalInputDemand {
        TypeOperands,
        CallResults,
    }

    pub(crate) struct StateLexicalInputs {
        pub(crate) machine: SymbolHandle,
        pub(crate) state: Handle<State>,
        pub(crate) locals: Vec<Statement>,
        /* Each input sees only the authored locals preceding its statement. */
        pub(crate) types: Vec<(usize, TypeReference)>,
        pub(crate) calls: Vec<(usize, symbol_resolved_trees::expression::ExpressionHandle)>,
    }

    pub(crate) fn lower_local_declaration(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        local: &syntax::statement::TableLocalData,
    ) -> Result<LocalData, Diagnostic> {
        let type_reference = if local.type_reference.is_valid() {
            lower_type_reference_handle(lowerer, syntax, local.type_reference)?
        } else {
            TypeReference::Unit
        };
        /*
            A record pattern's roster keeps its authored spellings here.
            Each entry receives its field once the subject's type is known,
            in typed preparation, so no entry is resolved by this stage.
        */
        let roster = syntax
            .expressions
            .match_fields(local.pattern_fields)
            .iter()
            .map(|field| {
                use symbol_resolved_trees::expression::{MatchFieldAction, TableMatchField};
                TableMatchField {
                    component: field.component,
                    member: crate::lowering::name::lower_name(syntax, &field.member),
                    field_symbol: SymbolHandle::invalid(),
                    action: match &field.action {
                        syntax::expression::MatchFieldAction::Bind(binding) => {
                            MatchFieldAction::Bind(crate::lowering::name::lower_name(
                                syntax, binding,
                            ))
                        }
                        syntax::expression::MatchFieldAction::Waive
                        | syntax::expression::MatchFieldAction::Equal(_) => MatchFieldAction::Waive,
                    },
                }
            })
            .collect::<Vec<_>>();
        let pattern_fields = lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .insert_match_fields(roster);
        Ok(LocalData {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax, &local.name),
            storage: LocalDataStorage {
                role: local.role,
                type_reference,
                initial_value: symbol_resolved_trees::expression::ExpressionHandle::invalid(),
                is_mutable: local.is_mutable,
                type_is_inferred: !local.type_reference.is_valid(),
                is_operand_capture: false,
                relevance: local.relevance,
                pattern_fields,
            },
        })
    }

    pub(crate) fn lower_state_lexical_inputs(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        machine: SymbolHandle,
        state: Handle<State>,
        statements: HandleSpan<syntax::statement::StatementHandle>,
        demand: LexicalInputDemand,
    ) -> Result<StateLexicalInputs, Diagnostic> {
        let declaration = lowerer.symbol_resolved_trees.machine_state(state).clone();
        super::with_state_scope(lowerer, &declaration, |lowerer| {
            let mut inputs = StateLexicalInputs {
                machine,
                state,
                locals: Vec::new(),
                types: Vec::new(),
                calls: Vec::new(),
            };
            for &statement in syntax.items.statements(statements) {
                let local = match syntax.statements.statement(statement) {
                    syntax::statement::StatementNode::LocalData(local) => Some(local),
                    _ => None,
                };
                let types = statement_type_inputs(syntax, statement)
                    .into_iter()
                    .filter(|reference| {
                        !local.is_some_and(|local| local.type_reference == *reference)
                    })
                    .collect::<Vec<_>>();
                let calls = if matches!(demand, LexicalInputDemand::CallResults) {
                    statement_call_inputs(syntax, statement)
                } else {
                    Vec::new()
                };
                if !types.is_empty() || !calls.is_empty() {
                    crate::lowering::expression::match_dispatch::with_transition_pattern_captures(
                        lowerer,
                        syntax,
                        statement,
                        |lowerer| {
                            for reference in types {
                                inputs.types.push((
                                    inputs.locals.len(),
                                    lower_type_reference_handle(lowerer, syntax, reference)?,
                                ));
                            }
                            for expression in calls {
                                inputs.calls.push((
                                    inputs.locals.len(),
                                    lower_private_expression_into_table(
                                        lowerer, syntax, expression,
                                    )?,
                                ));
                            }
                            Ok(())
                        },
                    )?;
                }
                if let Some(local) = local {
                    inputs
                        .locals
                        .push(Statement::LocalData(lower_local_declaration(
                            lowerer, syntax, local,
                        )?));
                }
            }
            Ok(inputs)
        })
    }
}
pub(crate) use lexical_inputs::{
    LexicalInputDemand, StateLexicalInputs, lower_local_declaration, lower_state_lexical_inputs,
};

use crate::lowering::domain::lower_proof_facts;
use crate::lowering::lowerer::Lowerer;
use crate::lowering::name::lower_names;
use crate::lowering::statement::lower_statement_handle;
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::tables::syntax_items;
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use std::mem::replace;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::signature::{
    CrashCause, NativeCallbackParameter, SignatureContract, SignatureContractKind, StateParameter,
    StateSignature, StateSignatureStorage,
};
use symbol_resolved_trees::state::State;
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;
use syntax::identifier::Identifier;
use syntax::item::CapabilityContract;
use syntax::item::CapabilityContractKind;
use syntax::item::NativeCallbackParameterNode;
use syntax::item::StateParameterHandle;
use syntax::item::TypeParameter;
use syntax::operator_spelling::OperatorSpelling;
use syntax::statement::StatementHandle;
use syntax::statement::StatementNode;
use syntax::types::TypeReferenceHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_state_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    state: &syntax::item::State,
    public_signature: bool,
) -> Result<State, Diagnostic> {
    lower_state_parts(
        lowerer,
        syntax_trees,
        &state.name,
        state.parameters,
        state.return_type,
        state.contracts,
        state.statements,
        public_signature,
    )
}

fn lower_state_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &Identifier,
    parameters: HandleSpan<StateParameterHandle>,
    return_type_handle: TypeReferenceHandle,
    contracts: HandleSpan<CapabilityContract>,
    statements: arena::HandleSpan<StatementHandle>,
    public_signature: bool,
) -> Result<State, Diagnostic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    let exposure = if public_signature {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    };
    let (parameters, return_type) =
        lowerer.with_authored_expression_exposure(exposure, |lowerer| {
            let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
            let return_type = return_type_handle
                .is_valid()
                .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
                .transpose()?;
            Ok::<_, Diagnostic>((parameters, return_type))
        })?;
    let mut state = State::header(
        crate::lowering::name::lower_name(syntax_trees, name),
        parameters,
        return_type,
    );
    state.contracts = with_state_scope(lowerer, &state, |lowerer| {
        lower_signature_contracts(lowerer, syntax_trees, contracts)
    })?;
    if !lowerer.defer_machine_bodies {
        let body = lower_state_body(lowerer, syntax_trees, &state, statements)?;
        state.set_construction_statements(body);
    }
    Ok(state)
}

/*
    Complete statements under their already indexed signature. The caller owns
    the pending syntax and only publishes the machine as Complete afterwards.
*/
pub(crate) fn lower_state_body(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    declaration: &State,
    statements: HandleSpan<StatementHandle>,
) -> Result<HandleSpan<Statement>, Diagnostic> {
    with_state_scope(lowerer, declaration, |lowerer| {
        lower_state_statements(lowerer, syntax, statements)
    })
}

/*
    Signatures and statements read the same declared parameter scope.
    Restore the previous scope on both success and failure; completing one state
    must not change how a later state's body or generated continuation is lowered.
*/
fn with_state_scope<T>(
    lowerer: &mut Lowerer,
    state: &State,
    operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    let previous_name = lowerer
        .current_state_name
        .replace(state.name.as_str().to_owned());
    let reference_parameters = if lowerer
        .machine_body
        .as_ref()
        .is_some_and(|scope| scope.is_boundary)
    {
        reference_struct_parameter_names(lowerer, &state.parameters)
    } else {
        Vec::new()
    };
    let parameter_names = lowerer
        .symbol_resolved_trees
        .state_parameters(state.parameters)
        .iter()
        .map(|parameter| parameter.name.as_str().to_string())
        .collect();
    let parameters = lowerer
        .symbol_resolved_trees
        .state_parameters(state.parameters)
        .iter()
        .map(|parameter| {
            (
                parameter.name.as_str().to_string(),
                parameter.type_reference.clone(),
                parameter.is_mutable,
            )
        })
        .collect();
    let previous_references = replace(
        &mut lowerer.reference_struct_parameters,
        reference_parameters,
    );
    let previous_names = replace(&mut lowerer.current_state_parameter_names, parameter_names);
    let previous_parameters = replace(&mut lowerer.current_state_parameters, parameters);
    let previous_return = replace(
        &mut lowerer.current_state_return_type,
        state.return_type.clone(),
    );
    let result = operation(lowerer);
    lowerer.current_state_name = previous_name;
    lowerer.reference_struct_parameters = previous_references;
    lowerer.current_state_parameter_names = previous_names;
    lowerer.current_state_parameters = previous_parameters;
    lowerer.current_state_return_type = previous_return;
    result
}

pub(crate) fn lower_state_signature_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    signature: &syntax::item::StateSignature,
) -> Result<LoweredStateSignature, Diagnostic> {
    lower_state_signature_parts(
        lowerer,
        syntax_trees,
        StateSignatureParts {
            name: &signature.name,
            spelling: signature.spelling,
            lifetime_parameters: &signature.lifetime_parameters,
            type_parameters: signature.type_parameters,
            conformance_bounds: &signature.conformance_bounds,
            parameters: signature.parameters,
            native_callback_parameters: &signature.native_callback_parameters,
            return_type_handle: signature.return_type,
            is_default: signature.is_default,
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: &signature.service_reach_keyword_source_spans,
            service_reaches: signature.service_reaches,
            invokes: signature.invokes,
            suspends_keyword_source_spans: &signature.suspends_keyword_source_spans,
            blocks_keyword_source_spans: &signature.blocks_keyword_source_spans,
            suspends: signature.suspends,
            blocks: signature.blocks,
            contracts: signature.contracts,
            terminates_guarantee: signature.terminates_guarantee,
            declares_no_crash_route: signature.declares_no_crash_route,
        },
    )
}

/*
    The authored parts of one state or structural machine-parameter signature,
    borrowed from its syntax node.
*/
#[derive(Debug)]
pub(crate) struct StateSignatureParts<'a> {
    pub(crate) name: &'a Identifier,
    pub(crate) spelling: Option<OperatorSpelling>,
    pub(crate) lifetime_parameters: &'a [Identifier],
    pub(crate) type_parameters: HandleSpan<TypeParameter>,
    pub(crate) conformance_bounds: &'a [syntax::item::GenericConformanceBound],
    pub(crate) parameters: HandleSpan<StateParameterHandle>,
    pub(crate) native_callback_parameters: &'a [NativeCallbackParameterNode],
    pub(crate) return_type_handle: TypeReferenceHandle,
    pub(crate) is_default: bool,
    pub(crate) service_reach_is_installation_bound: bool,
    pub(crate) service_reach_keyword_source_spans: &'a [source::SourceSpan],
    pub(crate) service_reaches: HandleSpan<Identifier>,
    pub(crate) invokes: HandleSpan<Identifier>,
    pub(crate) suspends_keyword_source_spans: &'a [source::SourceSpan],
    pub(crate) blocks_keyword_source_spans: &'a [source::SourceSpan],
    pub(crate) suspends: bool,
    pub(crate) blocks: bool,
    pub(crate) contracts: HandleSpan<CapabilityContract>,
    pub(crate) terminates_guarantee: bool,
    pub(crate) declares_no_crash_route: bool,
}

pub(crate) fn lower_state_signature_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    inputs: StateSignatureParts<'_>,
) -> Result<LoweredStateSignature, Diagnostic> {
    let scope = lowerer.runtime_callable_scopes.begin(None);
    let result = lower_state_signature_contents(lowerer, syntax_trees, inputs, scope);
    lowerer.runtime_callable_scopes.end(scope);
    result
}

fn lower_state_signature_contents(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    inputs: StateSignatureParts<'_>,
    callable_scope: crate::names::runtime_callable_scopes::CallableScopeHandle,
) -> Result<LoweredStateSignature, Diagnostic> {
    let StateSignatureParts {
        name,
        spelling,
        lifetime_parameters,
        type_parameters,
        conformance_bounds,
        parameters,
        native_callback_parameters,
        return_type_handle,
        is_default,
        service_reach_is_installation_bound,
        service_reach_keyword_source_spans,
        service_reaches,
        invokes,
        suspends_keyword_source_spans,
        blocks_keyword_source_spans,
        suspends,
        blocks,
        contracts,
        terminates_guarantee,
        declares_no_crash_route,
    } = inputs;
    let type_parameters =
        crate::lowering::data::lower_type_parameters(lowerer, syntax_trees, type_parameters)?;
    let conformance_bounds = crate::lowering::machine::lower_generic_conformance_bounds(
        lowerer,
        syntax_trees,
        conformance_bounds,
    )?;
    let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;
    let service_reaches = lower_service_reach_names(syntax_trees, service_reaches);
    let invokes = lower_signature_invokes(lowerer, syntax_trees, invokes);
    let contracts = lower_signature_contracts(lowerer, syntax_trees, contracts)?;

    Ok(LoweredStateSignature {
        callable_scope,
        signature: StateSignature {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, name),
            storage: StateSignatureStorage {
                spelling,
                lifetime_parameters: lower_names(syntax_trees, lifetime_parameters),
                type_parameters,
                conformance_bounds,
                is_default,
                parameters,
                native_callback_parameters: native_callback_parameters
                    .iter()
                    .map(|parameter| NativeCallbackParameter {
                        name: crate::lowering::name::lower_name(syntax_trees, &parameter.name),
                        binder: crate::lowering::name::lower_name(syntax_trees, &parameter.binder),
                        native_ordinal: parameter.native_ordinal,
                    })
                    .collect(),
                return_type,
                invokes,
                service_reach_row: language_semantics::ServiceReachRowId::NULL,
                service_reach_is_installation_bound,
                suspends_keyword_source_spans: suspends_keyword_source_spans.to_vec(),
                blocks_keyword_source_spans: blocks_keyword_source_spans.to_vec(),
                suspends,
                blocks,
                contracts,
                terminates_guarantee,
                declares_no_crash_route,
            },
        },
        service_reach_keyword_source_spans: service_reach_keyword_source_spans.to_vec(),
        service_reaches,
    })
}

pub(crate) struct LoweredStateSignature {
    pub(crate) callable_scope: crate::names::runtime_callable_scopes::CallableScopeHandle,
    pub(crate) signature: StateSignature,
    pub(crate) service_reach_keyword_source_spans: Vec<source::SourceSpan>,
    pub(crate) service_reaches: Vec<DiagnosticName>,
}

pub(crate) fn lower_signature_invokes(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    invokes: HandleSpan<Identifier>,
) -> HandleSpan<DiagnosticName> {
    crate::lowering::name::lower_name_members(
        syntax_trees,
        invokes,
        &mut lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .signature_invokes,
    )
}

pub(crate) fn lower_service_reach_names(
    syntax_trees: &SyntaxTrees,
    service_reaches: HandleSpan<Identifier>,
) -> Vec<DiagnosticName> {
    lower_names(
        syntax_trees,
        syntax_trees.items.identifier_path_members(service_reaches),
    )
}

pub(crate) fn lower_signature_contracts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    contracts: HandleSpan<CapabilityContract>,
) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
    lower_signature_contracts_with_result_sum(lowerer, syntax_trees, contracts, None)
}

pub(crate) fn lower_machine_signature_contracts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    contracts: HandleSpan<CapabilityContract>,
    states: HandleSpan<Handle<symbol_resolved_trees::state::State>>,
) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
    let has_outcome_rows = syntax_items(syntax_trees)
        .capability_contracts(contracts)
        .iter()
        .any(|contract| {
            matches!(
                &contract.kind,
                CapabilityContractKind::EnsuresForResultCase { .. }
            )
        });
    if !has_outcome_rows {
        return lower_signature_contracts_with_result_sum(lowerer, syntax_trees, contracts, None);
    }

    let root_state = lowerer
        .symbol_resolved_trees
        .machine_state_handles(states)
        .first()
        .copied()
        .map(|handle| lowerer.symbol_resolved_trees.machine_state(handle))
        .ok_or_else(|| {
            Diagnostic::error("outcome-specific ensures requires a machine result state")
        })?;
    let result_data_name = match root_state.return_type.as_ref() {
        Some(TypeReference::Named { name, .. }) => name,
        Some(TypeReference::Generic(generic)) => &generic.base_name,
        _ => {
            return Err(Diagnostic::error(
                "outcome-specific ensures requires a declared nominal sum result type",
            ));
        }
    };
    let data = lowerer
        .symbol_resolved_trees
        .data_definitions
        .iter()
        .filter(|data| {
            data.name.as_str() == result_data_name.as_str()
                && lowerer.source_reference_can_see_declaration(
                    result_data_name.source_span(),
                    data.name.source_span(),
                )
        })
        .min_by_key(|data| {
            let declaration = data.name.source_span();
            let reference = result_data_name.source_span();
            (
                declaration.source_id != reference.source_id,
                lowerer.source_resolution_strata_separate(reference, declaration),
            )
        })
        .ok_or_else(|| {
            Diagnostic::error(
                "outcome-specific ensures result does not resolve to a declared data sum",
            )
        })?;
    let variants = lowerer
        .symbol_resolved_trees
        .data_members(data.members)
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant.name.to_string()),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    if variants.is_empty() {
        return Err(Diagnostic::error(format!(
            "outcome-specific ensures requires a sum result; `{}` declares no cases",
            data.name
        )));
    }
    let result_sum = ResultSumContractContext {
        data_name: data.name.to_string(),
        variants,
    };
    lower_signature_contracts_with_result_sum(lowerer, syntax_trees, contracts, Some(&result_sum))
}

struct ResultSumContractContext {
    data_name: String,
    variants: Vec<String>,
}

fn lower_signature_contracts_with_result_sum(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    contracts: HandleSpan<CapabilityContract>,
    result_sum: Option<&ResultSumContractContext>,
) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for contract in syntax_trees.items.capability_contracts(contracts) {
        let (kind, pending_outcome) = match &contract.kind {
            CapabilityContractKind::Requires => (SignatureContractKind::Requires, None),
            CapabilityContractKind::Ensures => (SignatureContractKind::Ensures, None),
            CapabilityContractKind::EnsuresForResultCase { result_case } => {
                let Some(result_sum) = result_sum else {
                    return Err(Diagnostic::error(
                        "outcome-specific ensures is admitted only on a top-level machine result",
                    ));
                };
                let members = syntax_trees.items.identifier_path_members(*result_case);
                let [result_name, case_name] = members else {
                    return Err(Diagnostic::error(
                        "outcome-specific ensures requires the exact path `Result::Case`",
                    ));
                };
                if result_name.as_str(syntax_trees) != result_sum.data_name.as_str() {
                    return Err(Diagnostic::error(format!(
                        "outcome-specific ensures case `{result_name}::{case_name}` does not belong to declared result sum `{}`",
                        result_sum.data_name,
                        result_name = result_name.as_str(syntax_trees),
                        case_name = case_name.as_str(syntax_trees)
                    )));
                }
                let result_case_name = result_sum
                .variants
                .iter()
                .find(|name| name.as_str() == case_name.as_str(syntax_trees))
                .cloned()
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "outcome-specific ensures names unknown case `{case_name}` of declared result sum `{}`",
                        result_sum.data_name
                    , case_name = case_name.as_str(syntax_trees)))
                })?;
                (
                    SignatureContractKind::EnsuresForResultCase {
                        result_data: symbols::SymbolHandle::invalid(),
                        result_case: symbols::SymbolHandle::invalid(),
                    },
                    Some((
                        result_sum.data_name.clone(),
                        result_name.source_span(),
                        result_case_name,
                    )),
                )
            }
            CapabilityContractKind::Crashes { cause } => (
                SignatureContractKind::Crashes {
                    cause: match cause {
                        syntax::item::CrashCause::Trap => CrashCause::Trap,
                        syntax::item::CrashCause::Abort => CrashCause::Abort,
                    },
                },
                None,
            ),
        };
        let facts = lower_proof_facts(lowerer, syntax_trees, contract.facts)?;
        let handle = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .signature_contracts
            .append_to_span(
                &mut span,
                SignatureContract {
                    kind,
                    keyword_source_span: contract.keyword_source_span,
                    binding: contract
                        .binding
                        .as_ref()
                        .map(|name| crate::lowering::name::lower_name(syntax_trees, name)),
                    facts,
                    token_count: contract.token_count,
                },
            );
        if let Some((result_data_name, result_data_source_span, result_case_name)) = pending_outcome
        {
            lowerer.pending_outcome_specific_contracts.push(
                crate::lowering::lowerer::PendingOutcomeSpecificContract {
                    contract: handle,
                    result_data_name,
                    result_data_source_span,
                    result_case_name,
                },
            );
        }
    }

    Ok(span)
}

fn lower_state_statements(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statements: HandleSpan<StatementHandle>,
) -> Result<HandleSpan<Statement>, Diagnostic> {
    let mut pending: Vec<Statement> = Vec::new();
    let mut run_start: Option<usize> = None;
    let mut statement_origins = Vec::new();

    for statement in syntax_trees.items.statements(statements) {
        lower_statement_into_pending(
            lowerer,
            syntax_trees,
            *statement,
            &mut pending,
            &mut run_start,
            &mut statement_origins,
        )?;
    }

    let mut span = HandleSpan::empty();
    for (index, lowered) in pending.into_iter().enumerate() {
        let handle = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_statements
            .append_to_span(&mut span, lowered);
        if let Some(sources) = &mut lowerer.equation_sources
            && let Some((_, original)) = statement_origins
                .iter()
                .find(|(offset, _)| *offset == index)
        {
            sources.statements.push((*original, handle));
        }
    }
    Ok(span)
}

fn lower_statement_into_pending(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: StatementHandle,
    pending: &mut Vec<Statement>,
    run_start: &mut Option<usize>,
    statement_origins: &mut Vec<(usize, StatementHandle)>,
) -> Result<(), Diagnostic> {
    let has_preceding_transition = run_start.is_some();
    let mut lowered = lower_statement_handle(
        lowerer,
        syntax_trees,
        statement,
        pending.len(),
        has_preceding_transition,
    )?;
    let ends_in_transition = matches!(lowered.last(), Some(Statement::Transition(_)));
    if ends_in_transition {
        let lets_count = lowered.len() - 1;
        match *run_start {
            Some(start) if lets_count > 0 => {
                for (offset, _) in statement_origins.iter_mut() {
                    if *offset >= start {
                        *offset += lets_count;
                    }
                }
                let transition = lowered.pop().expect("transition chunk is non-empty");
                for (offset, hoisted) in lowered.into_iter().enumerate() {
                    pending.insert(start + offset, hoisted);
                }
                pending.push(transition);
            }
            _ => {
                if run_start.is_none() {
                    *run_start = Some(pending.len() + lets_count);
                }
                pending.extend(lowered);
            }
        }
    } else {
        pending.extend(lowered);
        *run_start = None;
    }

    if matches!(
        syntax_trees.statements.statement(statement),
        StatementNode::Transition(transition) if transition.ends_dispatch
    ) {
        *run_start = None;
    }

    if matches!(
        syntax_trees.statements.statement(statement),
        StatementNode::Call(_) | StatementNode::LocalData(_)
    ) {
        statement_origins.push((pending.len() - 1, statement));
    }
    Ok(())
}

pub(crate) fn lower_state_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    parameters: HandleSpan<StateParameterHandle>,
) -> Result<HandleSpan<StateParameter>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for parameter in syntax_trees.items.state_parameters(parameters) {
        let authored_type = syntax_items(syntax_trees)
            .state_parameter(*parameter)
            .type_reference;
        let parameter = lower_state_parameter(lowerer, syntax_trees, *parameter)?;
        let handle = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_parameters
            .append_to_span(&mut span, parameter);
        if let Some(sources) = &mut lowerer.equation_sources {
            sources.declared_types.push((
                crate::declarations::machine_equations::DeclaredTypeOrigin::Parameter(handle),
                authored_type,
            ));
        }
    }

    Ok(span)
}

pub(crate) fn lower_state_parameter(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    parameter: StateParameterHandle,
) -> Result<StateParameter, Diagnostic> {
    let parameter = syntax_trees.items.state_parameter(parameter);
    Ok(StateParameter {
        symbol: SymbolHandle::invalid(),
        name: crate::lowering::name::lower_name(syntax_trees, &parameter.name),
        type_reference: lower_type_reference_handle(
            lowerer,
            syntax_trees,
            parameter.type_reference,
        )?,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        relevance: parameter.relevance,
    })
}

/*
    The names of parameters declared as a SHARED reference to a NAMED type
    (`table: &EfiSystemTable`) -- the shapes whose member reads must deref the
    pointer slot rather than fold flat.
*/
fn reference_struct_parameter_names(
    lowerer: &Lowerer,
    parameters: &HandleSpan<symbol_resolved_trees::signature::StateParameter>,
) -> Vec<String> {
    lowerer
        .symbol_resolved_trees
        .state_parameters(*parameters)
        .iter()
        .filter_map(|parameter| {
            let TypeReference::Reference(reference) = &parameter.type_reference else {
                return None;
            };
            if reference.access.is_exclusive() {
                return None;
            }
            matches!(
                lowerer
                    .symbol_resolved_trees
                    .child_type_reference(reference.referee),
                TypeReference::Named { .. }
            )
            .then(|| parameter.name.as_str().to_string())
        })
        .collect()
}
