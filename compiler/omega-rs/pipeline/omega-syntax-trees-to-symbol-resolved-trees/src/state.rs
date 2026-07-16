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
    let statements = lower_state_statements(lowerer, syntax_trees, statements)?;
    lowerer.reference_struct_parameters = Vec::new();
    lowerer.current_state_parameter_names = Vec::new();

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
        signature.is_default,
        signature.effects,
        signature.contracts,
        // TPR4: the bodyless requirement's authored guarantee.
        signature.terminates_guarantee,
    )
}

fn lower_state_signature_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &syntax::identifier::Identifier,
    parameters: HandleSpan<syntax::item::StateParameterHandle>,
    return_type_handle: syntax::types::TypeReferenceHandle,
    is_default: bool,
    effects: HandleSpan<syntax::identifier::Identifier>,
    contracts: HandleSpan<syntax::item::CapabilityContract>,
    terminates_guarantee: bool,
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
            is_default,
            parameters,
            return_type,
            effects,
            contracts,
            terminates_guarantee,
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
    let mut lowered = lower_statement_handle(lowerer, syntax_trees, statement)?;
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

    let syntax::statement::StatementNode::Relax(relax) =
        syntax_trees.statements.statement(statement)
    else {
        return Ok(());
    };

    for nested in syntax_trees.items.statements(relax.statements) {
        lower_statement_into_pending(lowerer, syntax_trees, *nested, pending, run_start)?;
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
    parameters: &HandleSpan<omega_symbol_resolved_trees::signature::StateParameter>,
) -> Vec<String> {
    lowerer
        .symbol_resolved_trees
        .state_parameters(*parameters)
        .iter()
        .filter_map(|parameter| {
            let omega_symbol_resolved_trees::types::TypeReference::Reference(reference) =
                &parameter.type_reference
            else {
                return None;
            };
            if reference.is_mutable {
                return None;
            }
            matches!(
                lowerer
                    .symbol_resolved_trees
                    .child_type_reference(reference.referee),
                omega_symbol_resolved_trees::types::TypeReference::Named { .. }
            )
            .then(|| parameter.name.as_str().to_string())
        })
        .collect()
}
