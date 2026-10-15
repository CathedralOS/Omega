//! Checked discharge of authored `requires` premises in a call closure.
//!
//! An authored `requires` premise has no meaning outside an invocation, so
//! the common floor fences every call closure carrying one until ordinary
//! contract checking proves it. Two routes produce that proof here.
//!
//! A premise on the invoked machine or its entry state depends on the
//! invocation's arguments. Positions that own an authored call node —
//! constant initializers — build a checked probe around it. A const-position
//! leg such as a fixed-array length's `machine()` owns no call node, but the
//! invocation is just as concrete: the exact call is `machine()` at the leg's
//! own source span. Admission therefore builds the private probe itself,
//! appending a generated machine whose body is the synthesized call, and
//! treats a clean checked lowering with no retained crash routes as the
//! discharge evidence.
//!
//! Every other premise — on a callee, a later state, or a callable target —
//! sits at a source call or transition site whose proof reads only that
//! caller's own facts. When the entry carries no premise, a clean checked
//! lowering of the demanded checking closure proves it for any arguments, so an
//! argument-carrying policy or layout invocation needs no synthesized call.
//! Post-check evaluation borrows that judgment from the same checked program;
//! early typed-only evaluation still establishes it on the demanded closure.
//!
//! Every other floor axis — service reach, suspension, blocking,
//! termination, linear carriers, declaration-selection authority — is
//! enforced unchanged.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};

use super::BuildTimeProgram;
use super::closure_validation::has_authored_requires;

/// Whether a private checked probe discharges `machine`'s authored
/// `requires` premises for the concrete zero-argument call `machine()`.
/// Returns `true` only when the synthesized call survives ordinary checked
/// lowering and the probe machine retains no unhandled crash routes; a
/// premise that ordinary checking cannot prove at that call keeps the
/// conservative closure fence.
pub(super) fn zero_argument_invocation_discharges(
    program: &TypedTrees,
    machine: &Machine,
    source_span: Option<source::SourceSpan>,
) -> bool {
    let mut probe = program.clone();
    // The probe runs inside the preliminary package checkpoint: deferred
    // range-endpoint marks name the outer const-eval continuation, not this
    // private program, so they must not survive as a tolerated lost fold.
    probe.pending_const_range_endpoints.clear();

    let Some((entry_symbol, entry_name, entry_return)) = entry_state(program, machine)
        .filter(|entry| program.state_parameters(entry).is_empty())
        .map(|entry| (entry.symbol, entry.name.clone(), entry.return_type))
    else {
        return false;
    };

    let call = TableCallExpression {
        authored_machine_application: Default::default(),
        requirement_application: Default::default(),
        exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
        receiver: ExpressionHandle::invalid(),
        target_symbol: entry_symbol,
        static_machine_parameter: SymbolHandle::invalid(),
        target: entry_name,
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        arguments: probe.expression_table.insert_expression_handles([]),
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement::default(),
    };
    let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
    let call_span = source_span
        .or_else(|| probe.symbols.symbol_source_span(machine.symbol))
        .unwrap_or_default();
    probe.expression_table.set_source_span(concrete, call_span);
    let owner = append_invocation_probe(
        &mut probe,
        machine.symbol,
        "@const-length",
        concrete,
        entry_return,
    );
    // The const-eval window sits inside preliminary package checking: the
    // owning program has not reached settlement, so the probe lowers in the
    // same mode. Contract checking is not gated on the mode, so authored
    // `requires` premises are still decided at the synthesized call.
    let preparation_request = typed_trees_to_typed_trees::PreparationRequest::preliminary();
    let checking = typed_trees_to_checked_trees::CheckingRequest::new();
    let Ok((probe, preparation)) =
        typed_trees_to_typed_trees::prepare_typed_trees(probe, &preparation_request)
    else {
        return false;
    };
    let Ok(checked) =
        typed_trees_to_checked_trees::lower_typed_trees(probe, preparation, &checking)
    else {
        return false;
    };
    matches!(
        typed_trees_to_checked_trees::infer_checked_machine_crash_causes(
            &checked.typed,
            &checked.facts,
            owner,
        ),
        Some(causes) if causes.is_empty()
    )
}

/// Whether ordinary checking alone discharges every authored `requires`
/// premise in `machine`'s call closure, whatever the invocation's arguments.
/// Holds only when neither the machine nor its entry state carries an
/// authored premise and ordinary checking has established its internal calls.
/// Reuse that judgment when supplied; otherwise check the demanded closure.
/// An entry premise keeps its concrete routes or the conservative fence.
pub(super) fn argument_independent_closure_discharges(
    input: BuildTimeProgram<'_>,
    machine: &Machine,
) -> Result<bool, String> {
    let program = input.typed();
    let entry_is_unconditional = !has_authored_requires(program.machine_contracts(machine))
        && entry_state(program, machine)
            .is_some_and(|entry| !has_authored_requires(program.state_contracts(entry)));
    if !entry_is_unconditional {
        return Ok(false);
    }
    // The same immutable checked program already established all internal
    // call/transition premises. This proves nothing about entry arguments,
    // hence the unconditional-entry test above remains mandatory. The caller
    // still checks every independent execution-admission axis.
    if matches!(input, BuildTimeProgram::Checked(_)) {
        return Ok(true);
    }
    // A layout policy may be admitted before unrelated runtime declarations
    // have their authoritative layout/accessor plans. Those declarations are
    // not premises of this invocation. Close the root's real type, proof,
    // operator and body dependencies through the shared evaluation projection;
    // retain their contracts unchanged and check that exact demanded program.
    let domain = crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(
        program,
        &[machine.symbol],
        &[],
    );
    let mut probe = domain.execution_program(program);
    // Same preliminary-package window as the zero-argument probe above.
    probe.pending_const_range_endpoints.clear();
    let preparation_request = typed_trees_to_typed_trees::PreparationRequest::preliminary();
    let checking = typed_trees_to_checked_trees::CheckingRequest::new();
    let (probe, preparation) =
        typed_trees_to_typed_trees::prepare_typed_trees(probe, &preparation_request)
            .map_err(|error| format!("demanded premise closure preparation failed: {error:?}"))?;
    typed_trees_to_checked_trees::lower_typed_trees(probe, preparation, &checking)
        .map_err(|error| format!("demanded premise closure checking failed: {error:?}"))?;
    Ok(true)
}

/// The state an invocation of `machine` enters: the one named by the
/// machine's leaf, else its first state.
fn entry_state<'a>(program: &'a TypedTrees, machine: &Machine) -> Option<&'a State> {
    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == leaf)
        .or_else(|| states.first())
}

/// One call operand a snapshot probe substitutes: evaluated literal nodes
/// replace authored arguments; an operand with no literal keeps its authored
/// expression — a payloadless NaN has no literal spelling but stays
/// dischargeable through the retained computation.
pub(crate) struct SnapshotArgument {
    /// Whether this operand occupies the call's receiver position rather
    /// than its argument span.
    pub receiver: bool,
    /// The authored operand in the source program.
    pub original: ExpressionHandle,
    /// The evaluated literal node to substitute, when evaluation produced one.
    pub literal: Option<ExpressionNode>,
}

/// Whether `expression`'s authored call survives ordinary checked lowering
/// with its operands substituted by evaluated snapshots — the concrete
/// discharge evidence for authored `requires` premises on the invoked
/// machine or its entry state, and for any unhandled failure route at the
/// exact call. A published guarded ceiling has no private whole-body
/// summary, and an authored premise has no meaning outside an invocation,
/// so admission checks the exact call rather than inspecting the callee
/// body; successful interpretation is never admission evidence.
/// `retained` names the machines the probe program keeps. Both current
/// callers keep the whole authored program (every non-probe machine):
/// library satisfaction references and declaration reach read machines far
/// outside the callee's call closure, while still-spelled endpoint bounds
/// get closed stand-ins rather than membership filtering.
/// `endpoint_stand_ins` carries the caller-computed closed literal each
/// deferred range bound is rewritten to — the marks themselves are dropped
/// with `pending_const_range_endpoints`.
pub(crate) fn snapshot_arguments_discharge(
    program: &TypedTrees,
    retained: &[SymbolHandle],
    owner: SymbolHandle,
    probe_name: &str,
    preparation_request: &typed_trees_to_typed_trees::PreparationRequest,
    checking: &typed_trees_to_checked_trees::CheckingRequest,
    expression: ExpressionHandle,
    callee: &str,
    return_type: typed_trees::types::TypeReferenceHandle,
    snapshots: Vec<SnapshotArgument>,
    endpoint_stand_ins: &[(ExpressionHandle, ExpressionNode)],
) -> Result<(), String> {
    // All original probes must leave this private body check: their detached
    // expressions use exact evaluation, not runtime ranges. Deferred
    // range-endpoint marks name the outer const-eval continuation, not this
    // private program, so they must not survive as a tolerated lost fold;
    // the caller supplies each still-spelled bound's closed stand-in so the
    // signature that carries it stays checkable here.
    let mut probe = program.clone();
    // The endpoint call itself may be a marked bound: capture its spelling
    // before a stand-in rewrite replaces it inside the signature.
    let ExpressionNode::Call(mut call) = probe.expression_table.expression(expression).clone()
    else {
        return Err("constant invocation lost its selected call".into());
    };
    probe.pending_const_range_endpoints.clear();
    for (endpoint, stand_in) in endpoint_stand_ins {
        *probe.expression_table.expression_mut(*endpoint) = stand_in.clone();
    }
    let machines = probe
        .machines()
        .iter()
        .filter(|candidate| retained.contains(&candidate.symbol))
        .cloned()
        .collect::<Vec<_>>();
    probe.roots.machines = arena::HandleSpan::empty();
    for candidate in machines {
        probe.push_machine(candidate);
    }
    let mut arguments = Vec::new();
    for snapshot in snapshots {
        let expression = match snapshot.literal {
            Some(literal) => probe.expression_table.insert(literal),
            None => snapshot.original,
        };
        if snapshot.receiver {
            call.receiver = expression;
        } else {
            arguments.push(expression);
        }
    }
    call.arguments = probe.expression_table.insert_expression_handles(arguments);
    let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
    probe
        .expression_table
        .set_source_span(concrete, program.expression_table.source_span(expression));
    let owner = append_invocation_probe(&mut probe, owner, probe_name, concrete, return_type);
    let checked = typed_trees_to_typed_trees::prepare_typed_trees(probe, preparation_request)
        .and_then(|(probe, preparation)| {
            typed_trees_to_checked_trees::lower_typed_trees(probe, preparation, checking)
        })
        .map_err(|diagnostics| {
            format!("constant invocation of `{callee}` failed checking: {diagnostics:?}")
        })?;
    let causes = typed_trees_to_checked_trees::infer_checked_machine_crash_causes(
        &checked.typed,
        &checked.facts,
        owner,
    )
    .ok_or("constant invocation has no complete checked failure summary")?;
    if !causes.is_empty() {
        return Err(format!(
            "constant invocation of `{callee}` retains unhandled {causes:?} routes at its concrete arguments"
        ));
    }
    Ok(())
}

/// Append the generated probe machine whose single state returns the
/// synthesized call: `machine @const-length() -> <entry return> { machine() }`.
pub(super) fn append_invocation_probe(
    probe: &mut TypedTrees,
    owner: SymbolHandle,
    name: &str,
    expression: ExpressionHandle,
    destination: typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    let symbol = probe
        .symbols
        .insert_generated_root_from(owner, SymbolKind::Machine, name);
    let children = probe
        .symbols
        .insert_generated_children(symbol, [(SymbolKind::State, name)]);
    let target = probe
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(expression));
    let mut state = State {
        symbol: children.start(),
        name: typed_trees::name::Identifier::generated(name),
        return_type: destination,
        ..Default::default()
    };
    let source_span = probe.expression_table.source_span(expression);
    probe.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span,
            ..Default::default()
        }),
    );
    let mut probe_machine = Machine {
        symbol,
        name: typed_trees::name::Identifier::generated(name),
        ..Default::default()
    };
    probe.push_machine_state(&mut probe_machine, state);
    probe.push_machine(probe_machine);
    symbol
}
