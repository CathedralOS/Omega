//! Scalar operand calls belong to computation roots; structural operands retain
//! their own result producers within the same authored statement.
//! Assignment selectors and the stored value own their calls through those
//! exact roots, independently of the destination's spelling. This roster only
//! accounts for the calls: the store planner and emitter still establish the
//! destination and evaluate selectors before the value, once in authored order.
use super::{CheckFacts, StatementNode, SymbolHandle};
use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, machine_binders};

use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;

use checked_trees::CheckedScalarComputationHandle;

struct ComputationVisit {
    computation: CheckedScalarComputationHandle,
    completed: bool,
}

/// All consumers of one statement share exact computation occurrence custody.
/// A retained value can be reached through several roles without executing its
/// child twice. Different computations still cannot consume one captured call.
#[derive(Default)]
struct CallOccurrenceLedger {
    calls: Vec<arena::Handle<checked_trees::FlowCallFact>>,
    accounted_computations: Vec<AccountedComputation>,
    pending_minima: Vec<Option<u32>>,
}

struct AccountedComputation {
    state: SymbolHandle,
    statement: usize,
    computation: CheckedScalarComputationHandle,
    /// Smallest actual Source-call ordinal in this exact immutable subtree.
    /// Pure computations have no call ordinal to constrain a consumer.
    minimum_call_ordinal: Option<u32>,
}

impl CallOccurrenceLedger {
    fn record_minimum(&mut self, minimum: Option<u32>) {
        if let Some(minimum) = minimum {
            for pending in &mut self.pending_minima {
                *pending = Some(pending.map_or(minimum, |prior| prior.min(minimum)));
            }
        }
    }
    fn contains(&self, call: &arena::Handle<checked_trees::FlowCallFact>) -> bool {
        self.calls.contains(call)
    }
    fn push(&mut self, call: arena::Handle<checked_trees::FlowCallFact>) {
        self.calls.push(call);
    }
    /// Record one call occurrence once: a walk that reaches the same authored
    /// call twice records it at its first position and drops the repeat.
    fn record_once(&mut self, call: arena::Handle<checked_trees::FlowCallFact>) {
        crate::collections::push_unique(&mut self.calls, call);
    }
    fn iter(&self) -> std::slice::Iter<'_, arena::Handle<checked_trees::FlowCallFact>> {
        self.calls.iter()
    }
}

/// An unconditional machine invocation completes this body after
/// its ordinary call operation. Internal state destinations remain control
/// edges; guarded invocations must not be hoisted into this linear sequence.
/// Return the authored identity to join captured occurrences; the call builder
/// separately resolves the callee's executable entry and signature.
mod collecting;
mod traced;

pub(in crate::execution::terminal_unit) use collecting::call_owns_statement_computations;
pub(in crate::execution::terminal_unit) use traced::outer_calls_in_prefix_traced;

pub(in crate::execution::terminal_unit) fn terminal_invocation(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<SymbolHandle> {
    use typed_trees::statement::{TransitionExit, TransitionGuardNode};
    let statements = typed_trees::statement::reachable_state_statements(
        program.statement_table.statements(state.statement_nodes),
    );
    if statement_index.checked_add(1)? != statements.len() {
        return None;
    }
    let transition = statements.get(statement_index)?.as_transition()?;
    if transition.guard != TransitionGuardNode::Always
        || transition.continuation.is_valid()
        || transition.exit != TransitionExit::Ordinary
        || !program
            .statement_table
            .transition_target_is_valid(transition.target)
    {
        return None;
    }
    // A paired `Always` arm is the fallthrough of a guarded tail, not an
    // unconditional completion: its arm keeps its guard context on the
    // conditional-successor routes rather than sequencing as the body's
    // completing call.
    if statements[..statement_index]
        .iter()
        .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return None;
    }
    let invocation = validation::completing_transition_invocation(program, transition.target)?;
    // The invocation selects the callee as its machine or as its entry
    // state (a body-only machine resolves to the state itself); the flow
    // call facts name the entry state either way, so the terminal call is
    // identified by that resolved callable, not by the authored spelling.
    let (callable, return_type) =
        validation::terminal_callable_target(program, machine, invocation.target)?;
    // Result custody belongs to the shared sequence: a retained value root
    // owns its call computation, while an invocation without that root forwards
    // its ordinary bound result. Unit has no value root. Classify every result
    // here; the sequence schedules exactly one of those completion owners.
    (program.normalized_type_identity(return_type)
        == program.normalized_type_identity(state.return_type))
    .then_some(callable)
}

pub(in crate::execution::terminal_unit) fn tail_call<'a>(
    program: &'a dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    &'a typed_trees::expression::TableCallExpression,
)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    if !is_unit(program, state.return_type) || statement_index.checked_add(1)? != statements.len() {
        return None;
    }
    let expression = statements.get(statement_index)?.as_expression()?;
    if !program.expression_table.expression_is_valid(*expression) {
        return None;
    }
    let call = program.expression_table.call_node(*expression)?;
    Some((*expression, call))
}

/// A Unit-result call can occupy a statement without being the state's tail.
/// Tail consumers retain their existing final-statement check above.
pub(in crate::execution::terminal_unit) fn unit_statement_call<'a>(
    program: &'a dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    &'a typed_trees::expression::TableCallExpression,
)> {
    if let Some(call) = tail_call(program, state, statement_index) {
        return Some(call);
    }
    let expression = program
        .statement_table
        .statement_at(state, statement_index)?
        .as_expression()?;
    if !program.expression_table.expression_is_valid(*expression) {
        return None;
    }
    let call = program.expression_table.call_node(*expression)?;
    validation::unit_statement_call_is_admitted(program, machine, state, *expression)
        .then_some((*expression, call))
}

pub(super) fn ordered_statement_call<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    &'program typed_trees::expression::TableCallExpression,
)> {
    if let Some(call) = unit_statement_call(program, machine, state, statement_index) {
        return Some(call);
    }
    let expression = program
        .statement_table
        .statement_at(state, statement_index)?
        .as_expression()?;
    let call = program.expression_table.call_node(*expression)?;
    if !crate::values::is_scalar_return_call(program, state, *expression) {
        // A final structural call is an ordinary result-producing operation.
        // Its value/claim custody is checked by statement sequencing, not by
        // treating all non-scalar expression statements as Unit calls.
        if statement_index.checked_add(1)?
            != program
                .statement_table
                .statements(state.statement_nodes)
                .len()
            || program.normalized_type_identity(validation::call_target_return_type(
                program,
                validation::call_target_symbol(program, call),
            )?) != program.normalized_type_identity(state.return_type)
            || checked_structural_result_type(
                program,
                &mut ShapeCollector::new(program),
                state.return_type,
                &machine_binders(program, machine),
            )
            .is_none()
        {
            return None;
        }
    }
    Some((*expression, call))
}

/// `outer_calls_in_prefix` over the whole body, tracing where admission stopped.
pub(in crate::execution::terminal_unit) fn outer_calls_traced<'a>(
    program: &dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
    trace: &LocalConstructionTrace,
) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
    outer_calls_in_prefix_traced(
        program,
        facts,
        machine,
        state,
        calls,
        std::ops::Bound::Unbounded,
        trace,
    )
}

/// Authenticate the outer invocation of an unconditional terminal call. Nested
/// operands can share its declaration but never inherit its control position.
pub(in crate::execution::terminal_unit) fn is_terminal_call_occurrence(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> bool {
    let Some(expected) = terminal_invocation(program, machine, state, call.statement_index) else {
        return false;
    };
    if validation::terminal_callable_target(program, machine, call.target_symbol)
        .is_none_or(|(target, _)| target != expected)
    {
        return false;
    }
    let Some(transition) = program
        .statement_table
        .statement_at(state, call.statement_index)
        .and_then(StatementNode::as_transition)
    else {
        return false;
    };
    validation::find_call_site_in_state(
        program,
        machine,
        state,
        call.statement_index,
        call.call_ordinal,
    )
    .is_some_and(|site| site.completes_transition(program, transition.target))
}
