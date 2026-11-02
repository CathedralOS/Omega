//! paths slice of loop_invariants.

use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Whether `path` stays unchanged throughout the natural loop. Direct writes
/// and calls (including nested value calls) are treated with the same path
/// overlap law as R5. An opaque call summary rejects the candidate.
pub(super) fn loop_preserves_path(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    states: &[State],
    loop_states: &[SymbolHandle],
    path: &str,
) -> bool {
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            return false;
        };
        if !state_preserves_path(program, machine, call_frames, state, path) {
            return false;
        }
    }
    true
}

/// Arrival-contract relations are established only at machine entry. To use
/// one as a loop invariant, its named place must survive every machine state,
/// including preheaders outside the natural loop; otherwise a preheader write
/// could make the contract fact stale before the first loop edge.
pub(super) fn machine_preserves_path(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    path: &str,
) -> bool {
    program
        .machine_states(machine)
        .iter()
        .all(|state| state_preserves_path(program, machine, call_frames, state, path))
}

pub(super) fn state_preserves_path(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    state: &State,
    path: &str,
) -> bool {
    for statement in program.statement_table.statements(state.statement_nodes) {
        if statement_may_write_path(program, machine, state, call_frames, statement, path)
            != Some(false)
        {
            return false;
        }
        if let Some(assignment) = statement.as_assignment() {
            let target = validation::canonical_storage_label(
                program,
                machine,
                program.state_parameters(state),
                &program.expression_table.display_name(assignment.target),
            );
            if validation::frame_paths_overlap(&target, path) {
                return false;
            }
        }
    }
    true
}

/// The counter upper bound carried by `source`'s arm to `head`. `None` if `source` reaches
/// `head` unguarded, via a non-`counter < B` guard, via both arms of one transition (an
/// unconditional convergence), via the continuation (`_`) arm (a negated guard gives no upper
/// bound), or via more than one transition (ambiguous).
pub(super) fn source_arm_to_head_upper_bound(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    source: &State,
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<i64> {
    single_head_bound_edge(
        program,
        machine,
        call_frames,
        source,
        head,
        |program, machine, source, guard| {
            parse_counter_upper_bound(program, machine, source, guard, counter)
        },
    )
}

/// The exclusive upper bound `B` a comparison guard puts on `counter`: `counter < L` -> `L`;
/// `counter <= L` -> `L + 1`; and the reversed `L > counter` / `L >= counter`. A `{ true -> .. }`
/// transition arm wraps its subject as `subject == true`, so an outer boolean equality is
/// unwrapped first (matching `seed_boolean_equality_guard_facts`). `None` otherwise.
pub(super) fn parse_counter_upper_bound(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    source: &State,
    guard: ExpressionHandle,
    counter: SymbolHandle,
) -> Option<i64> {
    if !validation::has_builtin_bound_expression_meaning(program, machine, Some(source), guard) {
        return None;
    }
    let binary = program.expression_table.binary_node(guard)?;
    if binary.operator == BinaryOperator::Equal {
        // `subject == true` -> parse the subject; `== false` is a negation (no upper bound).
        let inner = boolean_equality_inner(program, binary.left, binary.right)?;
        return parse_counter_upper_bound(program, machine, source, inner, counter);
    }
    let left_is_counter =
        expression_is_counter_member(program, machine, source, binary.left, counter);
    let right_is_counter =
        expression_is_counter_member(program, machine, source, binary.right, counter);
    match binary.operator {
        BinaryOperator::Less if left_is_counter => integer_literal(program, binary.right),
        BinaryOperator::LessOrEqual if left_is_counter => {
            integer_literal(program, binary.right).and_then(|literal| literal.checked_add(1))
        }
        BinaryOperator::Greater if right_is_counter => integer_literal(program, binary.left),
        BinaryOperator::GreaterOrEqual if right_is_counter => {
            integer_literal(program, binary.left).and_then(|literal| literal.checked_add(1))
        }
        _ => None,
    }
}
