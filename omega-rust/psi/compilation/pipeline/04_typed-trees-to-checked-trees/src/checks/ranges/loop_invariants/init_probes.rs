//! init probes slice of loop_invariants.

use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// G2 + G4: every loop-ENTRY edge must assign `counter = K` for a constant
/// integer literal `K`. Returns `(min, max)` over the entry inits.
pub(super) fn entry_constant_init_range(
    program: &dyn ::validation::CheckingInput,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    counter_path: &str,
) -> Option<(i64, i64)> {
    let mut min_init: Option<i64> = None;
    let mut max_init: Option<i64> = None;
    let mut saw_entry = false;

    for edge in edges.iter().filter(|edge| edge.target == head) {
        if loop_states.contains(&edge.source) {
            continue; // a back edge -- not an entry
        }
        saw_entry = true;
        let init = counter_entry_init(
            program,
            machine,
            call_frames,
            states,
            edges,
            loop_states,
            edge.source,
            counter,
            counter_path,
        )?;
        min_init = Some(min_init.map_or(init, |current| current.min(init)));
        max_init = Some(max_init.map_or(init, |current| current.max(init)));
    }

    if !saw_entry {
        return None;
    }
    Some((min_init?, max_init?))
}

/// A state's net effect on `counter`, for the entry-init back-walk.
pub(super) enum InitProbe {
    /// The state's last effect sets `counter` to this constant integer literal.
    Constant(i64),
    /// The state sets `counter` to a non-constant, or makes a call / bare expression that
    /// may clobber it through `&mut self` -- the init is not a clean constant here.
    Unknown,
    /// The state does not touch `counter`; the walk may continue back through it.
    Untouched,
}

/// The constant `counter` holds at the loop head's entry along the chain ending at `start` (an
/// entry-edge source). Walks back through single-predecessor, counter-untouched states until the
/// constant init is found. `None` if any state on the chain sets `counter` to a non-constant or
/// may clobber it, the chain branches (more than one distinct predecessor) or reaches a loop
/// state, or no init is found. Strictly extends the old immediate-predecessor check: a
/// predecessor that DOES set the counter is classified exactly as before; only the previously
/// dead "predecessor does not set it" case now walks further back.
pub(super) fn counter_entry_init(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    start: SymbolHandle,
    counter: SymbolHandle,
    counter_path: &str,
) -> Option<i64> {
    let mut visited: Vec<SymbolHandle> = Vec::new();
    let mut current = start;
    loop {
        let state = find_state(states, current)?;
        match probe_state_init(program, machine, call_frames, state, counter, counter_path) {
            InitProbe::Constant(value) => return Some(value),
            InitProbe::Unknown => return None,
            InitProbe::Untouched => {}
        }
        if visited.contains(&current) {
            return None; // a cycle with no init
        }
        visited.push(current);
        // Continue only to a single distinct predecessor that lies outside the loop.
        let mut sources: Vec<SymbolHandle> = Vec::new();
        for edge in edges.iter().filter(|edge| edge.target == current) {
            crate::collections::push_unique(&mut sources, edge.source);
        }
        match sources.as_slice() {
            [single] if !loop_states.contains(single) => current = *single,
            _ => return None,
        }
    }
}

/// Classify a state's net effect on `counter` (last write wins), for `counter_entry_init`.
pub(super) fn probe_state_init(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    state: &State,
    counter: SymbolHandle,
    counter_path: &str,
) -> InitProbe {
    let mut probe = InitProbe::Untouched;
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement_may_write_path(
            program,
            machine,
            state,
            call_frames,
            statement,
            counter_path,
        ) {
            Some(true) | None => probe = InitProbe::Unknown,
            Some(false) => {}
        }
        if let Some(assignment) = statement.as_assignment() {
            if assignment_counter_field(program, machine, state, assignment) != Some(counter) {
                let disjoint = call_frames
                    .assignment_write_frame(machine, statement)
                    .into_complete_paths()
                    .is_some_and(|writes| {
                        writes.iter().all(|path| {
                            !validation::frame_paths_overlap(
                                &validation::canonical_storage_label(
                                    program,
                                    machine,
                                    program.state_parameters(state),
                                    path,
                                ),
                                counter_path,
                            )
                        })
                    });
                if !disjoint {
                    probe = InitProbe::Unknown;
                }
                continue;
            }
            // Check the original RHS before interpreting an initializer as a
            // constant entry bound, just as for the in-loop arithmetic writes.
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                assignment.value,
            ) {
                probe = InitProbe::Unknown;
                continue;
            }
            probe = match program.expression_table.expression(assignment.value) {
                ExpressionNode::Integer(value) => match value.value_i64() {
                    Some(value) => InitProbe::Constant(value),
                    None => InitProbe::Unknown,
                },
                _ => InitProbe::Unknown,
            };
        }
    }
    probe
}

/// Whether calls evaluated by `statement` may write `path`, which arrives
/// canonical. The aggregate value-call frame is checked for every statement; a
/// statement-position call adds its own frame. `None` is an opaque analysis and
/// therefore blocks the invariant.
///
/// A frame renders its written places in the calling state's own spelling, so
/// each one is canonicalized before the overlap test. Comparing a frame's
/// `main.i` against a canonical `self.i` as plain text would report no overlap
/// and call a written path preserved.
pub(super) fn statement_may_write_path(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    call_frames: &validation::CallFrameResolver<'_>,
    statement: &StatementNode,
    path: &str,
) -> Option<bool> {
    let overlaps = |written: &[String]| {
        written.iter().any(|written| {
            validation::frame_paths_overlap(
                &validation::canonical_storage_label(
                    program,
                    machine,
                    program.state_parameters(state),
                    written,
                ),
                path,
            )
        })
    };
    let value_writes = call_frames.statement_value_may_write_paths(machine, statement)?;
    if overlaps(&value_writes) {
        return Some(true);
    }
    let Some(call) = statement.as_call() else {
        return Some(false);
    };
    Some(overlaps(&call_frames.may_write_paths(machine, call)?))
}

/// The canonical label of `counter` as an index, sourced from an actual
/// monotone-write LHS so it names the place the index use names. The label is
/// receiver-rooted whatever the writing state spells, because the invariant it
/// keys is carried to other states and compared against their paths.
pub(super) fn counter_display_name(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    states: &[State],
    loop_states: &[SymbolHandle],
    counter: SymbolHandle,
) -> Option<String> {
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            let Some(assignment) = statement.as_assignment() else {
                continue;
            };
            if assignment_counter_field(program, machine, state, assignment) != Some(counter) {
                continue;
            }
            if expression_is_counter_member(program, machine, state, assignment.target, counter) {
                return Some(validation::canonical_storage_label(
                    program,
                    machine,
                    program.state_parameters(state),
                    &program.expression_table.display_name(assignment.target),
                ));
            }
        }
    }
    None
}
