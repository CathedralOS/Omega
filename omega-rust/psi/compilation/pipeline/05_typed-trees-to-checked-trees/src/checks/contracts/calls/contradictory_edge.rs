//! Discharge an arrival requirement on the arm no execution can reach.
//!
//! A conditional-entry assembly body normalizes its `jz`/`jnz` into one `When`
//! transition plus the unguarded continuation that follows it, and each
//! destination label's `requires` becomes an arrival obligation on the arm that
//! delivers it. When the state's own premises already refute the guard -- `cmp
//! eax, eax` always sets ZF, so the fallthrough cannot run -- the arm is
//! infeasible: `A ∧ ¬G` has no model, so every requirement follows, including
//! the literal `false`.
//!
//! A label keeps its meaning. This discharges a requirement only where the edge
//! that delivers it cannot be taken, so an interior `jmp` to the same label is
//! untouched and still owes that label's requirement in full.
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::ProgramPoint;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

/// The guard this call's statement is the refuted arm of -- the negation of the
/// immediately preceding `When` transition. Only the continuation of a dispatch
/// the previous statement opens qualifies, so the guard was evaluated on this
/// edge with no statement between it and the arrival.
fn refuted_edge_guard(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let (_, state) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    )?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Transition(continuation) = statements.get(call_flow.statement_index)? else {
        return None;
    };
    // A selected arm carries its own guard and proves its requirement through
    // it; only the unguarded arm of a dispatch is a refutation.
    if !matches!(continuation.guard, TransitionGuardNode::Always) {
        return None;
    }
    let index = call_flow.statement_index.checked_sub(1)?;
    let StatementNode::Transition(selected) = statements.get(index)? else {
        return None;
    };
    let TransitionGuardNode::When(guard) = selected.guard else {
        return None;
    };
    // A `When` that names a continuation sends the refuted arm there itself;
    // only an open dispatch -- guard, no continuation -- leaves the following
    // transition as the arm this state runs when the guard failed.
    (guard.is_valid() && !selected.continuation.is_valid()).then_some(guard)
}

/// `A ∧ ¬G` has no model: the premises this call already proves from refute
/// the guard of the arm it arrives on, so the arrival cannot happen.
pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(guard) = refuted_edge_guard(program, state_flow, call_flow) else {
        return false;
    };
    // The guard was read before the arm's operands ran. Those operands are the
    // only statements between it and this arrival, so their writes decide
    // whether its negation still names the delivered value.
    if !super::super::guard_operands::premise_reads_survive_arm_operand_writes(
        program,
        facts,
        state_flow,
        call_flow,
        guard,
        call_frames,
    ) {
        return false;
    }
    let Some(expression) = crate::facts::boolean_predicates::lower_current_guard(
        program,
        &facts.operators,
        state_flow.state_symbol,
        call_flow.statement_index,
        guard,
    ) else {
        return false;
    };
    // Bind the refutation itself: this arrival means the guard evaluated false.
    let Some(refuted) = crate::facts::boolean_predicates::bind_current_expression(
        program,
        state_flow.state_symbol,
        expression,
        false,
        |operand| {
            crate::facts::boolean_predicates::declared_observation(
                program,
                state_flow.state_symbol,
                call_flow.statement_index,
                operand,
            )
        },
    ) else {
        return false;
    };
    // The premise roster is the one this call's other Boolean routes read, at
    // this exact statement: a fact that arrives here and holds here.
    let point = ProgramPoint::Statement {
        machine_symbol: state_flow.machine_symbol,
        state_symbol: state_flow.state_symbol,
        statement_index: call_flow.statement_index,
    };
    let mut premises = crate::facts::boolean_predicates::retained(
        program,
        &facts.operators,
        &facts.semantic,
        entry_contexts,
        point,
    );
    if premises.is_empty() {
        return false;
    }
    premises.push(refuted);
    crate::facts::boolean_predicates::unsatisfiable(&premises)
}
