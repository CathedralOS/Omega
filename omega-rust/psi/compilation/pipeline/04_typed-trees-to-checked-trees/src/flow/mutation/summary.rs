//! Structured caller-visible mutation summaries.
//!
//! `validation::CallFrameResolver` remains the single owner of the
//! complete-or-opaque call and cycle law. This module retains symbol-based
//! field/range places for flow invalidation and propagates those places across
//! the calls which the shared resolver admitted as complete.
//! A boundary signature has no local state summary, but its complete frame is
//! still a dependency: project its receiver and argument writes into caller
//! storage before propagating them. Absence of a body is not opacity; absence
//! of a complete frame or representable storage origin still is.
//! Storage effects and exclusive access are separate: sealed atomic operations
//! invalidate values but use shared access. Retain that operation provenance
//! across calls instead of turning every caller-visible write into a mutable
//! loan requirement. Ordinary replacement of an atomic cell remains exclusive.

use validation::CanonicalPlace;
use validation::call_site_argument_expressions;
use validation::canonical_place_from_expression;
use validation::canonical_place_from_expression_in_state;
use validation::find_call_site;

use crate::flow::jump_loans::{HeldAccesses, TargetRun, held_accesses, target_run};
use crate::flow::mutation::{CallMutationWrite, WritePlaceNamespace};

use super::local_origins::rebase_local_write_places;
use checked_trees::expression::ExpressionNode;
use checked_trees::{BorrowCallFact, BorrowFacts, StateBorrowFact};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default)]
pub(crate) struct StateMutationSummaryCache {
    // Completed summaries depend only on the immutable program and borrow
    // facts. Publish after the fixed point, never during recursive inference.
    states: std::sync::OnceLock<Vec<StateMutationSummary>>,
}

/// Complete transitive writes in a state's own incoming storage namespace.
/// Cleanup uses the same summary as authored calls, without fabricating a
/// source call coordinate or interpreting the frame's rendered report paths.
pub(crate) fn state_storage_writes(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    cache: &StateMutationSummaryCache,
    state: &typed_trees::state::State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    Some(
        state_mutation_summary(program, borrow, cache, state, call_frames)?
            .writes
            .iter()
            .map(|write| write.place.clone())
            .collect(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateMutationSummary {
    state_symbol: SymbolHandle,
    complete: bool,
    /// The whole run's writes, which a call holds its argument loans across.
    writes: Vec<MutationWrite>,
    /// Writes while each incoming parameter's loans remain held.
    held_writes: Vec<HeldAccesses<MutationWrite>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MutationWrite {
    place: CanonicalPlace,
    requires_exclusive_access: bool,
}

pub(super) fn instantiate_known_call_mutation_summary_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    cache: &StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    loan: Option<&CanonicalPlace>,
) -> Option<Vec<CallMutationWrite>> {
    let target_state = program.state_by_symbol(borrow_call.target_symbol)?;
    let summary = state_mutation_summary(program, borrow, cache, target_state, call_frames)?;
    let run = target_run(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
    );
    let summary_places = run.accesses(program, &summary.writes, &summary.held_writes, loan);

    let mut instantiated = Vec::new();
    for write in summary_places {
        if matches!(namespace, WritePlaceNamespace::AccessRoute) && !write.requires_exclusive_access
        {
            continue;
        }
        let contributions = instantiate_call_relative_writes(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            borrow_call,
            &write.place,
            namespace,
            call_frames,
        )?;
        for contribution in contributions {
            if !instantiated.contains(&contribution) {
                instantiated.push(contribution);
            }
        }
    }

    Some(instantiated)
}

/// The callee's complete summary restricted to writes reached through its
/// data binding, instantiated at the call's actual parent place: the whole
/// run's writes for a call, the held run's for a jump, as far as the call's
/// own argument loans reach. `None` when the callee has no complete summary or
/// a receiver write cannot be placed.
pub(super) fn instantiate_known_call_receiver_summary_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    cache: &StateMutationSummaryCache,
    loan: &CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    let target_state = program.state_by_symbol(borrow_call.target_symbol)?;
    let target_machine_symbol = machine_symbol_for_state(program, target_state);
    let target_machine = program.machine_by_symbol(target_machine_symbol)?;
    let data_binding =
        program.data_binding_parameter(target_machine, program.state_parameters(target_state));
    let summary = state_mutation_summary(program, borrow, cache, target_state, None)?;
    let run = target_run(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
    );
    let summary_places = run.accesses(program, &summary.writes, &summary.held_writes, Some(loan));

    let mut instantiated = Vec::new();
    for write in summary_places {
        if !write.requires_exclusive_access {
            continue;
        }
        let summary_place = &write.place;
        let root = summary_place.root.as_symbol()?;
        let reached_through_receiver = root == target_machine_symbol
            || data_binding.is_some_and(|parameter| parameter.symbol == root);
        if !reached_through_receiver {
            continue;
        }
        for place in instantiate_call_relative_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            borrow_call,
            summary_place,
            WritePlaceNamespace::AccessRoute,
            None,
        )? {
            if !instantiated.contains(&place) {
                instantiated.push(place);
            }
        }
    }

    Some(instantiated)
}

fn state_mutation_summary<'cache>(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    cache: &'cache StateMutationSummaryCache,
    state: &typed_trees::state::State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<&'cache StateMutationSummary> {
    cache
        .states
        .get_or_init(|| build_state_mutation_summaries(program, borrow, call_frames))
        .iter()
        .find(|entry| entry.state_symbol == state.symbol)
        .filter(|entry| entry.complete)
}

fn build_state_mutation_summaries(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<StateMutationSummary> {
    let mut states = Vec::new();
    let mut inferred_completeness = Vec::new();
    let mut owned_frames = None;
    let call_frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
    if let Some(resolver) = call_frames {
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
        let Some(state) = program.state_by_symbol(borrow_state.state_symbol) else {
            continue;
        };
        let direct_writes = collect_state_mutation_summary_places(program, state, call_frames);
        let direct_complete = direct_writes.is_some();
        let writes = direct_writes.unwrap_or_default();
        states.push(StateMutationSummary {
            state_symbol: state.symbol,
            held_writes: held_accesses(program, state, &writes),
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
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    states: &mut [StateMutationSummary],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
    let owners: Vec<_> = states
        .iter()
        .map(|summary| {
            program
                .state_by_symbol(summary.state_symbol)
                .map(|state| (state, machine_symbol_for_state(program, state)))
        })
        .collect();
    // These are invocation-local dense summary positions, not durable symbol
    // identities. Resolve each dependency once while retaining authored call order.
    let calls: Vec<_> = states
        .iter()
        .zip(&owners)
        .map(|(state, owner)| {
            borrow_state_for_symbol(borrow, state.state_symbol)
                .map(|borrow_state| borrow.calls.span_or_empty(borrow_state.calls))
                .unwrap_or_default()
                .iter()
                .map(|call| {
                    let run = owner.map_or(TargetRun::Whole, |(_, machine_symbol)| {
                        target_run(program, machine_symbol, state.state_symbol, borrow, call)
                    });
                    // A declaration can own a state identity without owning a body.
                    // Its admitted frame belongs to the shared resolver, not
                    // the (necessarily incomplete) structured body summary.
                    let body_summary = validation::find_state(program, call.target_symbol)
                        .filter(|(machine, _)| machine.body_is_present)
                        .and_then(|_| summary_index_from(states, call.target_symbol));
                    (call, body_summary, run)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut callers = vec![Vec::new(); states.len()];
    for (caller_index, dependencies) in calls.iter().enumerate() {
        for (_, target_index, _) in dependencies {
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
            let caller_symbol = states[caller_index].state_symbol;
            let Some((caller_state, caller_machine)) = owners[caller_index] else {
                continue;
            };
            let caller = &states[caller_index];
            let mut additions = Vec::new();
            let mut held_additions = vec![Vec::new(); caller.held_writes.len()];
            let mut expose = |writes: Vec<MutationWrite>, held_writes: Vec<Vec<MutationWrite>>| {
                let retain = |existing: &[MutationWrite],
                              pending: &mut Vec<MutationWrite>,
                              writes: Vec<MutationWrite>| {
                    for write in writes {
                        if state_summary_exposes_place(
                            program,
                            caller_state,
                            &write.place,
                            call_frames,
                        ) && !existing.contains(&write)
                            && !pending.contains(&write)
                        {
                            pending.push(write);
                        }
                    }
                };
                retain(&caller.writes, &mut additions, writes);
                for ((held, pending), writes) in caller
                    .held_writes
                    .iter()
                    .zip(&mut held_additions)
                    .zip(held_writes)
                {
                    retain(&held.accesses, pending, writes);
                }
            };
            let mut complete = true;
            for (call, target_index, run) in &calls[caller_index] {
                let Some(target_index) = target_index else {
                    // Signature-only calls are already admitted by the shared
                    // frame law. Import that frame in caller coordinates; an
                    // unresolved frame/origin must not become an empty summary.
                    // The held run includes all of it, like any call's.
                    let Some(writes) = super::shared_call_storage_places(
                        program,
                        caller_machine,
                        caller_symbol,
                        borrow,
                        call,
                        call_frames,
                    ) else {
                        complete = false;
                        break;
                    };
                    let writes: Vec<_> = writes
                        .into_iter()
                        .map(|place| MutationWrite {
                            place,
                            requires_exclusive_access: true,
                        })
                        .collect();
                    let held = vec![writes.clone(); caller.held_writes.len()];
                    expose(writes, held);
                    continue;
                };
                let target = &states[*target_index];
                if !target.complete {
                    complete = false;
                    break;
                }
                let instantiate = |writes: &[MutationWrite]| {
                    instantiate_target_writes(
                        program,
                        caller_machine,
                        caller_symbol,
                        borrow,
                        call,
                        writes,
                        call_frames,
                    )
                };
                let Some(whole) = instantiate(&target.writes) else {
                    complete = false;
                    break;
                };
                let held = caller
                    .held_writes
                    .iter()
                    .map(|incoming| {
                        let loan = CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(incoming.parameter),
                            segments: Vec::new(),
                        };
                        let selected: Vec<_> = run
                            .accesses(program, &target.writes, &target.held_writes, Some(&loan))
                            .cloned()
                            .collect();
                        instantiate(&selected)
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(held) = held else {
                    complete = false;
                    break;
                };
                expose(whole, held);
            }
            if !complete
                || !additions.is_empty()
                || held_additions.iter().any(|pending| !pending.is_empty())
            {
                updates.push((caller_index, complete, additions, held_additions));
            }
        }
        for (caller_index, complete, additions, held_additions) in updates.drain(..) {
            let summary = &mut states[caller_index];
            summary.complete = complete;
            if complete {
                summary.writes.extend(additions);
                for (held, additions) in summary.held_writes.iter_mut().zip(held_additions) {
                    held.accesses.extend(additions);
                }
            } else {
                summary.writes.clear();
                summary.held_writes.clear();
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

/// A target summary's writes in the caller's storage coordinates at `call`,
/// in summary order. `None` when one cannot be placed.
fn instantiate_target_writes(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    call: &BorrowCallFact,
    writes: &[MutationWrite],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<MutationWrite>> {
    let mut instantiated = Vec::new();
    for write in writes {
        for place in instantiate_call_relative_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            call,
            &write.place,
            WritePlaceNamespace::Storage,
            call_frames,
        )? {
            instantiated.push(MutationWrite {
                place,
                requires_exclusive_access: write.requires_exclusive_access,
            });
        }
    }
    Some(instantiated)
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
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> SymbolHandle {
    validation::find_state(program, state.symbol)
        .map(|(machine, _)| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn state_has_concrete_body_signature(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> bool {
    validation::find_state(program, state.symbol).is_some_and(|(machine, _)| {
        machine.body_is_present
            && program
                .state_parameters(state)
                .iter()
                .all(|parameter| parameter.type_reference.is_valid())
    })
}

// Owned primitive formals contain no references: writes change callee storage,
// not the caller's delivered argument. Keep reference-bearing roots visible.
fn state_summary_exposes_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    place: &CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(root) = place.root.as_symbol() else {
        return false;
    };
    root == machine_symbol_for_state(program, state)
        || program.state_parameters(state).iter().any(|parameter| {
            parameter.symbol == root
                && (program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none())
                // Incoming owned storage is a private value, even when a
                // local loan lets a nested call mutate it. Only references
                // carried by the formal can expose those writes to a caller.
                // Reuse the resolver's closed-shape law; opaque or recursive
                // storage stays conservative rather than guessing isolation.
                && call_frames.is_none_or(|frames| {
                    !frames.value_is_reference_free(parameter.type_reference)
                })
        })
}

fn collect_state_mutation_summary_places(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<MutationWrite>> {
    let machine_symbol = machine_symbol_for_state(program, state);
    let machine = program.machine_by_symbol(machine_symbol)?;
    let mut writes = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        if let Some(binding) = statement.as_root_binding() {
            let places = crate::flow::reference_expression_storage_places(
                program,
                state.symbol,
                statement_index,
                binding.receiver,
                call_frames,
            )?;
            for place in places {
                let write = MutationWrite {
                    place,
                    requires_exclusive_access: true,
                };
                if state_summary_exposes_place(program, state, &write.place, call_frames)
                    && !writes.contains(&write)
                {
                    writes.push(write);
                }
            }
            continue;
        }
        let Some(assignment) = statement.as_assignment() else {
            continue;
        };
        let requires_exclusive_access =
            !(validation::atomic_assignment_carrier(program, assignment).is_some()
                && validation::place_is_atomic_storage(
                    program,
                    machine,
                    Some(state),
                    assignment.target,
                ));
        let places = super::local_origins::assignment_storage_places(
            program,
            machine_symbol,
            state.symbol,
            statement_index,
            statement,
            call_frames,
        )?;
        for place in places {
            let write = MutationWrite {
                place,
                requires_exclusive_access,
            };
            if state_summary_exposes_place(program, state, &write.place, call_frames)
                && !writes.contains(&write)
            {
                writes.push(write);
            }
        }
    }

    Some(writes)
}

pub(in crate::flow) fn instantiate_call_relative_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    instantiate_call_relative_writes(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        relative_place,
        namespace,
        call_frames,
    )
    .map(|writes| writes.into_iter().map(|write| write.place).collect())
}

fn instantiate_call_relative_writes(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CallMutationWrite>> {
    let attributed = |places: Vec<CanonicalPlace>| {
        places
            .into_iter()
            .map(|place| CallMutationWrite {
                place,
                source: Some(relative_place.clone()),
            })
            .collect()
    };
    let parameter_symbol = relative_place.root.as_symbol()?;
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
    let target_state = program.state_by_symbol(borrow_call.target_symbol)?;
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let base_place = {
            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);
            if parameter.symbol != parameter_symbol {
                continue;
            }
            // A body summary includes writes to its own incoming copies.
            // Only reference-bearing actuals can project those writes into
            // caller storage; keep their existing precise path/origin join.
            let mut owned_frames = None;
            if crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
                .is_some_and(|resolver| resolver.value_is_reference_free(parameter.type_reference))
            {
                return Some(Vec::new());
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
                return Some(attributed(places));
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
            WritePlaceNamespace::AccessRoute => Some(attributed(vec![instantiated])),
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
                .map(super::unattributed_writes)
            }
            WritePlaceNamespace::Storage => rebase_local_write_places(
                program,
                caller_state_symbol,
                borrow_call.statement_index,
                instantiated,
                call_frames,
            )
            .map(attributed),
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
