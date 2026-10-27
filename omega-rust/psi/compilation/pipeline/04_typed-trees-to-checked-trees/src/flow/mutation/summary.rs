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

impl StateMutationSummaryCache {
    pub(crate) fn prewarm(
        &self,
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) {
        let _ = self.states.get_or_init(|| {
            phase_profile::scope("05.d.3.b.flow.build_state_mutation_summaries", || {
                build_state_mutation_summaries(program, borrow, call_frames)
            })
        });
    }
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
            crate::collections::push_unique(&mut instantiated, contribution);
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
            crate::collections::push_unique(&mut instantiated, place);
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
    let built = cache.states.get_or_init(|| {
        phase_profile::scope("05.d.3.b.flow.build_state_mutation_summaries", || {
            build_state_mutation_summaries(program, borrow, call_frames)
        })
    });
    let &index = built.index_by_state.get(&state.symbol)?;
    built.summaries.get(index).filter(|entry| entry.complete)
}

fn build_state_mutation_summaries(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> BuiltStateMutationSummaries {
    validation::checking_thread_pool().install(|| {
        use rayon::prelude::*;
        let mut owned_frames = None;
        let call_frames =
            crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);

        let (mut states, borrow_states_for_summaries): (Vec<_>, Vec<_>) =
            phase_profile::scope("05.d.3.b.flow.summaries.1_direct", || {
                let borrow_state_refs: Vec<_> =
                    borrow.states.iter().map(|(_, state)| state).collect();
                borrow_state_refs
                    .into_par_iter()
                    .filter_map(|borrow_state| {
                        let state = program.state_by_symbol(borrow_state.state_symbol)?;
                        let direct_writes =
                            collect_state_mutation_summary_places(program, state, call_frames);
                        let direct_complete = direct_writes.is_some();
                        let writes = direct_writes.unwrap_or_default();
                        let complete = direct_complete
                            && state_has_concrete_body_signature(program, state)
                            && call_frames.is_some_and(|resolver| {
                                validation::find_state(program, state.symbol).is_some_and(
                                    |(machine, _)| {
                                        resolver
                                            .inferred_state_write_frame(machine, state)
                                            .is_complete()
                                    },
                                )
                            });
                        Some((
                            StateMutationSummary {
                                state_symbol: state.symbol,
                                held_writes: held_accesses(program, state, &writes),
                                complete,
                                writes,
                            },
                            borrow_state,
                        ))
                    })
                    .unzip()
            });

        // The first summary for a symbol answers lookups, as the former linear
        // search did.
        let mut index_by_state: symbols::SymbolKeyMap<SymbolHandle, usize> =
            symbols::SymbolKeyMap::default();
        for (index, summary) in states.iter().enumerate() {
            index_by_state.entry(summary.state_symbol).or_insert(index);
        }

        phase_profile::scope("05.d.3.b.flow.summaries.2_propagate", || {
            propagate_state_mutation_summaries(
                program,
                borrow,
                &borrow_states_for_summaries,
                &index_by_state,
                &mut states,
                call_frames,
            );
        });
        BuiltStateMutationSummaries {
            summaries: states,
            index_by_state,
        }
    })
}

fn propagate_state_mutation_summaries(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    borrow_states: &[&StateBorrowFact],
    index_by_state: &symbols::SymbolKeyMap<SymbolHandle, usize>,
    states: &mut [StateMutationSummary],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
    use rayon::prelude::*;
    let owners: Vec<_> = states
        .iter()
        .map(|summary| {
            program
                .state_by_symbol(summary.state_symbol)
                .map(|state| (state, machine_symbol_for_state(program, state)))
        })
        .collect();
    // Pre- evaluate whether each caller state's parameters are exposed so
    // `state_summary_exposes_place` is an O(1) slice check rather than
    // repeating `machine_symbol_for_state` and `value_is_reference_free`
    // on every write in every round.
    let exposed_parameters: Vec<Vec<SymbolHandle>> = owners
        .par_iter()
        .map(|owner| {
            let Some((caller_state, _)) = owner else {
                return Vec::new();
            };
            program
                .state_parameters(caller_state)
                .iter()
                .filter(|parameter| {
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
                        && call_frames.is_none_or(|frames| {
                            !frames.value_is_reference_free(parameter.type_reference)
                        })
                })
                .map(|parameter| parameter.symbol)
                .collect()
        })
        .collect();
    // These are invocation-local dense summary positions, not durable symbol
    // identities. Resolve each dependency once while retaining authored call order.
    let mut calls: Vec<_> = phase_profile::scope("05.d.3.b.flow.summaries.2a_calls", || {
        (0..states.len())
            .into_par_iter()
            .map(|caller_index| {
                let state = &states[caller_index];
                let borrow_state = borrow_states[caller_index];
                let owner = &owners[caller_index];
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
                        let call_site = owner.and_then(|(_, machine_symbol)| {
                            find_call_site(
                                program,
                                machine_symbol,
                                state.state_symbol,
                                call.statement_index,
                                call.call_ordinal,
                            )
                        });
                        let has_builtin_coordinates = match (*owner, call_site.as_ref()) {
                            (Some((_, machine_symbol)), Some(site)) => {
                                super::operand_coordinates::call_operands_have_builtin_coordinates(
                                    program,
                                    machine_symbol,
                                    state.state_symbol,
                                    site,
                                )
                            }
                            _ => false,
                        };
                        (
                            call,
                            body_summary,
                            run,
                            call_site,
                            has_builtin_coordinates,
                            Vec::<(
                                (StorageRegion, WriteDestination),
                                Option<Vec<MutationWrite>>,
                            )>::new(),
                            0usize,
                            Vec::<usize>::new(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    });
    let mut callers = vec![Vec::new(); states.len()];
    for (caller_index, dependencies) in calls.iter().enumerate() {
        for (_, target_index, _, _, _, _, _, _) in dependencies {
            if let Some(target_index) = target_index
                && !callers[*target_index].contains(&caller_index)
            {
                callers[*target_index].push(caller_index);
            }
        }
    }
    let _loop_guard = phase_profile::guard("05.d.3.b.flow.summaries.2b_loop");
    let mut dirty_mask = vec![true; states.len()];
    let mut has_dirty = !states.is_empty();
    while has_dirty {
        phase_profile::count("05.d.3.b.flow.summaries.2b_rounds", 1);
        // Publish after the round: later callers must not observe this round's
        // additions early, which would change first-discovery write ordering.
        let updates: Vec<_> = calls
            .par_iter_mut()
            .enumerate()
            .filter_map(|(caller_index, caller_calls)| {
                if !dirty_mask[caller_index] || !states[caller_index].complete {
                    return None;
                }
                let caller_symbol = states[caller_index].state_symbol;
                let (caller_state, caller_machine) = owners[caller_index]?;
                let caller_exposed_params = &exposed_parameters[caller_index];
                let caller = &states[caller_index];
                let mut additions = Vec::new();
                let mut held_additions = vec![Vec::new(); caller.held_writes.len()];
                let mut expose =
                    |writes: Vec<MutationWrite>, held_writes: Vec<Vec<MutationWrite>>| {
                        let retain =
                            |existing: &[MutationWrite],
                             pending: &mut Vec<MutationWrite>,
                             writes: Vec<MutationWrite>| {
                                for write in writes {
                                    let exposed = {
                                        let declared =
                                            matches!(write.region, StorageRegion::Declared(_));
                                        match write.region.anchor().root.as_symbol() {
                                            None => {
                                                if !declared {
                                                    return None;
                                                }
                                                false
                                            }
                                            Some(root) if root == caller_machine => true,
                                            Some(root) => {
                                                if program
                                                    .state_parameters(caller_state)
                                                    .iter()
                                                    .any(|parameter| parameter.symbol == root)
                                                {
                                                    !declared
                                                        || caller_exposed_params.contains(&root)
                                                } else {
                                                    if !declared {
                                                        return None;
                                                    }
                                                    false
                                                }
                                            }
                                        }
                                    };
                                    if exposed
                                        && !existing.contains(&write)
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
                for (
                    call,
                    target_index,
                    run,
                    call_site,
                    has_builtin_coordinates,
                    inst_cache,
                    seen_writes,
                    seen_held,
                ) in caller_calls.iter_mut()
                {
                    // Propagate the target's writes, not its unused authority.
                    // Signature ceilings remain the fallback for opaque calls.
                    let Some(target_index) = *target_index else {
                        if *seen_writes > 0 {
                            continue;
                        }
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
                        *seen_writes = 1;
                        continue;
                    };
                    let target = &states[target_index];
                    if !target.complete {
                        complete = false;
                        break;
                    }
                    let Some(call_site) = call_site.as_ref() else {
                        complete = false;
                        break;
                    };
                    let Some((target_state, _)) = owners[target_index] else {
                        complete = false;
                        break;
                    };
                    let prev_writes = *seen_writes;
                    let new_writes = &target.writes[prev_writes.min(target.writes.len())..];
                    let is_whole_run = matches!(run, TargetRun::Whole);
                    if !is_whole_run && seen_held.len() < target.held_writes.len() {
                        seen_held.resize(target.held_writes.len(), 0);
                    }
                    if new_writes.is_empty()
                        && (is_whole_run
                            || seen_held
                                .iter()
                                .zip(&target.held_writes)
                                .all(|(&s, h)| s >= h.accesses.len()))
                    {
                        continue;
                    }
                    let has_builtin_coords = *has_builtin_coordinates;
                    let call_ref = *call;
                    let mut instantiate = |writes: &[MutationWrite],
                                           use_cache: bool|
                     -> Option<Vec<MutationWrite>> {
                        if !writes.is_empty() && !has_builtin_coords {
                            return None;
                        }
                        let mut instantiated = Vec::new();
                        for write in writes {
                            if !use_cache {
                                let contributions = instantiate_call_relative_writes_pre_resolved(
                                    program,
                                    caller_machine,
                                    caller_symbol,
                                    call_ref,
                                    call_site,
                                    has_builtin_coords,
                                    target_state,
                                    &write.region,
                                    write.destination,
                                    WritePlaceNamespace::Storage,
                                    call_frames,
                                )?;
                                for contribution in contributions {
                                    instantiated.push(MutationWrite {
                                        region: contribution.region,
                                        requires_exclusive_access: write.requires_exclusive_access,
                                        destination: write.destination,
                                    });
                                }
                                continue;
                            }
                            let cached_index = inst_cache.iter().position(|((r, d), _)| {
                                r == &write.region && *d == write.destination
                            });
                            let contributions = match cached_index {
                                Some(idx) => inst_cache[idx].1.as_ref()?,
                                None => {
                                    let computed = instantiate_call_relative_writes_pre_resolved(
                                        program,
                                        caller_machine,
                                        caller_symbol,
                                        call_ref,
                                        call_site,
                                        has_builtin_coords,
                                        target_state,
                                        &write.region,
                                        write.destination,
                                        WritePlaceNamespace::Storage,
                                        call_frames,
                                    )
                                    .map(|contributions| {
                                        contributions
                                            .into_iter()
                                            .map(|contribution| MutationWrite {
                                                region: contribution.region,
                                                requires_exclusive_access: write
                                                    .requires_exclusive_access,
                                                destination: write.destination,
                                            })
                                            .collect::<Vec<_>>()
                                    });
                                    inst_cache.push((
                                        (write.region.clone(), write.destination),
                                        computed,
                                    ));
                                    inst_cache.last().unwrap().1.as_ref()?
                                }
                            };
                            for item in contributions {
                                instantiated.push(MutationWrite {
                                    region: item.region.clone(),
                                    requires_exclusive_access: write.requires_exclusive_access,
                                    destination: write.destination,
                                });
                            }
                        }
                        Some(instantiated)
                    };
                    let Some(whole) = instantiate(new_writes, !is_whole_run) else {
                        complete = false;
                        break;
                    };
                    let held = if is_whole_run {
                        vec![whole.clone(); caller.held_writes.len()]
                    } else {
                        let Some(held) = caller
                            .held_writes
                            .iter()
                            .map(|incoming| {
                                let loan = CanonicalPlace {
                                    root: facts::PlaceRoot::Symbol(incoming.parameter),
                                    segments: Vec::new(),
                                };
                                let selected: Vec<_> = run
                                    .new_accesses(
                                        program,
                                        &target.writes,
                                        prev_writes,
                                        &target.held_writes,
                                        seen_held,
                                        Some(&loan),
                                    )
                                    .cloned()
                                    .collect();
                                instantiate(&selected, true)
                            })
                            .collect::<Option<Vec<_>>>()
                        else {
                            complete = false;
                            break;
                        };
                        held
                    };
                    if expose(whole, held).is_none() {
                        complete = false;
                        break;
                    }
                    *seen_writes = target.writes.len();
                    if !is_whole_run {
                        for (s, h) in seen_held.iter_mut().zip(&target.held_writes) {
                            *s = h.accesses.len();
                        }
                    }
                }
                if !complete
                    || !additions.is_empty()
                    || held_additions.iter().any(|pending| !pending.is_empty())
                {
                    Some((caller_index, complete, additions, held_additions))
                } else {
                    None
                }
            })
            .collect();
        dirty_mask.fill(false);
        has_dirty = false;
        for (caller_index, complete, additions, held_additions) in updates {
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
                if !dirty_mask[dependent] {
                    dirty_mask[dependent] = true;
                    has_dirty = true;
                }
            }
        }
    }
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
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let has_builtin_coordinates = !matches!(namespace, WritePlaceNamespace::Storage)
        || super::operand_coordinates::call_operands_have_builtin_coordinates(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            &call_site,
        );
    let target_state = program.state_by_symbol(borrow_call.target_symbol)?;
    instantiate_call_relative_writes_pre_resolved(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call,
        &call_site,
        has_builtin_coordinates,
        target_state,
        relative_region,
        destination,
        namespace,
        call_frames,
    )
}

fn instantiate_call_relative_writes_pre_resolved(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    call_site: &validation::CallSite<'_>,
    has_builtin_coordinates: bool,
    target_state: &typed_trees::state::State,
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
    if matches!(namespace, WritePlaceNamespace::Storage) && !has_builtin_coordinates {
        return None;
    }
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let base_place = {
            let argument = call_site_argument_expressions(program, call_site)
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
                    call_site,
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
    let trace = std::env::var_os("OMEGA_TRACE_STORAGE_CEILING").is_some();
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
                    crate::collections::push_unique(&mut regions, region);
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
    };
    if trace {
        eprintln!(
            "storage-capture statement={} parameter={} origins={:?}",
            statement_index,
            position,
            origins.as_ref().map(|origins| &origins.arguments)
        );
    }
    let origins = origins?;
    let mut places = Vec::new();
    for source in origins.arguments.get(position)?.as_ref()? {
        let projected = super::local_origins::write_source_region(
            program,
            state,
            statement_index,
            source,
            relative,
        );
        if trace {
            eprintln!(
                "storage-capture projected={:?}",
                projected.as_ref().map(|region| region.anchor())
            );
        }
        let projected = projected?;
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
        );
        if trace {
            eprintln!(
                "storage-capture rebased={:?}",
                rebased.as_ref().map(Vec::len)
            );
        }
        let rebased = rebased?;
        for place in rebased {
            crate::collections::push_unique(&mut places, place);
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
