use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableAssignment, TransitionTargetHandle, TransitionTargetNode,
};

use super::facts::RangeFacts;

mod update_bounds;

use graph::*;
use head_bounds::*;
use init_probes::*;
use paths::*;
mod graph;
mod head_bounds;
mod init_probes;
mod paths;

/// A sound, inductive loop-invariant range fact for a MONOTONE counter.
///
/// For a loop head `H` (a state reached by a back edge from inside its own
/// cycle), a counter field `i` that is
///   (1) established to a constant `K` on every loop-entry edge (a predecessor
///       of `H` OUTSIDE the loop assigns `self.i = K`), and
///   (2) modified inside the loop ONLY in one direction -- every write is a
///       decrement `self.i = self.i - c`, or every write is an increment
///       `self.i = self.i + c` (`c` a positive integer literal), nothing else,
///       with the selected policy preserving order on normal completion
///       (Wrapping additionally needs an independent no-wrap proof per update),
/// satisfies a one-sided bound at every loop state:
///   * DECREASING (init `K`): `i < K + 1` -- entry gives `i = K < K + 1`, each
///     decrement preserves it. Seeded as an index UPPER bound (so `arr[i]` proves
///     when `K + 1 <= capacity`; the index check still requires `i < capacity`).
///   * INCREASING (init `M >= 0`): `i >= 0` -- entry gives `i = M >= 0`, each
///     increment preserves it (the counter only grows from a non-negative start,
///     so this IS a blanket loop invariant, unlike a decreasing counter's lower
///     bound). Seeded as a NON-NEGATIVE fact, the lower half of a SIGNED index
///     obligation for an `i = 0; i < len; i = i + 1` loop with no `>= 0` guard.
pub(super) struct LoopInvariant {
    state: SymbolHandle,
    /// The canonical (receiver-rooted) label of the counter as an index.
    /// Seeding renders it back into the seeded state's own spelling, so it
    /// matches `expression_table.display_name` of the index expression there.
    index_name: String,
    kind: InvariantKind,
}

#[derive(Clone)]
enum InvariantKind {
    /// `i < bound` (exclusive). Decreasing counter.
    UpperBound(i64),
    /// `i >= 0`. Increasing counter with a non-negative init.
    NonNegative,
    /// `i < collection.len`. Every incoming edge to the loop head establishes
    /// the same symbolic relation, and the collection place is stable through
    /// the loop. Kept collection-relative rather than collapsed to a constant.
    IndexWithin(String),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum UpperTerm {
    /// `counter < self.collection.len`.
    CollectionLength(String),
    /// `counter < self.bound`.
    Place(String),
}

#[derive(Clone, PartialEq, Eq)]
struct RelationalUpper {
    term: UpperTerm,
    /// `true` for `<` / `>`, `false` for `<=` / `>=`.
    strict: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AuthoredUpperRelation {
    lower: String,
    upper: UpperTerm,
    strict: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BoundCollectionChain {
    collection: String,
    strict: bool,
}

/// One write a state performs to the counter field, classified for G1.
#[derive(PartialEq)]
enum CounterWrite {
    /// `self.i = self.i - c`, `c` a positive integer literal. Preserves an upper
    /// bound (decreasing).
    Decrement,
    /// `self.i = self.i + c` / `self.i = c + self.i`, `c` a positive literal.
    /// Preserves `>= init` (increasing).
    Increment,
    /// Any other modification of the counter. Blocks the seed.
    Other,
}

/// The single monotone direction of every in-loop write to a counter, or `None`
/// if the writes are mixed / non-monotone / a call is present.
#[derive(Clone, Copy, PartialEq)]
enum Direction {
    Decreasing,
    Increasing,
}

/// Collect every sound monotone-counter invariant for `machine`. Computed once
/// per machine with the checked-fact pass's shared call-frame resolver;
/// `seed_loop_invariant_facts` then seeds the matching states.
pub(super) fn collect_loop_invariant_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<LoopInvariant> {
    let Some(call_frames) = call_frames else {
        return Vec::new();
    };
    let states = program.machine_states(machine);
    let edges = build_edges(program, machine);
    let Some(entry) = states.first().map(|state| state.symbol) else {
        return Vec::new();
    };

    let mut invariants = Vec::new();

    for head in states {
        // A TRUE back edge `S -> head` is one where `head` DOMINATES `S`: every
        // path from the machine entry to `S` goes through `head`, i.e. `S` is
        // unreachable from the entry once `head` is removed. Dominance (not mere
        // "head reaches S") excludes a NESTED inner head, keeping the inner
        // loop's body from swallowing the outer states.
        let reachable_without_head = reachable_from_excluding(&edges, entry, head.symbol);
        let back_sources: Vec<SymbolHandle> = edges
            .iter()
            .filter(|edge| {
                edge.target == head.symbol && !reachable_without_head.contains(&edge.source)
            })
            .map(|edge| edge.source)
            .collect();
        if back_sources.is_empty() {
            continue;
        }

        // G4: the loop states are the NATURAL LOOP of those back edges.
        let loop_states = natural_loop(&edges, head.symbol, &back_sources);

        // Candidate counters: any field a loop state increments or decrements.
        let mut candidates: Vec<SymbolHandle> = Vec::new();
        for &state_symbol in &loop_states {
            let Some(state) = find_state(states, state_symbol) else {
                continue;
            };
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(assignment) = statement.as_assignment()
                    && let Some(field) =
                        assignment_counter_field(program, machine, state, assignment)
                    && matches!(
                        classify_counter_write(program, machine, state, field, assignment),
                        CounterWrite::Decrement | CounterWrite::Increment
                    )
                    && !candidates.contains(&field)
                {
                    candidates.push(field);
                }
            }
        }

        for counter in candidates {
            let Some(index_name) =
                counter_display_name(program, machine, states, &loop_states, counter)
            else {
                continue;
            };
            // G1: every modification of `counter` inside the loop moves in ONE
            // monotone direction. Calls are admitted only when the shared R5
            // frame resolver proves their may-write set disjoint from it.
            let Some(direction) = loop_modifications_direction(
                program,
                machine,
                call_frames,
                states,
                &loop_states,
                counter,
                &index_name,
            ) else {
                continue;
            };

            // G2: every loop-entry edge must establish `counter = K` constant.
            let Some((min_init, max_init)) = entry_constant_init_range(
                program,
                states,
                &edges,
                &loop_states,
                head.symbol,
                counter,
                machine,
                call_frames,
                &index_name,
            ) else {
                continue;
            };

            // A positive literal step is not a mathematical monotonicity
            // proof under Wrapping. Establish update snapshots independently
            // of the invariant being proposed, then prove every update keeps
            // its direction on normal completion.
            if !update_bounds::updates_preserve_order(
                program,
                machine,
                call_frames,
                states,
                &loop_states,
                head.symbol,
                counter,
                min_init,
                max_init,
            ) {
                continue;
            }

            let kind = match direction {
                // `i < K + 1` (G3): the weakest sound bound uses the MAX init.
                Direction::Decreasing => match max_init.checked_add(1) {
                    Some(exclusive_upper_bound) => InvariantKind::UpperBound(exclusive_upper_bound),
                    None => continue,
                },
                // `i >= 0` holds only if EVERY entry starts the counter `>= 0`
                // (the MIN init `>= 0`); an increment never decreases it.
                Direction::Increasing => {
                    if min_init < 0 {
                        continue;
                    }
                    InvariantKind::NonNegative
                }
            };

            for &state_symbol in loop_states.iter().chain(std::iter::once(&head.symbol)) {
                invariants.push(LoopInvariant {
                    state: state_symbol,
                    index_name: index_name.clone(),
                    kind: kind.clone(),
                });
            }

            // An INCREASING counter's guard sits on the back edge (after the increment), so a
            // write-first loop head -- a JOIN of the entry edge and the back edge -- gets no
            // dominating upper bound (the single-guard walk bails at the join, and the meet
            // drops the loop bound there). But when EVERY back edge into the head reaches it via
            // `counter < B` and every entry starts the counter below B, `i < B` holds at the
            // head's ENTRY: each back edge carries it (the guard ran just before, on the same i),
            // and the entry init is < B. Seed it at the HEAD ONLY. Soundness for a use that
            // follows the in-loop increment (where i may reach B) is preserved by the checker's
            // forget-on-reassign: `self.i = self.i + c` drops this bound before that use.
            if matches!(direction, Direction::Increasing)
                && let Some(bound) = back_edge_counter_upper_bound(
                    program,
                    machine,
                    call_frames,
                    states,
                    &back_sources,
                    head.symbol,
                    counter,
                )
                && max_init < bound
            {
                invariants.push(LoopInvariant {
                    state: head.symbol,
                    index_name: index_name.clone(),
                    kind: InvariantKind::UpperBound(bound),
                });
            }

            // Relational Houdini candidate: every ENTRY and BACK edge into the
            // head establishes `counter < the_same_collection.len`. This is a
            // semantic meet, so equivalent guards authored in different states
            // do not need to share an expression handle. The collection itself
            // must remain stable throughout the natural loop; direct writes and
            // opaque/overlapping calls reject the candidate. As with the
            // constant increasing bound above, seed the relation at the HEAD
            // only and let statement-level invalidation drop it after a counter
            // reassignment.
            if matches!(direction, Direction::Increasing)
                && let Some(collection) = loop_head_index_collection(
                    program,
                    machine,
                    call_frames,
                    states,
                    &edges,
                    &loop_states,
                    head.symbol,
                    counter,
                )
                && loop_preserves_path(
                    program,
                    machine,
                    call_frames,
                    states,
                    &loop_states,
                    &collection,
                )
            {
                invariants.push(LoopInvariant {
                    state: head.symbol,
                    index_name: index_name.clone(),
                    kind: InvariantKind::IndexWithin(collection),
                });
            }

            // Further relational class: every incoming edge establishes an
            // upper relation to one stable machine place, while authored
            // machine-arrival facts carry a finite chain from that place to
            // `self.collection.len`. At least one link must be strict:
            // `i < limit <= len` and `i <= outer <= limit < len` both prove
            // `i < len`, while a fully non-strict chain deliberately does not.
            // Every contract-owned intermediate and the collection must be
            // frame-stable throughout the whole machine (including
            // preheaders); otherwise an arrival fact could describe old values
            // before the first loop edge.
            if matches!(direction, Direction::Increasing)
                && let Some((bound, edge_strict)) = loop_head_upper_place(
                    program,
                    machine,
                    call_frames,
                    states,
                    &edges,
                    &loop_states,
                    head.symbol,
                    counter,
                )
                && machine_preserves_path(program, machine, call_frames, &bound)
            {
                for chain in machine_bound_collection_chains(program, machine, call_frames, &bound)
                {
                    if edge_strict || chain.strict {
                        invariants.push(LoopInvariant {
                            state: head.symbol,
                            index_name: index_name.clone(),
                            kind: InvariantKind::IndexWithin(chain.collection),
                        });
                        break;
                    }
                }
            }
        }
    }

    invariants
}

/// Seed `facts` with every loop-invariant fact collected for `state`.
///
/// The collected labels are canonical; the fact store is keyed by the labels
/// this state's own expressions render, so each one is rendered back into this
/// state's spelling of the storage root first. A state that reaches no
/// attached storage can spell no such place, and is seeded nothing.
pub(super) fn seed_loop_invariant_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    facts: &mut RangeFacts<'_>,
    state: &State,
    invariants: &[LoopInvariant],
) {
    for invariant in invariants
        .iter()
        .filter(|invariant| invariant.state == state.symbol)
    {
        let Some(index_name) = validation::declaration_storage_label(
            program,
            machine,
            program.state_parameters(state),
            &invariant.index_name,
        ) else {
            continue;
        };
        match &invariant.kind {
            InvariantKind::UpperBound(exclusive_upper_bound) => {
                facts.prove_index_upper_bound(index_name, *exclusive_upper_bound);
            }
            InvariantKind::NonNegative => {
                facts.prove_non_negative(index_name);
            }
            InvariantKind::IndexWithin(collection) => {
                let Some(collection) = validation::declaration_storage_label(
                    program,
                    machine,
                    program.state_parameters(state),
                    collection,
                ) else {
                    continue;
                };
                facts.prove_index(collection.clone(), index_name.clone());
                facts.prove_range_bound(collection, index_name);
            }
        }
    }
}

/// The counter field a state assignment targets, when the target is a direct
/// field of the machine's attached data reached through whichever root this
/// state binds (`self.field` or `main.field`). Returns the field symbol.
fn assignment_counter_field(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
) -> Option<SymbolHandle> {
    validation::exact_attached_storage_field(
        program,
        machine,
        program.state_parameters(state),
        assignment.target,
    )
    .map(|field| field.symbol)
}

/// Classify a single write to `counter`: a decrement `self.i = self.i - c`, an
/// increment `self.i = self.i + c` / `self.i = c + self.i` (`c` a positive
/// integer literal), or anything else. A literal `c` is deliberately required.
fn classify_counter_write(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    counter: SymbolHandle,
    assignment: &TableAssignment,
) -> CounterWrite {
    if !validation::has_builtin_bound_expression_meaning(
        program,
        machine,
        Some(state),
        assignment.value,
    ) {
        return CounterWrite::Other;
    }
    let Some(binary) = program.expression_table.binary_node(assignment.value) else {
        return CounterWrite::Other;
    };
    match binary.operator {
        BinaryOperator::Subtract => {
            // Not commutative: only `self.i - c` decreases `i`.
            if expression_is_counter_member(program, machine, state, binary.left, counter)
                && is_positive_integer_literal(program, binary.right)
            {
                CounterWrite::Decrement
            } else {
                CounterWrite::Other
            }
        }
        BinaryOperator::Add => {
            let left_is_counter =
                expression_is_counter_member(program, machine, state, binary.left, counter);
            let right_is_counter =
                expression_is_counter_member(program, machine, state, binary.right, counter);
            if (left_is_counter && is_positive_integer_literal(program, binary.right))
                || (right_is_counter && is_positive_integer_literal(program, binary.left))
            {
                CounterWrite::Increment
            } else {
                CounterWrite::Other
            }
        }
        _ => CounterWrite::Other,
    }
}

/// For an `Equal` whose one side is `Boolean(true)`, the other (the real subject). `None` if
/// neither side is `Boolean(true)` (a `== false` negation, or a non-boolean equality).
fn boolean_equality_inner(
    program: &dyn ::validation::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let is_true = |handle| {
        matches!(
            program.expression_table.expression(handle),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(left) {
        Some(right)
    } else if is_true(right) {
        Some(left)
    } else {
        None
    }
}

fn integer_literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<i64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64(),
        _ => None,
    }
}

fn transition_target_symbol(
    program: &dyn ::validation::CheckingInput,
    target: TransitionTargetHandle,
) -> Option<SymbolHandle> {
    if !target.is_valid() {
        return None;
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, .. } => Some(path.symbol),
        _ => None,
    }
}

fn is_positive_integer_literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Integer(value) if value.value_i64().is_some_and(|value| value > 0)
    )
}

/// Whether `expression` is a direct attached-field member naming `counter`,
/// reached through whichever root this state binds.
fn expression_is_counter_member(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    counter: SymbolHandle,
) -> bool {
    validation::exact_attached_storage_field(
        program,
        machine,
        program.state_parameters(state),
        expression,
    )
    .is_some_and(|field| field.symbol == counter)
}

/// G1: every in-loop modification of `counter` moves in ONE monotone direction.
/// Calls and value-position calls are allowed only when the shared R5 frame
/// resolver proves their complete may-write sets disjoint from the counter.
/// Opaque or overlapping calls fail closed. Returns the common direction, or
/// `None`.
fn loop_modifications_direction(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    states: &[State],
    loop_states: &[SymbolHandle],
    counter: SymbolHandle,
    counter_path: &str,
) -> Option<Direction> {
    let mut direction: Option<Direction> = None;
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            if statement_may_write_path(
                program,
                machine,
                state,
                call_frames,
                statement,
                counter_path,
            )? {
                return None;
            }
            if let Some(assignment) = statement.as_assignment() {
                if assignment_counter_field(program, machine, state, assignment) != Some(counter) {
                    // A different syntactic receiver can alias the attached
                    // storage. Only a complete disjoint store frame permits
                    // ignoring it.
                    let writes = call_frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths()?;
                    if writes.iter().any(|path| {
                        validation::frame_paths_overlap(
                            &validation::canonical_storage_label(
                                program,
                                machine,
                                program.state_parameters(state),
                                path,
                            ),
                            counter_path,
                        )
                    }) {
                        return None;
                    }
                    continue;
                }
                let observed =
                    match classify_counter_write(program, machine, state, counter, assignment) {
                        CounterWrite::Decrement => Direction::Decreasing,
                        CounterWrite::Increment => Direction::Increasing,
                        CounterWrite::Other => return None,
                    };
                match direction {
                    Some(existing) if existing != observed => return None,
                    _ => direction = Some(observed),
                }
            }
        }
    }
    direction
}
