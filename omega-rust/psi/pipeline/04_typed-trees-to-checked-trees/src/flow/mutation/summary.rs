//! Structured caller-visible mutation summaries.
//!
//! `validation::CallFrameResolver` remains the single owner of the
//! complete-or-opaque call and cycle law. This module retains symbol-based
//! field/range places for flow invalidation and propagates those places across
//! the calls which the shared resolver admitted as complete.
use crate::flow::CanonicalPlace;
use crate::flow::canonical_place_from_expression;
use crate::flow::canonical_place_from_expression_in_state;
use crate::flow::mutation::WritePlaceNamespace;
use crate::semantic::calls::call_site_argument_expressions;
use crate::semantic::calls::find_call_site;
use crate::semantic::calls::find_state;

use super::local_origins::rebase_local_write_places;
use crate::flow::mutation::receiver::canonical_receiver_place_for_call_site;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowCallFact, BorrowFacts, StateBorrowFact};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default)]
pub(crate) struct StateMutationSummaryCache {
    // Completed summaries depend only on the immutable program and borrow
    // facts. Publish after the fixed point, never during recursive inference.
    states: std::sync::OnceLock<Vec<StateMutationSummary>>,
}

#[cfg(test)]
thread_local! {
    static SUMMARY_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SUMMARY_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
impl StateMutationSummaryCache {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateMutationSummary {
    state_symbol: SymbolHandle,
    complete: bool,
    writes: Vec<CanonicalPlace>,
}

pub(super) fn instantiate_known_call_mutation_summary_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    cache: &StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let summary_places =
        state_mutation_summary_places(program, borrow, cache, target_state, call_frames)?;

    let mut instantiated = Vec::new();
    for summary_place in summary_places {
        let places = instantiate_call_relative_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            borrow_call,
            summary_place,
            namespace,
            call_frames,
        )?;
        for place in places {
            if !instantiated.contains(&place) {
                instantiated.push(place);
            }
        }
    }

    Some(instantiated)
}

fn state_mutation_summary_places<'cache>(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    cache: &'cache StateMutationSummaryCache,
    state: &typed_trees::state::State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<&'cache [CanonicalPlace]> {
    cache
        .states
        .get_or_init(|| build_state_mutation_summaries(program, borrow, call_frames))
        .iter()
        .find(|entry| entry.state_symbol == state.symbol)
        .filter(|entry| entry.complete)
        .map(|entry| entry.writes.as_slice())
}

fn build_state_mutation_summaries(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<StateMutationSummary> {
    #[cfg(test)]
    SUMMARY_BUILDS.set(SUMMARY_BUILDS.get() + 1);
    let mut states = Vec::new();
    let mut inferred_completeness = Vec::new();
    let mut owned_frames = None;
    if let Some(resolver) =
        crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
    {
        for machine in program.machines() {
            for (state, frame) in program
                .machine_states(machine)
                .iter()
                .zip(resolver.inferred_machine_state_write_frames(machine))
            {
                inferred_completeness.push((state.symbol, frame.is_complete()));
            }
        }
    }

    for borrow_state in borrow.states.iter().map(|(_, state)| state) {
        let Some(state) = find_state(program, borrow_state.state_symbol) else {
            continue;
        };
        let direct_writes = collect_state_mutation_summary_places(program, state, call_frames);
        let direct_complete = direct_writes.is_some();
        let writes = direct_writes.unwrap_or_default();
        states.push(StateMutationSummary {
            state_symbol: state.symbol,
            complete: direct_complete
                && state_has_concrete_body_signature(program, state)
                && inferred_completeness
                    .iter()
                    .find_map(|(symbol, complete)| (*symbol == state.symbol).then_some(*complete))
                    .unwrap_or(false),
            writes,
        });
    }

    propagate_state_mutation_summaries(program, borrow, &mut states, call_frames);
    states
}

fn propagate_state_mutation_summaries(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    states: &mut [StateMutationSummary],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
    // These are invocation-local dense summary positions, not durable symbol
    // identities. Resolve each dependency once while retaining authored call order.
    let calls: Vec<_> = states
        .iter()
        .map(|state| {
            borrow_state_for_symbol(borrow, state.state_symbol)
                .map(|borrow_state| borrow.calls.span_or_empty(borrow_state.calls))
                .unwrap_or_default()
                .iter()
                .map(|call| (call, summary_index_from(states, call.target_symbol)))
                .collect::<Vec<_>>()
        })
        .collect();
    let owners: Vec<_> = states
        .iter()
        .map(|summary| {
            find_state(program, summary.state_symbol)
                .map(|state| (state, machine_symbol_for_state(program, state)))
        })
        .collect();
    let mut callers = vec![Vec::new(); states.len()];
    for (caller_index, dependencies) in calls.iter().enumerate() {
        for (_, target_index) in dependencies {
            if let Some(target_index) = target_index
                && !callers[*target_index].contains(&caller_index)
            {
                callers[*target_index].push(caller_index);
            }
        }
    }
    let mut dirty: Vec<_> = (0..states.len()).collect();
    let mut next_dirty = Vec::new();
    let mut scheduled = vec![false; states.len()];
    let mut updates = Vec::new();
    while !dirty.is_empty() {
        // Publish after the round: later callers must not observe this round's
        // additions early, which would change first-discovery write ordering.
        for caller_index in dirty.drain(..) {
            if !states[caller_index].complete {
                continue;
            }
            #[cfg(test)]
            SUMMARY_VISITS.set(SUMMARY_VISITS.get() + 1);
            let caller_symbol = states[caller_index].state_symbol;
            let Some((caller_state, caller_machine)) = owners[caller_index] else {
                continue;
            };
            let mut additions = Vec::new();
            let mut complete = true;
            for (call, target_index) in &calls[caller_index] {
                let Some(target_index) = target_index else {
                    complete = false;
                    break;
                };
                let target = &states[*target_index];
                if !target.complete {
                    complete = false;
                    break;
                }
                for write in &target.writes {
                    let Some(instantiated_places) = instantiate_call_relative_places(
                        program,
                        caller_machine,
                        caller_symbol,
                        borrow,
                        call,
                        write,
                        WritePlaceNamespace::Storage,
                        call_frames,
                    ) else {
                        complete = false;
                        break;
                    };
                    for instantiated in instantiated_places {
                        if state_summary_exposes_place(program, caller_state, &instantiated)
                            && !states[caller_index].writes.contains(&instantiated)
                            && !additions.contains(&instantiated)
                        {
                            additions.push(instantiated);
                        }
                    }
                }
                if !complete {
                    break;
                }
            }
            if !complete || !additions.is_empty() {
                updates.push((caller_index, complete, additions));
            }
        }
        for (caller_index, complete, additions) in updates.drain(..) {
            states[caller_index].complete = complete;
            if complete {
                states[caller_index].writes.extend(additions);
            } else {
                states[caller_index].writes.clear();
            }
            for &dependent in &callers[caller_index] {
                if !scheduled[dependent] {
                    scheduled[dependent] = true;
                    next_dirty.push(dependent);
                }
            }
        }
        next_dirty.sort_unstable();
        for &caller_index in &next_dirty {
            scheduled[caller_index] = false;
        }
        std::mem::swap(&mut dirty, &mut next_dirty);
    }
}

fn borrow_state_for_symbol(
    borrow: &BorrowFacts,
    state_symbol: SymbolHandle,
) -> Option<&StateBorrowFact> {
    borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == state_symbol)
}

fn summary_index_from(
    summaries: &[StateMutationSummary],
    state_symbol: SymbolHandle,
) -> Option<usize> {
    summaries
        .iter()
        .position(|summary| summary.state_symbol == state_symbol)
}

fn machine_symbol_for_state(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> SymbolHandle {
    crate::semantic::calls::find_state_with_machine(program, state.symbol)
        .map(|(machine, _)| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn state_has_concrete_body_signature(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    crate::semantic::calls::find_state_with_machine(program, state.symbol).is_some_and(
        |(machine, _)| {
            machine.body_is_present
                && program
                    .state_parameters(state)
                    .iter()
                    .all(|parameter| parameter.type_reference.is_valid())
        },
    )
}

// Owned primitive formals contain no references: writes change callee storage,
// not the caller's delivered argument. Keep reference-bearing roots visible.
fn state_summary_exposes_place(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    place: &CanonicalPlace,
) -> bool {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    root == machine_symbol_for_state(program, state)
        || program.state_parameters(state).iter().any(|parameter| {
            parameter.symbol == root
                && (parameter.is_self
                    || program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none())
        })
}

fn collect_state_mutation_summary_places(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let machine_symbol = machine_symbol_for_state(program, state);
    let mut writes = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if let StatementNode::RootBinding(binding) = statement {
            let places = crate::flow::reference_expression_storage_places(
                program,
                state.symbol,
                statement_index,
                binding.receiver,
                call_frames,
            )?;
            for place in places {
                if state_summary_exposes_place(program, state, &place) && !writes.contains(&place) {
                    writes.push(place);
                }
            }
            continue;
        }
        let StatementNode::Assignment(_) = statement else {
            continue;
        };
        let places = super::local_origins::assignment_storage_places(
            program,
            machine_symbol,
            state.symbol,
            statement_index,
            statement,
            call_frames,
        )?;
        for place in places {
            if state_summary_exposes_place(program, state, &place) && !writes.contains(&place) {
                writes.push(place);
            }
        }
    }

    Some(writes)
}

fn instantiate_call_relative_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let facts::PlaceRoot::Symbol(parameter_symbol) = relative_place.root else {
        return None;
    };
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    if matches!(namespace, WritePlaceNamespace::Storage)
        && !super::operand_coordinates::call_operands_have_builtin_coordinates(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            &call_site,
        )
    {
        return None;
    }
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let target_machine_symbol = machine_symbol_for_state(program, target_state);
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let base_place = if parameter.is_self {
            if parameter.symbol != parameter_symbol && target_machine_symbol != parameter_symbol {
                continue;
            }
            canonical_receiver_place_for_call_site(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &call_site,
                borrow_call.statement_index,
            )
        } else {
            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);
            if parameter.symbol != parameter_symbol {
                continue;
            }
            if let Some(expression) = argument
                && matches!(namespace, WritePlaceNamespace::AccessRoute)
                && matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
                )
                && let Some(places) = crate::flow::literal_argument_access_places(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    expression,
                    parameter.type_reference,
                    &relative_place.segments,
                )
            {
                return Some(places);
            }
            argument.and_then(|expression| {
                canonical_place_from_expression_in_state(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    expression,
                )
                .or_else(|| canonical_place_from_expression(program, expression))
            })
        }?;

        let mut instantiated = base_place;
        instantiated
            .segments
            .extend(relative_place.segments.iter().copied());
        return match namespace {
            WritePlaceNamespace::AccessRoute => Some(vec![instantiated]),
            WritePlaceNamespace::Storage if !storage_place_has_declared_identity(&instantiated) => {
                // Expression identity describes evaluation, not storage. The
                // shared complete frame owns origins of literal/computed
                // actuals, including their reference-bearing leaves.
                super::shared_call_storage_places(
                    program,
                    caller_machine_symbol,
                    caller_state_symbol,
                    borrow,
                    borrow_call,
                    call_frames,
                )
            }
            WritePlaceNamespace::Storage => rebase_local_write_places(
                program,
                caller_state_symbol,
                borrow_call.statement_index,
                instantiated,
                call_frames,
            ),
        };
    }

    None
}

fn storage_place_has_declared_identity(place: &CanonicalPlace) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && place.segments.iter().all(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            facts::PlaceSegment::Case { variant } => variant.is_valid(),
            _ => true,
        })
}
