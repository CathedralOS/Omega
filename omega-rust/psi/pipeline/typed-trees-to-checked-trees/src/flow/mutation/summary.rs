//! Structured caller-visible mutation summaries.
//!
//! `validation::CallFrameResolver` remains the single owner of the
//! complete-or-opaque call and cycle law. This module retains symbol-based
//! field/range places for flow invalidation and propagates those places across
//! the calls which the shared resolver admitted as complete.

use super::local_origins::rebase_local_write_places;
use super::*;
use crate::flow::mutation::receiver::canonical_receiver_place_for_call_site;

#[derive(Debug, Clone, Default)]
pub(crate) struct StateMutationSummaryCache {
    initialized: bool,
    states: Vec<StateMutationSummary>,
}

#[derive(Debug, Clone)]
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
    cache: &mut StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
) -> Option<Vec<CanonicalPlace>> {
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let summary_places = state_mutation_summary_places(program, borrow, cache, target_state)?;

    let mut instantiated = Vec::new();
    for summary_place in summary_places {
        let places = instantiate_call_relative_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            summary_place,
            namespace,
        )?;
        for place in places {
            if !instantiated.contains(&place) {
                instantiated.push(place);
            }
        }
    }

    Some(instantiated)
}

fn core_method_mutates_receiver(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    let Some(machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    }) else {
        return false;
    };

    let method_name = machine
        .name
        .as_str()
        .rsplit_once("::")
        .map(|(_, method)| method)
        .unwrap_or_else(|| state.name.as_str());
    if !matches!(method_name, "as_mut_slice" | "index_mut" | "pop" | "push") {
        return false;
    }

    let is_vec_attached_machine = machine
        .attached_data
        .as_ref()
        .is_some_and(|attached_data| attached_data.as_str() == "Vec");
    let is_vec_method_machine = machine.name.as_str().starts_with("Vec::");

    is_vec_attached_machine || is_vec_method_machine
}

fn state_mutation_summary_places<'cache>(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    cache: &'cache mut StateMutationSummaryCache,
    state: &typed_trees::state::State,
) -> Option<&'cache [CanonicalPlace]> {
    ensure_state_mutation_summaries(program, borrow, cache);

    cache
        .states
        .iter()
        .find(|entry| entry.state_symbol == state.symbol)
        .filter(|entry| entry.complete)
        .map(|entry| entry.writes.as_slice())
}

fn ensure_state_mutation_summaries(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    cache: &mut StateMutationSummaryCache,
) {
    if cache.initialized {
        return;
    }
    cache.initialized = true;

    let mut inferred_completeness = Vec::new();
    if let Some(resolver) = validation::CallFrameResolver::new(program) {
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
        let direct_writes = collect_state_mutation_summary_places(program, state);
        let direct_complete = direct_writes.is_some();
        let mut writes = direct_writes.unwrap_or_default();
        let core_receiver_write = core_method_mutates_receiver(program, state);
        if core_receiver_write
            && let Some(receiver) = state_receiver_summary_place(program, state)
            && !writes.contains(&receiver)
        {
            writes.push(receiver);
        }
        cache.states.push(StateMutationSummary {
            state_symbol: state.symbol,
            complete: core_receiver_write
                || (direct_complete
                    && state_has_concrete_body_signature(program, state)
                    && inferred_completeness
                        .iter()
                        .find_map(|(symbol, complete)| {
                            (*symbol == state.symbol).then_some(*complete)
                        })
                        .unwrap_or(false)),
            writes,
        });
    }

    loop {
        let snapshot = cache.states.clone();
        let mut changed = false;
        for caller_index in 0..snapshot.len() {
            if !snapshot[caller_index].complete {
                continue;
            }
            let caller_symbol = snapshot[caller_index].state_symbol;
            let Some(caller_state) = find_state(program, caller_symbol) else {
                continue;
            };
            let caller_machine = machine_symbol_for_state(program, caller_state);
            let Some(borrow_state) = borrow_state_for_symbol(borrow, caller_symbol) else {
                continue;
            };
            let mut additions = Vec::new();
            let mut complete = true;
            for call in borrow.calls.span_or_empty(borrow_state.calls) {
                let Some(target_index) = summary_index_from(&snapshot, call.target_symbol) else {
                    complete = false;
                    break;
                };
                let target = &snapshot[target_index];
                if !target.complete {
                    complete = false;
                    break;
                }
                for write in &target.writes {
                    let Some(instantiated_places) = instantiate_call_relative_places(
                        program,
                        caller_machine,
                        caller_symbol,
                        call,
                        write,
                        WritePlaceNamespace::Storage,
                    ) else {
                        complete = false;
                        break;
                    };
                    for instantiated in instantiated_places {
                        if state_summary_exposes_place(program, caller_state, &instantiated)
                            && !snapshot[caller_index].writes.contains(&instantiated)
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
            if !complete {
                cache.states[caller_index].complete = false;
                cache.states[caller_index].writes.clear();
                changed = true;
            } else if !additions.is_empty() {
                cache.states[caller_index].writes.extend(additions);
                changed = true;
            }
        }
        if !changed {
            break;
        }
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
    program
        .machines()
        .iter()
        .find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
        })
        .map(|machine| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn state_has_concrete_body_signature(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    program
        .machines()
        .iter()
        .find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
        })
        .is_some_and(|machine| {
            machine.body_is_present
                && program
                    .state_parameters(state)
                    .iter()
                    .all(|parameter| parameter.type_reference.is_valid())
        })
}

fn state_receiver_summary_place(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> Option<CanonicalPlace> {
    let receiver = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)?;
    canonical_place_from_symbol(receiver.symbol)
        .or_else(|| canonical_place_from_symbol(machine_symbol_for_state(program, state)))
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
) -> Option<Vec<CanonicalPlace>> {
    let machine_symbol = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
                .then_some(machine.symbol)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    let mut writes = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Assignment(_) = statement else {
            continue;
        };
        let places = super::local_origins::assignment_storage_places(
            program,
            machine_symbol,
            state.symbol,
            statement_index,
            statement,
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
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
    namespace: WritePlaceNamespace,
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
    let target_machine_symbol = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == target_state.symbol)
                .then_some(machine.symbol)
        })
        .unwrap_or_else(SymbolHandle::invalid);
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
                    borrow_call,
                )
            }
            WritePlaceNamespace::Storage => rebase_local_write_places(
                program,
                caller_state_symbol,
                borrow_call.statement_index,
                instantiated,
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
