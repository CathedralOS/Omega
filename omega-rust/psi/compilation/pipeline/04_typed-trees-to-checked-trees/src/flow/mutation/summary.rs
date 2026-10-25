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
use crate::flow::mutation::{CallMutationWrite, StorageRegion, WritePlaceNamespace};

use super::local_origins::rebase_storage_region;
use checked_trees::expression::ExpressionNode;
use checked_trees::{BorrowCallFact, BorrowFacts, StateBorrowFact};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default)]
pub(crate) struct StateMutationSummaryCache {
    // Completed summaries depend only on the immutable program and borrow
    // facts. Publish after the fixed point, never during recursive inference.
    states: std::sync::OnceLock<BuiltStateMutationSummaries>,
}

#[derive(Debug, Clone)]
struct BuiltStateMutationSummaries {
    summaries: Vec<StateMutationSummary>,
    index_by_state: symbols::SymbolKeyMap<SymbolHandle, usize>,
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
    StorageRegion::declared_places(
        state_mutation_summary(program, borrow, cache, state, call_frames)?
            .writes
            .iter()
            .map(|write| write.region.clone()),
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

/// A reference-valued receiving cell is distinct from the storage its value
/// denotes. Preserve that destination judgment until caller projection ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WriteDestination {
    Referent,
    DescriptorSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MutationWrite {
    region: StorageRegion,
    requires_exclusive_access: bool,
    destination: WriteDestination,
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
    let summary = state_mutation_summary(program, borrow, cache, target_state, call_frames);
    let summary = summary?;
    let run = target_run(
        program,
        caller_machine_symbol,
        caller_state_symbol,
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
            borrow_call,
            &write.region,
            write.destination,
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
        borrow_call,
    );
    let summary_places = run.accesses(program, &summary.writes, &summary.held_writes, Some(loan));

    let mut instantiated = Vec::new();
    for write in summary_places {
        if !write.requires_exclusive_access {
            continue;
        }
        let summary_place = write.region.anchor();
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
    let built = cache
        .states
        .get_or_init(|| build_state_mutation_summaries(program, borrow, call_frames));
    let &index = built.index_by_state.get(&state.symbol)?;
    built.summaries.get(index).filter(|entry| entry.complete)
}

fn build_state_mutation_summaries(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> BuiltStateMutationSummaries {
    let mut owned_frames = None;
    let call_frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
    let mut states = Vec::with_capacity(borrow.states.len());
    let mut borrow_states_for_summaries = Vec::with_capacity(borrow.states.len());

    for borrow_state in borrow.states.iter().map(|(_, state)| state) {
        let Some(state) = program.state_by_symbol(borrow_state.state_symbol) else {
            continue;
        };
        let direct_writes = collect_state_mutation_summary_places(program, state, call_frames);
        let direct_complete = direct_writes.is_some();
        let writes = direct_writes.unwrap_or_default();
        let complete = direct_complete
            && state_has_concrete_body_signature(program, state)
            && call_frames.is_some_and(|resolver| {
                validation::find_state(program, state.symbol)
                    .is_some_and(|(machine, _)| {
                        resolver
                            .inferred_state_write_frame(machine, state)
                            .is_complete()
                    })
            });
        states.push(StateMutationSummary {
            state_symbol: state.symbol,
            held_writes: held_accesses(program, state, &writes),
            complete,
            writes,
        });
        borrow_states_for_summaries.push(borrow_state);
    }

    // The first summary for a symbol answers lookups, as the former linear
    // search did.
    let mut index_by_state: symbols::SymbolKeyMap<SymbolHandle, usize> =
        symbols::SymbolKeyMap::default();
    for (index, summary) in states.iter().enumerate() {
        index_by_state.entry(summary.state_symbol).or_insert(index);
    }

    propagate_state_mutation_summaries(
        program,
        borrow,
        &borrow_states_for_summaries,
        &index_by_state,
        &mut states,
        call_frames,
    );
    BuiltStateMutationSummaries {
        summaries: states,
        index_by_state,
    }
}

fn propagate_state_mutation_summaries(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    borrow_states: &[&StateBorrowFact],
    index_by_state: &symbols::SymbolKeyMap<SymbolHandle, usize>,
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
        .zip(borrow_states)
        .zip(&owners)
        .map(|((state, borrow_state), owner)| {
            borrow
                .calls
                .span_or_empty(borrow_state.calls)
                .iter()
                .map(|call| {
                    let run = owner.map_or(TargetRun::Whole, |(_, machine_symbol)| {
                        target_run(program, machine_symbol, state.state_symbol, call)
                    });
                    // A declaration can own a state identity without owning a body.
                    // Its admitted frame belongs to the shared resolver, not
                    // the (necessarily incomplete) structured body summary.
                    let body_summary = validation::find_state(program, call.target_symbol)
                        .filter(|(machine, _)| machine.body_is_present)
                        .and_then(|_| index_by_state.get(&call.target_symbol).copied());
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
                            &write.region,
                            call_frames,
                        )? && !existing.contains(&write)
                            && !pending.contains(&write)
                        {
                            pending.push(write);
                        }
                    }
                    Some(())
                };
                retain(&caller.writes, &mut additions, writes)?;
                for ((held, pending), writes) in caller
                    .held_writes
                    .iter()
                    .zip(&mut held_additions)
                    .zip(held_writes)
                {
                    retain(&held.accesses, pending, writes)?;
                }
                Some(())
            };
            let mut complete = true;
            for (call, target_index, run) in &calls[caller_index] {
                // Propagate the target's writes, not its unused authority.
                // Signature ceilings remain the fallback for opaque calls.
                let Some(target_index) = target_index else {
                    // Signature-only calls are already admitted by the shared
                    // frame law. Import that frame in caller coordinates; an
                    // unresolved frame/origin must not become an empty summary.
                    // The held run includes all of it, like any call's.
                    let Some(writes) = super::shared_call_storage_regions(
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
                        .map(|region| MutationWrite {
                            region,
                            requires_exclusive_access: true,
                            destination: WriteDestination::Referent,
                        })
                        .collect();
                    let held = vec![writes.clone(); caller.held_writes.len()];
                    if expose(writes, held).is_none() {
                        complete = false;
                        break;
                    }
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
                if expose(whole, held).is_none() {
                    complete = false;
                    break;
                }
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
    call: &BorrowCallFact,
    writes: &[MutationWrite],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<MutationWrite>> {
    let mut instantiated = Vec::new();
    for write in writes {
        for contribution in instantiate_call_relative_writes(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            call,
            &write.region,
            write.destination,
            WritePlaceNamespace::Storage,
            call_frames,
        )? {
            instantiated.push(MutationWrite {
                region: contribution.region,
                requires_exclusive_access: write.requires_exclusive_access,
                destination: write.destination,
            });
        }
    }
    Some(instantiated)
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

// Declared local storage can be private; content reached through a local owner
// cannot. Without an exact exportable owner coordinate the summary is unknown,
// so callers retain the signature ceiling rather than an empty effect.
fn state_summary_exposes_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    region: &StorageRegion,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    let declared = matches!(region, StorageRegion::Declared(_));
    let Some(root) = region.anchor().root.as_symbol() else {
        return declared.then_some(false);
    };
    if root == machine_symbol_for_state(program, state) {
        return Some(true);
    }
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root)
    else {
        return declared.then_some(false);
    };
    if !declared {
        return Some(true);
    }
    // A write to a reference-free formal changes only its incoming value.
    // This isolation rule does not apply to separately owned content.
    Some(
        program
            .primitive_type_reference(parameter.type_reference)
            .is_none()
            && call_frames
                .is_none_or(|frames| !frames.value_is_reference_free(parameter.type_reference)),
    )
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
                    region: StorageRegion::Declared(place),
                    requires_exclusive_access: true,
                    destination: WriteDestination::Referent,
                };
                if state_summary_exposes_place(program, state, &write.region, call_frames)?
                    && !writes.contains(&write)
                {
                    writes.push(write);
                }
            }
            continue;
        }
        if let Some(local) = statement.as_local_data()
            && validation::atomic_once_carrier(program, local.initial_value).is_some()
        {
            for place in super::atomic_once_storage_writes(
                program,
                machine_symbol,
                state.symbol,
                statement_index,
                local.initial_value,
                call_frames,
            )? {
                let write = MutationWrite {
                    region: StorageRegion::Declared(place),
                    requires_exclusive_access: false,
                    destination: WriteDestination::Referent,
                };
                if state_summary_exposes_place(program, state, &write.region, call_frames)?
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
        let regions = super::local_origins::assignment_storage_regions(
            program,
            machine_symbol,
            state.symbol,
            statement_index,
            statement,
            call_frames,
        )?;
        for region in regions {
            let destination = validation::assignment_stored_value_type(
                program, machine, state, assignment,
            )
            .map(|reference| {
                if validation::reference_shell(program, reference).is_some() {
                    WriteDestination::DescriptorSlot
                } else {
                    WriteDestination::Referent
                }
            })?;
            let write = MutationWrite {
                region,
                requires_exclusive_access,
                destination,
            };
            if state_summary_exposes_place(program, state, &write.region, call_frames)?
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
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    instantiate_call_relative_writes(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call,
        &StorageRegion::Declared(relative_place.clone()),
        WriteDestination::Referent,
        namespace,
        call_frames,
    )
    .and_then(|writes| StorageRegion::declared_places(writes.into_iter().map(|write| write.region)))
}

fn instantiate_call_relative_writes(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    relative_region: &StorageRegion,
    destination: WriteDestination,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CallMutationWrite>> {
    let relative_place = relative_region.anchor();
    let attributed = |regions: Vec<StorageRegion>| {
        regions
            .into_iter()
            .map(|region| CallMutationWrite {
                region,
                source: Some(relative_region.clone()),
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
            // Inline bytes of a by-value argument belong to the callee.
            // Only a genuine reference crossing reaches caller storage.
            if matches!(relative_region, StorageRegion::Declared(_))
                && !write_reaches_caller(
                    program,
                    parameter.type_reference,
                    &relative_place.segments,
                    destination,
                )?
            {
                return Some(Vec::new());
            }
            if matches!(namespace, WritePlaceNamespace::Storage) {
                // Every actual uses its captured storage interpretation.
                // A declaration-looking expression can still be a recast:
                // appending the callee's field symbols to that declaration
                // would confuse logical selectors with physical storage.
                // Unknown origins keep the conservative signature ceiling.
                let regions = captured_argument_write_regions(
                    program,
                    caller_machine_symbol,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    borrow_call.call_ordinal,
                    &call_site,
                    argument_index.checked_sub(1)?,
                    parameter.type_reference,
                    &relative_place.segments,
                    destination,
                    call_frames,
                )?;
                let regions = match relative_region {
                    StorageRegion::Declared(_) => regions,
                    StorageRegion::ResidentContent {
                        formation,
                        granted,
                        resident,
                        ..
                    } => regions
                        .into_iter()
                        .map(|region| {
                            Some(StorageRegion::ResidentContent {
                                owner: region.into_declared()?,
                                formation: *formation,
                                granted: *granted,
                                resident: *resident,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?,
                    StorageRegion::NonresidentContent { formation, .. } => regions
                        .into_iter()
                        .map(|region| {
                            Some(StorageRegion::NonresidentContent {
                                owner: region.into_declared()?,
                                formation: *formation,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?,
                    StorageRegion::UnresolvedContent { .. } => regions
                        .into_iter()
                        .map(|region| StorageRegion::UnresolvedContent {
                            carrier: region.anchor().clone(),
                        })
                        .collect(),
                };
                return Some(attributed(regions));
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
                return Some(attributed(
                    places.into_iter().map(StorageRegion::Declared).collect(),
                ));
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
        return Some(attributed(vec![StorageRegion::Declared(instantiated)]));
    }

    None
}

pub(super) fn captured_argument_write_regions(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    site: &validation::CallSite<'_>,
    position: usize,
    parameter_type: typed_trees::types::TypeReferenceHandle,
    relative: &[facts::PlaceSegment],
    destination: WriteDestination,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<StorageRegion>> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_by_symbol(state_symbol)?;
    let resolver = call_frames?;
    if validation::reference_shell(program, parameter_type).is_none() {
        let actual = *call_site_argument_expressions(program, site).get(position)?;
        let mut regions = Vec::new();
        for projection in crate::flow::literal_expression_projections(
            program,
            actual,
            parameter_type,
            relative,
            false,
        )? {
            let projected_type = validation::project_type_reference_from_segments(
                program,
                parameter_type,
                &projection.destination,
            )?;
            let projected_regions =
                if validation::reference_shell(program, projected_type).is_some() {
                    resolver
                        .reference_operand_write_sources(
                            machine,
                            projection.expression,
                            projected_type,
                        )?
                        .iter()
                        .map(|source| {
                            super::local_origins::write_source_region(
                                program,
                                state,
                                statement_index,
                                source,
                                &projection.remaining,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?
                } else {
                    let mut place = canonical_place_from_expression_in_state(
                        program,
                        state_symbol,
                        statement_index,
                        projection.expression,
                    )?;
                    place.segments.extend_from_slice(&projection.remaining);
                    vec![StorageRegion::Declared(place)]
                };
            for region in projected_regions {
                for region in rebase_projected_call_region(
                    program,
                    state_symbol,
                    statement_index,
                    region,
                    projected_type,
                    &projection.remaining,
                    destination,
                    call_frames,
                )? {
                    if !regions.contains(&region) {
                        regions.push(region);
                    }
                }
            }
        }
        return Some(regions);
    }
    let origins = match site {
        validation::CallSite::Statement(call) => resolver.call_write_origins(machine, call),
        validation::CallSite::Expression { expression, .. } => {
            resolver.expression_call_write_origins(machine, *expression)
        }
        validation::CallSite::TransitionNamed { .. } => resolver.transition_write_origins(
            machine,
            validation::transition_call_target(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
            )?,
        ),
    }?;
    let mut places = Vec::new();
    for source in origins.arguments.get(position)?.as_ref()? {
        let projected = super::local_origins::write_source_region(
            program,
            state,
            statement_index,
            source,
            relative,
        )?;
        if !storage_place_has_declared_identity(projected.anchor()) {
            return None;
        }
        let rebased = rebase_projected_call_region(
            program,
            state_symbol,
            statement_index,
            projected,
            parameter_type,
            relative,
            destination,
            call_frames,
        )?;
        for place in rebased {
            if !places.contains(&place) {
                places.push(place);
            }
        }
    }
    Some(places)
}

/// Rebase the selected source coordinate with its genuine receiving mode.
/// Shape matching is never reference existence, access or lifetime evidence.
fn rebase_projected_call_region(
    program: &dyn validation::CheckingInput,
    state: SymbolHandle,
    statement: usize,
    region: StorageRegion,
    parameter: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
    destination: WriteDestination,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<StorageRegion>> {
    if destination == WriteDestination::Referent {
        return rebase_storage_region(program, state, statement, region, frames);
    }
    let StorageRegion::Declared(place) = region else {
        return None;
    };
    let root_type =
        validation::reference_shell(program, parameter).map_or(parameter, |shell| shell.referee);
    let expected = validation::project_type_reference_from_segments(program, root_type, path)?;
    let actual = validation::canonical_place_type_reference(program, state, statement, &place)?;
    validation::reference_shell(program, expected)?;
    validation::reference_shell(program, actual)?;
    if !validation::storage_origin_types_match(program, expected, actual) {
        return None;
    }
    super::local_origins::rebase_descriptor_slot_regions(program, state, statement, place, frames)
}

fn write_reaches_caller(
    program: &dyn validation::CheckingInput,
    parameter: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
    destination: WriteDestination,
) -> Option<bool> {
    if crate::flow::operand_contexts::operand_binds_live_view(program, parameter) {
        return Some(true);
    }
    if !validation::has_contents(
        program,
        parameter,
        &[],
        validation::ContentsRequirement::StoredValue,
    ) {
        return None;
    }
    for length in 1..=path.len() {
        let reference =
            validation::project_type_reference_from_segments(program, parameter, &path[..length])?;
        if validation::reference_shell(program, reference).is_some()
            && (length < path.len() || destination == WriteDestination::Referent)
        {
            return Some(true);
        }
    }
    Some(false)
}

fn storage_place_has_declared_identity(place: &CanonicalPlace) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && place.segments.iter().all(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            facts::PlaceSegment::Case { variant } => variant.is_valid(),
            _ => true,
        })
}
