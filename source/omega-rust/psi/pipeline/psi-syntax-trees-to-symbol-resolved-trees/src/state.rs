use crate::domain::lower_proof_facts;
use crate::lowerer::Lowerer;
use crate::statement::lower_statement_handle;
use crate::type_reference::lower_type_reference_handle;
use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbol_resolved_trees::signature::{
    SignatureContract, SignatureContractKind, StateParameter, StateSignature, StateSignatureStorage,
};
use psi_symbol_resolved_trees::state::{State, StateStorage};
use psi_symbol_resolved_trees::statement::Statement;
use psi_symbol_resolved_trees::types::TypeReference;
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

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
        state.contracts,
        state.statements,
    )
}

fn lower_state_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &syntax::identifier::Identifier,
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
    return_type_handle: syntax::types::TypeReferenceHandle,
    contracts: HandleSpan<syntax::item::CapabilityContract>,
    statements: psi_arena::HandleSpan<syntax::statement::StatementHandle>,
) -> Result<State, Diagnostic> {
    lowerer.current_state_name = Some(name.as_str().to_owned());
    let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;
    // Record which of THIS state's params are shared references to a NAMED
    // type (`table: &EfiSystemTable`): a member read through one must
    // dereference the pointer slot, so the guard/operand hoists materialize it
    // into a `let` (the boot-verified pointee path). `&mut` params are
    // excluded -- their alias slots share the caller's storage and fold flat
    // correctly. Overwrites the previous state's list.
    lowerer.reference_struct_parameters = if lowerer.current_machine_is_boundary {
        reference_struct_parameter_names(lowerer, &parameters)
    } else {
        // Non-boundary `&Struct` params are call-site alias slots sharing the
        // caller's storage -- their member reads fold correctly and must NOT
        // be pointee-materialized.
        Vec::new()
    };
    lowerer.current_state_parameter_names = lowerer
        .symbol_resolved_trees
        .state_parameters(parameters)
        .iter()
        .map(|parameter| parameter.name.as_str().to_string())
        .collect();
    // The guarded-arm value-call rewrite copies parameter records (and the
    // return type) into its synthesized continuation states.
    lowerer.current_state_parameters = lowerer
        .symbol_resolved_trees
        .state_parameters(parameters)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| {
            (
                parameter.name.as_str().to_string(),
                parameter.type_reference.clone(),
                parameter.is_mutable,
            )
        })
        .collect();
    lowerer.current_state_self_parameter = lowerer
        .symbol_resolved_trees
        .state_parameters(parameters)
        .iter()
        .find(|parameter| parameter.is_self)
        .cloned();
    lowerer.current_state_locals.clear();
    lowerer.current_state_return_type = return_type.clone();
    let contracts = lower_signature_contracts(lowerer, syntax_trees, contracts)?;
    let statements = lower_state_statements(lowerer, syntax_trees, statements)?;
    lowerer.reference_struct_parameters = Vec::new();
    lowerer.current_state_parameter_names = Vec::new();
    lowerer.current_state_parameters = Vec::new();
    lowerer.current_state_self_parameter = None;
    lowerer.current_state_locals.clear();
    lowerer.current_state_return_type = None;

    Ok(State {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(name),
        storage: StateStorage {
            parameters,
            return_type,
            contracts,
            statements,
            statement_nodes: Default::default(),
        },
    })
}

pub(crate) fn lower_state_signature_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    signature: &syntax::item::StateSignatureNode,
) -> Result<LoweredStateSignature, Diagnostic> {
    lower_state_signature_parts(
        lowerer,
        syntax_trees,
        &signature.name,
        signature.spelling,
        &signature.lifetime_parameters,
        signature.type_parameters,
        signature.parameters,
        &signature.native_callback_parameters,
        signature.return_type,
        signature.is_default,
        signature.service_reach_is_installation_bound,
        &signature.service_reach_keyword_source_spans,
        signature.service_reaches,
        signature.invokes,
        &signature.suspends_keyword_source_spans,
        &signature.blocks_keyword_source_spans,
        signature.suspends,
        signature.blocks,
        signature.contracts,
        // TPR4: the bodyless requirement's authored guarantee.
        signature.terminates_guarantee,
    )
}

pub(crate) fn lower_state_signature_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &syntax::identifier::Identifier,
    spelling: Option<syntax::operator_spelling::OperatorSpelling>,
    lifetime_parameters: &[syntax::identifier::Identifier],
    type_parameters: HandleSpan<syntax::item::TypeParameter>,
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
    native_callback_parameters: &[syntax::item::NativeCallbackParameterNode],
    return_type_handle: syntax::types::TypeReferenceHandle,
    is_default: bool,
    service_reach_is_installation_bound: bool,
    service_reach_keyword_source_spans: &[psi_source::SourceSpan],
    service_reaches: HandleSpan<syntax::identifier::Identifier>,
    invokes: HandleSpan<syntax::identifier::Identifier>,
    suspends_keyword_source_spans: &[psi_source::SourceSpan],
    blocks_keyword_source_spans: &[psi_source::SourceSpan],
    suspends: bool,
    blocks: bool,
    contracts: HandleSpan<syntax::item::CapabilityContract>,
    terminates_guarantee: bool,
) -> Result<LoweredStateSignature, Diagnostic> {
    let type_parameters =
        crate::data::lower_type_parameters(lowerer, syntax_trees, type_parameters)?;
    let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
    let return_type = return_type_handle
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
        .transpose()?;
    let service_reaches = lower_service_reach_names(syntax_trees, service_reaches);
    let invokes = lower_signature_invokes(lowerer, syntax_trees, invokes);
    let contracts = lower_signature_contracts(lowerer, syntax_trees, contracts)?;

    Ok(LoweredStateSignature {
        signature: StateSignature {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
            storage: StateSignatureStorage {
                spelling,
                lifetime_parameters: lifetime_parameters
                    .iter()
                    .map(crate::name::lower_name)
                    .collect(),
                type_parameters,
                is_default,
                parameters,
                native_callback_parameters: native_callback_parameters
                    .iter()
                    .map(|parameter| {
                        psi_symbol_resolved_trees::signature::NativeCallbackParameter {
                            name: crate::name::lower_name(&parameter.name),
                            binder: crate::name::lower_name(&parameter.binder),
                            native_ordinal: parameter.native_ordinal,
                        }
                    })
                    .collect(),
                return_type,
                invokes,
                service_reach_row: psi_language_semantics::ServiceReachRowId::NULL,
                service_reach_is_installation_bound,
                suspends_keyword_source_spans: suspends_keyword_source_spans.to_vec(),
                blocks_keyword_source_spans: blocks_keyword_source_spans.to_vec(),
                suspends,
                blocks,
                contracts,
                terminates_guarantee,
            },
        },
        service_reach_keyword_source_spans: service_reach_keyword_source_spans.to_vec(),
        service_reaches,
    })
}

pub(crate) struct LoweredStateSignature {
    pub(crate) signature: StateSignature,
    pub(crate) service_reach_keyword_source_spans: Vec<psi_source::SourceSpan>,
    pub(crate) service_reaches: Vec<DiagnosticName>,
}

pub(crate) fn lower_signature_invokes(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    invokes: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<psi_symbol_resolved_trees::name::DiagnosticName> {
    let mut span = HandleSpan::empty();
    for binding in syntax_trees.items.identifier_path_members(invokes) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .signature_invokes
            .append_to_span(&mut span, crate::name::lower_name(binding));
    }
    span
}

pub(crate) fn lower_service_reach_names(
    syntax_trees: &SyntaxTrees,
    service_reaches: HandleSpan<syntax::identifier::Identifier>,
) -> Vec<psi_symbol_resolved_trees::name::DiagnosticName> {
    syntax_trees
        .items
        .identifier_path_members(service_reaches)
        .iter()
        .map(crate::name::lower_name)
        .collect()
}

pub(crate) fn lower_signature_contracts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    contracts: HandleSpan<syntax::item::CapabilityContract>,
) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
    lower_signature_contracts_with_result_sum(lowerer, syntax_trees, contracts, None)
}

pub(crate) fn lower_machine_signature_contracts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    contracts: HandleSpan<syntax::item::CapabilityContract>,
    states: HandleSpan<Handle<psi_symbol_resolved_trees::state::State>>,
) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
    let has_outcome_rows = syntax_trees
        .items
        .capability_contracts(contracts)
        .iter()
        .any(|contract| {
            matches!(
                &contract.kind,
                syntax::item::CapabilityContractKind::EnsuresForResultCase { .. }
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
        Some(psi_symbol_resolved_trees::types::TypeReference::Named { name, .. }) => name,
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(generic)) => {
            &generic.base_name
        }
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
            psi_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                Some(variant.name.to_string())
            }
            psi_symbol_resolved_trees::data::DataMember::Field(_) => None,
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
    contracts: HandleSpan<syntax::item::CapabilityContract>,
    result_sum: Option<&ResultSumContractContext>,
) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for contract in syntax_trees.items.capability_contracts(contracts) {
        let (kind, pending_outcome) = match &contract.kind {
            syntax::item::CapabilityContractKind::Requires => {
                (SignatureContractKind::Requires, None)
            }
            syntax::item::CapabilityContractKind::Ensures => (SignatureContractKind::Ensures, None),
            syntax::item::CapabilityContractKind::EnsuresForResultCase { result_case } => {
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
                if result_name.as_str() != result_sum.data_name.as_str() {
                    return Err(Diagnostic::error(format!(
                        "outcome-specific ensures case `{result_name}::{case_name}` does not belong to declared result sum `{}`",
                        result_sum.data_name
                    )));
                }
                let result_case_name = result_sum
                    .variants
                    .iter()
                    .find(|name| name.as_str() == case_name.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "outcome-specific ensures names unknown case `{case_name}` of declared result sum `{}`",
                            result_sum.data_name
                        ))
                    })?;
                (
                    SignatureContractKind::EnsuresForResultCase {
                        result_data: psi_symbols::SymbolHandle::invalid(),
                        result_case: psi_symbols::SymbolHandle::invalid(),
                    },
                    Some((
                        result_sum.data_name.clone(),
                        result_name.source_span(),
                        result_case_name,
                    )),
                )
            }
            syntax::item::CapabilityContractKind::Crashes { cause } => (
                SignatureContractKind::Crashes {
                    cause: match cause {
                        syntax::item::CrashCause::Trap => {
                            psi_symbol_resolved_trees::signature::CrashCause::Trap
                        }
                        syntax::item::CrashCause::Abort => {
                            psi_symbol_resolved_trees::signature::CrashCause::Abort
                        }
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
                    binding: contract.binding.as_ref().map(crate::name::lower_name),
                    facts,
                    token_count: contract.token_count,
                },
            );
        if let Some((result_data_name, result_data_source_span, result_case_name)) = pending_outcome
        {
            lowerer.pending_outcome_specific_contracts.push(
                crate::lowerer::PendingOutcomeSpecificContract {
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

pub(crate) fn finalize_outcome_specific_contract_symbols(
    program: &mut psi_symbol_resolved_trees::SymbolResolvedTrees,
    pending: &[crate::lowerer::PendingOutcomeSpecificContract],
) -> Result<(), Diagnostic> {
    for pending in pending {
        let (result_data, result_case) = {
            let data_symbol = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    &pending.result_data_name,
                    &[psi_symbols::SymbolKind::Data],
                    pending.result_data_source_span,
                )
                .unwrap_or_else(SymbolHandle::invalid);
            let data = program
                .data_definitions
                .iter()
                .find(|data| data.symbol == data_symbol)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "outcome-specific ensures lost declared result sum `{}` during symbol assignment",
                        pending.result_data_name
                    ))
                })?;
            let result_case = program
                .data_members(data.members)
                .iter()
                .find_map(|member| match member {
                    psi_symbol_resolved_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == pending.result_case_name =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "outcome-specific ensures lost declared result case `{}::{}` during symbol assignment",
                        pending.result_data_name, pending.result_case_name
                    ))
                })?;
            (data.symbol, result_case)
        };
        let contract = program
            .tables
            .declarations
            .signature_contracts
            .get_mut(pending.contract);
        let SignatureContractKind::EnsuresForResultCase {
            result_data: contract_data,
            result_case: contract_case,
        } = &mut contract.kind
        else {
            return Err(Diagnostic::error(
                "outcome-specific ensures changed kind before symbol normalization",
            ));
        };
        *contract_data = result_data;
        *contract_case = result_case;
    }
    Ok(())
}

fn lower_state_statements(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statements: HandleSpan<syntax::statement::StatementHandle>,
) -> Result<HandleSpan<Statement>, Diagnostic> {
    let mut pending: Vec<Statement> = Vec::new();
    let mut run_start: Option<usize> = None;

    for statement in syntax_trees.items.statements(statements) {
        lower_statement_into_pending(
            lowerer,
            syntax_trees,
            *statement,
            &mut pending,
            &mut run_start,
        )?;
    }

    let mut span = HandleSpan::empty();
    for lowered in pending {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_statements
            .append_to_span(&mut span, lowered);
    }
    Ok(span)
}

fn lower_statement_into_pending(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: syntax::statement::StatementHandle,
    pending: &mut Vec<Statement>,
    run_start: &mut Option<usize>,
) -> Result<(), Diagnostic> {
    // A single syntax statement can lower to MULTIPLE resolved statements: an
    // assignment or local whose value reads a runtime-indexed element in
    // operand position hoists synthetic `let __hoist_N = arr[i];` temps that
    // must precede the rewritten statement in source order (later passes seed
    // and bind their symbols by statement order).
    //
    // DISPATCH-RUN SPLICE: a multi-arm transition block desugars into
    // CONSECUTIVE per-arm `Transition` statements, and both the
    // exhaustiveness pairing and the state-graph dispatch grouping key on
    // maximal runs of consecutive transitions. A LATER arm whose hoisted
    // `let`s landed between two arm transitions would SPLIT the run
    // (un-pairing `true`/`false` arms into a phantom fall-through), so the
    // lets of every arm after the first are spliced BEFORE the run's first
    // transition instead. The hoisted reads are pure loads; evaluating them
    // ahead of the whole dispatch has no observable effect.
    let mut lowered = lower_statement_handle(lowerer, syntax_trees, statement, pending.len())?;
    let ends_in_transition = matches!(lowered.last(), Some(Statement::Transition(_)));
    if ends_in_transition {
        let lets_count = lowered.len() - 1;
        match *run_start {
            Some(start) if lets_count > 0 => {
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

    Ok(())
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

pub(crate) fn lower_state_parameter(
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

/// The names of parameters declared as a SHARED reference to a NAMED type
/// (`table: &EfiSystemTable`) -- the shapes whose member reads must deref the
/// pointer slot rather than fold flat.
fn reference_struct_parameter_names(
    lowerer: &Lowerer,
    parameters: &HandleSpan<psi_symbol_resolved_trees::signature::StateParameter>,
) -> Vec<String> {
    lowerer
        .symbol_resolved_trees
        .state_parameters(*parameters)
        .iter()
        .filter_map(|parameter| {
            let psi_symbol_resolved_trees::types::TypeReference::Reference(reference) =
                &parameter.type_reference
            else {
                return None;
            };
            if reference.access.is_exclusive() {
                return None;
            }
            matches!(
                lowerer
                    .symbol_resolved_trees
                    .child_type_reference(reference.referee),
                psi_symbol_resolved_trees::types::TypeReference::Named { .. }
            )
            .then(|| parameter.name.as_str().to_string())
        })
        .collect()
}

/// Build one continuation state the guarded-arm value-call rewrite
/// synthesized: parameters copied (by name and type) from the enclosing
/// state, and a body of exactly the served let-bound spelling --
/// `let __hoist_N = call(..); transition { _ -> (__hoist_N) }` (the same
/// two statements `hoist_terminal_value_machine_call` mints for Always
/// arms; the call's Name arguments resolve against the SAME-named
/// parameters here).
pub(crate) fn build_synthesized_arm_state(
    lowerer: &mut Lowerer,
    arm: crate::lowerer::SynthesizedArmState,
) -> State {
    use psi_symbol_resolved_trees::expression::{ExpressionNode, TableNamePath};
    use psi_symbol_resolved_trees::statement::{
        LocalData, LocalDataStorage, Statement, Transition, TransitionGuard, TransitionTarget,
    };

    let mut parameters = HandleSpan::empty();
    for (name, type_reference) in arm.parameters {
        let parameter = StateParameter {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated(name),
            type_reference,
            is_const: false,
            is_mutable: false,
            is_self: false,
        };
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_parameters
            .append_to_span(&mut parameters, parameter);
    }

    let hoist_name = DiagnosticName::generated(lowerer.next_hoist_name());
    let hoist_local = Statement::LocalData(LocalData {
        symbol: SymbolHandle::invalid(),
        name: hoist_name.clone(),
        storage: LocalDataStorage {
            // Unit is the inference sentinel; the resolved -> typed lowering
            // types the temp from the callee's declared return.
            type_reference: TypeReference::Unit,
            initial_value: arm.call,
            is_mutable: false,
        },
    });
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, hoist_name);
    let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
    let terminal = expressions.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        is_self_value: false,
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
    }));
    let transition = Statement::Transition(Transition {
        target: TransitionTarget::Value(terminal),
        continuation: None,
        guard: TransitionGuard::Always,
        proof_selectors: Box::default(),
        exit: psi_symbol_resolved_trees::statement::TransitionExit::Ordinary,
        source_span: Default::default(),
    });

    let mut statements = HandleSpan::empty();
    for statement in [hoist_local, transition] {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_statements
            .append_to_span(&mut statements, statement);
    }

    State {
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated(arm.name),
        storage: StateStorage {
            parameters,
            return_type: Some(arm.return_type),
            contracts: HandleSpan::empty(),
            statements,
            statement_nodes: Default::default(),
        },
    }
}

/// Build an arm-selected continuation for guarded named-target value calls:
/// materialize every direct call argument in source order, then jump to the
/// original target with those result locals substituted at their positions.
pub(crate) fn build_synthesized_transition_argument_state(
    lowerer: &mut Lowerer,
    arm: crate::lowerer::SynthesizedTransitionArgumentState,
) -> State {
    use psi_symbol_resolved_trees::expression::{ExpressionNode, TableNamePath};
    use psi_symbol_resolved_trees::statement::{
        LocalData, LocalDataStorage, Statement, Transition, TransitionGuard, TransitionTarget,
    };

    let mut parameters = HandleSpan::empty();
    if let Some(mut self_parameter) = arm.self_parameter {
        self_parameter.symbol = SymbolHandle::invalid();
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_parameters
            .append_to_span(&mut parameters, self_parameter);
    }
    for (name, type_reference, is_mutable) in arm.parameters {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_parameters
            .append_to_span(
                &mut parameters,
                StateParameter {
                    symbol: SymbolHandle::invalid(),
                    name: DiagnosticName::generated(name),
                    type_reference,
                    is_const: false,
                    is_mutable,
                    is_self: false,
                },
            );
    }

    let target = arm.target;
    let mut statements = HandleSpan::empty();
    for call in arm.calls {
        let hoist_name = DiagnosticName::generated(lowerer.next_hoist_name());
        let call_initializer = lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .copy_from_self(call);
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .state_statements
            .append_to_span(
                &mut statements,
                Statement::LocalData(LocalData {
                    symbol: SymbolHandle::invalid(),
                    name: hoist_name.clone(),
                    storage: LocalDataStorage {
                        type_reference: TypeReference::Unit,
                        initial_value: call_initializer,
                        is_mutable: false,
                    },
                }),
            );

        let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let mut members = HandleSpan::empty();
        expressions.push_name_path_member(&mut members, hoist_name);
        let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
        let result = expressions.insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
        *expressions.expression_mut(call) = expressions.expression(result).clone();
    }

    lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .state_statements
        .append_to_span(
            &mut statements,
            Statement::Transition(Transition {
                target: TransitionTarget::Named(target),
                continuation: None,
                guard: TransitionGuard::Always,
                proof_selectors: Box::default(),
                exit: psi_symbol_resolved_trees::statement::TransitionExit::Ordinary,
                source_span: Default::default(),
            }),
        );

    State {
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated(arm.name),
        storage: StateStorage {
            parameters,
            return_type: arm.return_type,
            contracts: HandleSpan::empty(),
            statements,
            statement_nodes: Default::default(),
        },
    }
}
