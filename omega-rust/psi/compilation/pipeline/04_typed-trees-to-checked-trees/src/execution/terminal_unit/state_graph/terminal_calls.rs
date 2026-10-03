//! Terminal machine calls in transition arms
//! ([state contracts](../../../../../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)).
//!
//! An arm is classified by its resolved target: a local state is an internal
//! jump (`successor` plans its edge), while another machine is an ordinary
//! invocation whose normal result completes this machine. Two placements
//! exist, and neither bypasses the ordinary call plan:
//!
//! - An unconditional final arm (`transition { _ -> self.finish() }`) is the
//!   state's last operation. The shared statement sequence plans it through
//!   `call_occurrences::terminal_named_call`, exactly as a single-state body
//!   does, so its occurrence counts against the state's call roster; the
//!   terminator then completes after it.
//! - A guarded arm's call runs only when its arm is selected, so it cannot
//!   join the state's linear operations. `arm_call` plans it with the same
//!   `build_call_operation` as the arm's own operation
//!   (`CheckedConditionalReturnArm::Call`); emission places it in the arm's
//!   private completion block.
//!
//! Its subslice operands replay at the call-argument site the checker records
//! for an invocation (`values::scalar::expression_plans::transition`), like an
//! expression call's.
//!
//! The callee's normal result completes this machine, so Unit and primitive
//! scalar results are planned. A scalar result is the call's ordinary scalar
//! binding: the unconditional arm binds it as the state's returned value
//! (`CheckedScalarReturnPlan::Binding` at the transition), and a guarded arm
//! binds it at the next ordinal of the namespace its arm inherits and returns
//! it there. The two signatures must agree exactly; checking rejects a
//! terminal call whose result differs from the caller's declared result.
//!
//! Honest boundaries: structural results are not planned here; an arm call
//! is ordinary, transfers no claims, is its statement's only call (its guard
//! makes none), and has no operand that needs its own establishment (nested
//! calls, constructed values or arrays), because those operands would have to
//! run inside the arm as well.
use super::{
    CheckFacts, CheckedUnitStructuralParameterPlan, ScalarCalleePlans, TransitionExit,
    TransitionTargetNode, TypedTrees, control,
};
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::control::bind_scalar_call_result;
use crate::execution::terminal_unit::types::{is_unit, state_flow};

/// The machine state an arm's named target invokes, when that target is
/// another machine rather than a state of this one. Naming the enclosing
/// machine re-enters its entry state: that is a state-graph edge.
pub(super) fn invoked_machine_state(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    transition: &typed_trees::statement::TableTransition,
) -> Option<symbols::SymbolHandle> {
    if transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
        || !program
            .statement_table
            .transition_target_is_valid(transition.target)
    {
        return None;
    }
    let TransitionTargetNode::Named {
        path,
        static_machine_parameter,
        ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    if static_machine_parameter.is_valid() {
        return None;
    }
    let (owner, target) = if let Some(owner) = program.machine_by_symbol(path.symbol) {
        (owner, program.machine_states(owner).first()?)
    } else {
        crate::semantic::calls::find_state_with_machine(program, path.symbol)?
    };
    (owner.symbol != machine.symbol).then_some(target.symbol)
}

/// Plan a guarded arm's terminal machine call as the arm's own operation.
/// The arm's one flow call must be the invocation itself (preorder zero, so
/// the arm's guard makes no call before it), with no operand that needs a
/// separate establishment.
///
/// A scalar-result machine forwards the callee's result: the call binds it
/// at `next_scalar_binding`, the ordinal the arm's namespace (the state's
/// retained bindings) gives its next result, and the arm returns that value.
#[allow(clippy::too_many_arguments)]
pub(super) fn arm_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    result: &checked_trees::CheckedControlResultPlan,
    next_scalar_binding: usize,
    transition: &typed_trees::statement::TableTransition,
    statement_ordinal: u32,
    trace: &control::LocalConstructionTrace,
) -> Option<checked_trees::CheckedConditionalReturnArm> {
    let target = invoked_machine_state(program, machine, transition)?;
    trace.phase("state graph: terminator: terminal call arm: completion");
    trace.statement(Some(statement_ordinal));
    let target_state = program.state_by_symbol(target)?;
    // The callee's normal result is this machine's result, so the two
    // signatures must agree exactly; checking rejects any other pairing.
    let scalar_result = match result {
        checked_trees::CheckedControlResultPlan::Unit
            if is_unit(program, target_state.return_type) =>
        {
            None
        }
        checked_trees::CheckedControlResultPlan::Scalar { primitive_type }
            if program.primitive_type_reference(target_state.return_type)
                == Some(*primitive_type)
                && program.normalized_type_identity(target_state.return_type)
                    == program.normalized_type_identity(state.return_type) =>
        {
            Some(checked_trees::CheckedUnitScalarResultBindingPlan {
                statement_index: statement_ordinal,
                binding_ordinal: u32::try_from(next_scalar_binding).ok()?,
                primitive_type: *primitive_type,
            })
        }
        _ => return None,
    };
    trace.phase("state graph: terminator: terminal call arm: occurrence");
    trace.statement(Some(statement_ordinal));
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let mut occurrences = facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == statement_ordinal as usize
                && !facts.flow.control.is_retired(state.symbol, call)
        });
    let call = occurrences.next()?;
    // Flow custody retains the resolved authored symbol, which can name the
    // machine rather than its entry state. Compare executable destinations
    // without replacing that source identity in the ordinary call fact.
    if occurrences.next().is_some()
        || call.call_ordinal != 0
        || crate::semantic::calls::find_machine_head(program, call.target_symbol)
            .is_none_or(|(_, entry)| entry.symbol != target)
    {
        return None;
    }
    if !control::structural_operands::operations_for_call(program, facts, machine, state, call)?
        .is_empty()
    {
        return None;
    }
    trace.phase("state graph: terminator: terminal call arm: call operation");
    trace.statement(Some(statement_ordinal));
    let operation = build_call_operation(
        program,
        facts,
        Some(scalar_callees),
        machine,
        state,
        structural,
        &[],
        entry_claims,
        call,
        false,
        scalar_result.map(|result| ExpectedCallValueResult::Scalar(result.primitive_type)),
        &[],
        trace,
    )?;
    let planned = operation.call()?;
    if planned.is_boundary()
        || *planned.result() != checked_trees::CheckedUnitCallResult::Unit
        || !planned.claim_transfers().is_empty()
        || planned.coordinate.statement_index != statement_ordinal
    {
        return None;
    }
    let operation = match scalar_result {
        Some(result) => bind_scalar_call_result(facts, operation, result, false)?,
        None => operation,
    };
    Some(checked_trees::CheckedConditionalReturnArm::Call(operation))
}

/// Whether this state's authored tail holds a guarded arm that invokes
/// another machine. Such a body needs the state graph even with one state:
/// the linear single-state sequence cannot select which arm's call runs.
pub(super) fn has_guarded_machine_arm(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> bool {
    let statements = program.statement_table.statements(state.statement_nodes);
    let Some(start) = statements.iter().position(|statement| {
        matches!(
            statement,
            typed_trees::statement::StatementNode::Transition(_)
        )
    }) else {
        return false;
    };
    let tail = &statements[start..];
    tail.len() > 1
        && tail.iter().all(|statement| {
            matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            )
        })
        && tail.iter().any(|statement| {
            matches!(statement, typed_trees::statement::StatementNode::Transition(transition)
                if invoked_machine_state(program, machine, transition).is_some())
        })
}
