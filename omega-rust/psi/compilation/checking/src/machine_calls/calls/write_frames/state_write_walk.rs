//! Walking state write prefixes to summarize the paths a state writes.
//!
//! One transfer serves whole-state effects and incoming statement contexts.
//! Prefix-only demand stops at its last boundary instead of solving effects
//! after it; those effects belong to the complete-summary query. Keeping that
//! distinction explicit avoids traversing successor graphs for local alias
//! queries without publishing truncated effects as complete evidence.

#[cfg(test)]
mod tests;

#[cfg(test)]
use super::PREFIX_WALKS;
use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::alias_bindings::{
    rebind_stable_local_mutable_alias_origin, stable_local_mutable_alias_rebinding_is_representable,
};
use crate::machine_calls::calls::write_frames::alias_origins::{
    stable_alias_initializer_origin, stable_alias_initializer_origins,
    stable_assignment_target_path, stable_local_reference_alias_origin,
    stable_local_reference_alias_origins,
};
use crate::machine_calls::calls::write_frames::assignment_targets::expression_is_effectful_indexed_place;
use crate::machine_calls::calls::write_frames::boundary_calls::{
    known_boundary_call_written_paths_for_parts, known_requirement_call_written_paths_for_parts,
};
use crate::machine_calls::calls::write_frames::caller_aliases::AssignmentWriteTarget;
use crate::machine_calls::calls::write_frames::caller_aliases::CallOriginContext;
use crate::machine_calls::calls::write_frames::caller_aliases::CallerWriteSite;
use crate::machine_calls::calls::write_frames::cycle_frames::{
    summarize_state_written_paths_with_cycles, summarize_transition_target_written_paths,
};
use crate::machine_calls::calls::write_frames::demand::{
    collect_expression_call_written_paths, statement_value_expression_roots,
    syntactic_call_written_paths,
};
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::isolation::type_is_caller_isolated_local;
use crate::machine_calls::calls::write_frames::known_call_written_paths_for_parts_with_origins;
use crate::machine_calls::calls::write_frames::local_aliases::expression_reborrows_unresolved_reference_binding;
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, coarse_place_path, split_place_root,
};
use crate::machine_calls::calls::write_frames::state_paths::{
    push_visible_frame_path, relative_state_path_is_visible,
};
use crate::machine_calls::calls::write_frames::stored_origins::{
    StoredLocalOrigins, expand_write_path, expand_write_paths,
};
use crate::machine_calls::calls::write_frames::transition_topology::{
    named_state_transition_subgraph_is_acyclic, named_transition_subgraph_is_acyclic,
};
use crate::machine_calls::calls::write_frames::type_capabilities::{
    type_may_carry_write, type_reference_is_reference,
};
use crate::machine_calls::calls::write_frames::{
    alias_bindings, local_aliases, reference_subjects, stored_origins, wire_codecs,
};
use std::sync::Mutex;
use symbols::SymbolHandle;
use symbols::SymbolKeyMap as HashMap;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

pub(crate) fn summarize_state_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    summarize_complete_state_written_paths(program, machine, state, symbols, inference).or_else(
        || {
            if named_state_transition_subgraph_is_acyclic(program, machine, state) {
                return None;
            }
            // A named backedge can close this walk in the active state's exact
            // namespace. That contextual prefix is not a reusable callee summary:
            // a call can re-enter a machine whose named state is already active,
            // and truncating there would omit the rest of that invocation's writes.
            walk_state_write_prefix(program, machine, state, symbols, inference, None)
                .map(|prefix| prefix.written)
        },
    )
}

/// Only context-independent summaries may cross an ordinary call boundary or
/// enter the shared memo. In particular, a cyclic callee must solve its whole
/// transition graph; a stack-truncated prefix cannot become complete merely
/// because its immediate caller has an acyclic transition graph.
pub(crate) fn summarize_complete_state_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    if let Some(paths) = symbols.caller_sites.completed_state_writes(state.symbol) {
        return Some(paths);
    }
    if !named_state_transition_subgraph_is_acyclic(program, machine, state) {
        // The solver already records every state it solves — including `state`
        // — into the shared completed-summary memo.
        return summarize_state_written_paths_with_cycles(
            program, machine, state, symbols, inference,
        );
    }
    let prefix = walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        inference,
        Some(StateWriteQuery::Complete),
    )?;
    symbols
        .caller_sites
        .record_completed_state_writes(state.symbol, &prefix.written);
    Some(prefix.written)
}

pub(crate) struct StateWritePrefix {
    written: Vec<String>,
    pub(crate) stored: Vec<StoredLocalOrigins>,
    /// The single-statement `Assignment` query's target. Production reads
    /// every assignment from `walk_state_assignments`; the query remains as
    /// the tests' independent route to compare it against.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) assignment: Option<AssignmentWriteTarget>,
}

/// What an `Assignment` query returns for one statement: the direct target
/// and the prefix origins it closes over.
#[derive(Clone)]
pub(crate) struct AssignmentPrefix {
    pub(crate) target: AssignmentWriteTarget,
    pub(crate) aliases: Vec<(String, FramePlaceOrigin)>,
    pub(crate) divergent: Vec<(String, Vec<FramePlaceOrigin>)>,
    pub(crate) stored: Vec<StoredLocalOrigins>,
}

/// Every assignment statement's `Assignment` query result, by statement index,
/// from one walk of the state. An `Assignment` query stops at its statement,
/// so the walk up to that point is the plain walk; this walk records the
/// result where that query would return and carries on as the plain walk. A
/// statement without an entry has no target, exactly as its query returns
/// `None`, including every statement after a point where the walk fails.
pub(crate) fn walk_state_assignments(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
) -> Vec<(usize, AssignmentPrefix)> {
    let mut assignments = Vec::new();
    let mut inference = FrameInference::default();
    let _ = inference.with_local_scope(|inference| {
        walk_state_write_prefix_inner(
            program,
            machine,
            state,
            symbols,
            inference,
            None,
            None,
            Some(&mut assignments),
        )
    });
    assignments
}

pub(crate) enum StateWriteQuery<'statement> {
    /// A reusable whole-state frame, never an ancestor-truncated prefix.
    Complete,
    ReferenceResult,
    /// One state's slot transfer. Named successors belong to the finite
    /// relation solver, so this walk validates only this state's operations.
    StoredReferenceTransfer,
    /// Only incoming statement boundaries, not the enclosing state's effects.
    StatementPrefixes {
        include_shared: bool,
    },
    /// Stop at one assignment and return its target; see
    /// `StateWritePrefix::assignment`.
    #[cfg_attr(not(test), allow(dead_code))]
    Assignment(&'statement StatementNode),
}

/// A per-statement snapshot of the non-shared prefix walk's alias/stored
/// accumulators. `collect_state_write_prefixes` captures one entry per
/// statement index: entry i holds exactly what `Before(statement[i])` would
/// have returned, so a state pays one O(statements) walk instead of a fresh
/// O(index) prefix walk per demand site.
#[derive(Debug, Clone)]
pub(crate) struct CollectedStatementPrefix {
    pub(crate) aliases: Vec<(String, FramePlaceOrigin)>,
    pub(crate) divergent: Vec<(String, Vec<FramePlaceOrigin>)>,
    pub(crate) stored: Vec<StoredLocalOrigins>,
}

struct StatementPrefixCollection<'request> {
    requested: &'request dyn Fn(usize) -> bool,
    prefixes: Vec<Option<CollectedStatementPrefix>>,
}

/// One walk returning both the whole-body prefix and requested statements'
/// `Before`/`ReferenceBefore` context: `include_shared` selects the shared
/// parameter-seeded `stored` exactly as `ReferenceResult` does, and entry i
/// of the collected vector is what `Before(statement[i])` (or
/// `ReferenceBefore`) returns for that boundary. A state's result arms share
/// this walk instead of re-walking the prefix per arm. Unrequested boundaries
/// retain their index as `None` without cloning accumulated origin evidence.
/// Selection changes only snapshot materialization, never body validation.
pub(crate) fn walk_state_write_prefix_collected(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    requested: &dyn Fn(usize) -> bool,
) -> Option<(StateWritePrefix, Vec<Option<CollectedStatementPrefix>>)> {
    inference.with_local_scope(|inference| {
        let mut collection = StatementPrefixCollection {
            requested,
            prefixes: Vec::new(),
        };
        let prefix = walk_state_write_prefix_inner(
            program,
            machine,
            state,
            symbols,
            inference,
            if include_shared {
                Some(StateWriteQuery::ReferenceResult)
            } else {
                None
            },
            Some(&mut collection),
            None,
        )?;
        Some((prefix, collection.prefixes))
    })
}

pub(super) fn stored_reference_transfer(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<(StateWritePrefix, Vec<Option<CollectedStatementPrefix>>)> {
    inference.with_local_scope(|inference| {
        let mut collection = StatementPrefixCollection {
            requested: &|_| true,
            prefixes: Vec::new(),
        };
        let prefix = walk_state_write_prefix_inner(
            program,
            machine,
            state,
            symbols,
            inference,
            Some(StateWriteQuery::StoredReferenceTransfer),
            Some(&mut collection),
            None,
        )?;
        Some((prefix, collection.prefixes))
    })
}

/// One prefix walk snapshotting the accumulator before every statement.
/// `include_shared` selects the shared parameter-seeded accumulators exactly
/// as `ReferenceResult`/`ReferenceBefore` do, so entry i is what
/// `Before(statement[i])` (or `ReferenceBefore`) returns for that boundary.
/// Statements at or after an uncomputable point record `None`, matching the
/// boundary walk's `None` for those indices exactly.
pub(crate) fn collect_state_write_prefixes(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    include_shared: bool,
) -> Option<Vec<Option<CollectedStatementPrefix>>> {
    let statement_count = program
        .statement_table
        .statements(state.statement_nodes)
        .len();
    let mut collection = StatementPrefixCollection {
        requested: &|_| true,
        prefixes: Vec::new(),
    };
    let mut inference = FrameInference::default();
    // The walk's own result is unused here: `prefixes` carries everything the
    // caller reads, and solved callee summaries land in the shared memo.
    let _ = inference.with_local_scope(|inference| {
        walk_state_write_prefix_inner(
            program,
            machine,
            state,
            symbols,
            inference,
            Some(StateWriteQuery::StatementPrefixes { include_shared }),
            Some(&mut collection),
            None,
        )
    });
    while collection.prefixes.len() < statement_count {
        collection.prefixes.push(None);
    }
    Some(collection.prefixes)
}

/// A state's collected per-statement prefixes are a pure property of
/// (machine, state, include_shared): the first demand site builds the one
/// O(statements) walk and every later boundary index reads its collected
/// entry instead of paying a fresh O(index) prefix walk.
pub(crate) fn collected_prefix_at(
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    index: usize,
    include_shared: bool,
) -> Option<CollectedStatementPrefix> {
    let key = (machine.symbol, state.symbol);
    let known = collections
        .lock()
        .ok()
        .is_some_and(|cache| cache.contains_key(&key));
    if !known {
        let built = collect_state_write_prefixes(program, machine, state, symbols, include_shared);
        if let Ok(mut cache) = collections.lock() {
            cache.insert(key, built);
        }
    }
    collections.lock().ok().and_then(|cache| {
        cache
            .get(&key)
            .and_then(|entry| entry.as_ref())
            .and_then(|prefixes| prefixes.get(index))
            .cloned()
            .flatten()
    })
}

/// The same state transfer computes whole-body summaries and the alias context
/// immediately before a demand query. Prefix results never enter the complete
/// state-summary cache.
pub(crate) fn walk_state_write_prefix(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    query: Option<StateWriteQuery<'_>>,
) -> Option<StateWritePrefix> {
    inference.with_local_scope(|inference| {
        walk_state_write_prefix_inner(
            program, machine, state, symbols, inference, query, None, None,
        )
    })
}

fn walk_state_write_prefix_inner(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    query: Option<StateWriteQuery<'_>>,
    mut collect: Option<&mut StatementPrefixCollection<'_>>,
    mut assignments: Option<&mut Vec<(usize, AssignmentPrefix)>>,
) -> Option<StateWritePrefix> {
    if crate::declarations::standard_declarations::is_core_vector_surface_state(
        program, machine, state,
    ) {
        return None;
    }
    #[cfg(test)]
    PREFIX_WALKS.with(|walks| walks.set(walks.get() + 1));
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    // Exclusive-reference locals bound to a divergent conditional result keep
    // their whole proven referent set here rather than a single origin. A
    // write through such a binding lands on one of its candidates, so the
    // frame unions every route, including through nested call substitution.
    let mut divergent_alias_origins = Vec::<(String, Vec<FramePlaceOrigin>)>::new();
    let include_shared = matches!(
        query,
        Some(
            StateWriteQuery::ReferenceResult
                | StateWriteQuery::StoredReferenceTransfer
                | StateWriteQuery::StatementPrefixes {
                    include_shared: true
                }
        )
    );
    let mut stored = if include_shared {
        parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .filter_map(|parameter| {
                let reference =
                    reference_subjects::carrier_storage_type(program, parameter.type_reference)?;
                let origins = stored_origins::declared_origins_for_query(
                    program,
                    parameter.symbol,
                    parameter.name.as_str(),
                    reference,
                    true,
                )?;
                (!type_reference_is_reference(program, parameter.type_reference)
                    || !origins.references.is_empty())
                .then_some(origins)
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut written = Vec::new();

    let mut nested_diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut nested_diagnostics);
    if !nested_diagnostics.is_empty() {
        return None;
    }

    let statements = program.statement_table.statements(state.statement_nodes);
    for (statement_index, statement) in statements.iter().enumerate() {
        if let Some(collection) = collect.as_deref_mut() {
            collection
                .prefixes
                .push(
                    (collection.requested)(statement_index).then(|| CollectedStatementPrefix {
                        aliases: local_alias_origins.clone(),
                        divergent: divergent_alias_origins.clone(),
                        stored: stored.clone(),
                    }),
                );
        }
        if matches!(query, Some(StateWriteQuery::StatementPrefixes { .. }))
            && statement_index + 1 == statements.len()
        {
            // The last incoming boundary is already captured. Its statement's
            // effects cannot change any requested prefix; following its calls
            // or transitions would solve a different, potentially large query.
            // Return no whole-state result: this prefix must never be cached as
            // a complete summary. Full-body and assignment queries still walk it.
            return None;
        }
        let mut queried_assignment = matches!(query,
            Some(StateWriteQuery::Assignment(candidate)) if std::ptr::eq(candidate, statement))
            || (assignments.is_some() && matches!(statement, StatementNode::Assignment(_)));
        if queried_assignment
            && local_alias_origins.is_empty()
            && stored.is_empty()
            && divergent_alias_origins.is_empty()
            && let StatementNode::Assignment(assignment) = statement
            && let Some(path) = coarse_place_path(program, assignment.target)
        {
            // The direct-store query has always admitted an untracked coarse
            // target without evaluating operand effects. Keep that boundary
            // when locating the target and its origins in one prefix walk.
            let exposes_frozen_binding = stored_origins::statement_exposes_frozen_binding(
                program,
                machine,
                state,
                statement,
                &stored,
                &local_alias_origins,
                symbols,
                inference,
            );
            let Some(record) = assignments.as_deref_mut() else {
                if exposes_frozen_binding {
                    return None;
                }
                return Some(StateWritePrefix {
                    written,
                    stored,
                    assignment: Some(AssignmentWriteTarget::Storage { paths: vec![path] }),
                });
            };
            if !exposes_frozen_binding {
                // The accumulators are empty on this route.
                record.push((
                    statement_index,
                    AssignmentPrefix {
                        target: AssignmentWriteTarget::Storage { paths: vec![path] },
                        aliases: Vec::new(),
                        divergent: Vec::new(),
                        stored: Vec::new(),
                    },
                ));
            }
            queried_assignment = false;
        }
        let declared_local_alias_origin = match statement {
            StatementNode::LocalData(local)
                if (type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference))
                    || (include_shared
                        && type_reference_is_reference(program, local.type_reference)) =>
            {
                stable_local_reference_alias_origin(
                    program,
                    machine,
                    &machine_symbols,
                    inference,
                    local,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    &divergent_alias_origins,
                    symbols,
                    &stored,
                    include_shared,
                )
            }
            _ => None,
        };
        let representable_alias_rebinding = match statement {
            StatementNode::Assignment(assignment) => coarse_place_path(program, assignment.target)
                .is_some_and(|target| {
                    stable_local_mutable_alias_rebinding_is_representable(
                        program,
                        machine,
                        state,
                        &target,
                        assignment.value,
                        &local_alias_origins,
                        |aliases| {
                            if include_shared {
                                return reference_subjects::backing_origin(
                                    program,
                                    machine,
                                    assignment.value,
                                    symbols,
                                    inference,
                                    aliases,
                                    &stored,
                                );
                            }
                            stable_alias_initializer_origin(
                                program,
                                machine,
                                &machine_symbols,
                                inference,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                &divergent_alias_origins,
                                symbols,
                                true,
                                &stored,
                            )
                        },
                    )
                }),
            _ => false,
        };
        let declared_stored_origins = match statement {
            StatementNode::LocalData(local)
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)
                    && declared_local_alias_origin.is_none() =>
            {
                stored_origins::declaration_origins_for_query(
                    program,
                    machine,
                    local,
                    &local_alias_origins,
                    &stored,
                    symbols,
                    inference,
                    include_shared,
                )
            }
            _ => None,
        };
        // A synthesized wire codec call's exclusive borrows are the codec
        // leaf's own written-paths model: rebinding through them is a
        // recorded write, not an untracked replacement, so the
        // frozen-binding opacity fence does not apply to it.
        let wire_codec_call = matches!(statement, StatementNode::Call(call)
            if wire_codecs::is_wire_codec_call(program, call));
        let stored_call_effects = if let StatementNode::Call(call) = statement {
            super::stored_calls::call_effects(
                program,
                machine,
                call,
                &CollectedStatementPrefix {
                    aliases: local_alias_origins.clone(),
                    divergent: divergent_alias_origins.clone(),
                    stored: stored.clone(),
                },
                symbols,
                inference,
            )
        } else {
            None
        };
        if !wire_codec_call && let StatementNode::Call(call) = statement {
            stored_origins::retire_private_call_argument_cases(
                program,
                state,
                call,
                &local_alias_origins,
                &mut stored,
                inference,
            );
        }
        if !wire_codec_call
            && stored_call_effects.as_ref().is_none_or(Vec::is_empty)
            && stored_origins::statement_exposes_frozen_binding(
                program,
                machine,
                state,
                statement,
                &stored,
                &local_alias_origins,
                symbols,
                inference,
            )
        {
            return None;
        }
        if !divergent_alias_origins.is_empty() {
            let divergent_roots = divergent_alias_origins
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>();
            if local_aliases::statement_mentions_place_roots(program, statement, &divergent_roots) {
                // Assignment transfer distinguishes replacing the local
                // reference binding from writing through its referent set.
                if let StatementNode::Assignment(assignment) = statement
                    && let Some(relative) = coarse_place_path(program, assignment.target)
                    && let Some(position) = divergent_alias_origins
                        .iter()
                        .position(|(name, _)| name.as_str() == split_place_root(&relative).0)
                {
                    if local_aliases::expression_mentions_place_roots(
                        program,
                        assignment.value,
                        &divergent_roots,
                    ) {
                        return None;
                    }
                    let (_, suffix) = split_place_root(&relative);
                    // Only a bare-name target can rebind the binding itself;
                    // a member or indexed target writes inside the referent
                    // (`coarse_place_path` already coarsened `alias[i]` to
                    // `alias`, so it lands here as a write, not a rebind).
                    let bare_name_target = matches!(
                        program.expression_table.expression(assignment.target),
                        typed_trees::expression::ExpressionNode::Name(_)
                    );
                    if bare_name_target
                        && suffix.is_empty()
                        && local_aliases::expression_may_rebind_mutable_alias(
                            program,
                            machine,
                            state,
                            assignment.value,
                        )
                    {
                        let origins = stable_alias_initializer_origins(
                            program,
                            machine,
                            &machine_symbols,
                            inference,
                            assignment.value,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            &divergent_alias_origins,
                            symbols,
                            true,
                            &stored,
                        )?;
                        if queried_assignment {
                            let target =
                                AssignmentWriteTarget::LocalBindingReplacement { path: relative };
                            let Some(record) = assignments.as_deref_mut() else {
                                return Some(StateWritePrefix {
                                    written,
                                    stored,
                                    assignment: Some(target),
                                });
                            };
                            record.push((
                                statement_index,
                                AssignmentPrefix {
                                    target,
                                    aliases: local_alias_origins.clone(),
                                    divergent: divergent_alias_origins.clone(),
                                    stored: stored.clone(),
                                },
                            ));
                        }
                        divergent_alias_origins[position].1 = origins;
                    } else {
                        // A value RHS into a reference-typed interior slot
                        // writes through that slot's own referent, which the
                        // path frame cannot name.
                        if !bare_name_target
                            && !local_aliases::expression_may_rebind_mutable_alias(
                                program,
                                machine,
                                state,
                                assignment.value,
                            )
                            && crate::value_custody::places::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                            .is_some_and(|reference| {
                                type_reference_is_reference(program, reference)
                            })
                        {
                            return None;
                        }
                        // A bare write through the binding lands on one proven
                        // referent, so the frame records the whole union. Each
                        // candidate still expands through the ordinary write
                        // path so stored carrier slots contribute their
                        // overlapping external leaves.
                        let mut paths = Vec::new();
                        for origin in &divergent_alias_origins[position].1 {
                            let composed = match origin.precision {
                                FramePathPrecision::Exact => {
                                    append_place_suffix(&origin.path, suffix)
                                }
                                FramePathPrecision::CollectionCoarse => origin.path.clone(),
                            };
                            for path in expand_write_path(&composed, &local_alias_origins, &stored)?
                            {
                                if !paths.contains(&path) {
                                    paths.push(path);
                                }
                            }
                        }
                        if queried_assignment {
                            let Some(record) = assignments.as_deref_mut() else {
                                return Some(StateWritePrefix {
                                    written,
                                    stored,
                                    assignment: Some(AssignmentWriteTarget::Storage { paths }),
                                });
                            };
                            record.push((
                                statement_index,
                                AssignmentPrefix {
                                    target: AssignmentWriteTarget::Storage {
                                        paths: paths.clone(),
                                    },
                                    aliases: local_alias_origins.clone(),
                                    divergent: divergent_alias_origins.clone(),
                                    stored: stored.clone(),
                                },
                            ));
                        }
                        for path in paths {
                            push_visible_frame_path(&mut written, path, parameters, &locals)?;
                        }
                    }
                    continue;
                }
            }
        }
        let origins = CallOriginContext {
            parameters,
            isolated_locals: &isolated_local_roots,
            aliases: &local_alias_origins,
            divergent: &divergent_alias_origins,
            stored: &stored,
        };
        for expression in statement_value_expression_roots(program, statement) {
            local_aliases::validate_divergent_projections(
                program,
                machine,
                expression,
                &divergent_alias_origins,
            )?;
            let exposes_reference_binding = if include_shared {
                local_aliases::expression_reborrows_stable_alias_binding(
                    program,
                    expression,
                    parameters,
                    &local_alias_origins,
                )
            } else {
                expression_reborrows_unresolved_reference_binding(
                    program,
                    machine,
                    expression,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    &divergent_alias_origins,
                )
            };
            if exposes_reference_binding
                && declared_local_alias_origin.is_none()
                && !representable_alias_rebinding
                && !(include_shared
                    && declared_stored_origins.is_some()
                    && !reference_subjects::bindings::calls_expose_bindings(
                        program,
                        expression,
                        parameters,
                        &local_alias_origins,
                    ))
                && !alias_bindings::statement_returns_reference_without_effects(
                    program, state, statement,
                )
            {
                return None;
            }
            let mut expression_writes = Vec::new();
            collect_expression_call_written_paths(
                program,
                expression,
                machine,
                &machine_symbols,
                symbols,
                inference,
                &mut expression_writes,
                &origins,
            )?;
            for relative in expand_write_paths(&expression_writes, &local_alias_origins, &stored)? {
                if relative_state_path_is_visible(&relative, parameters, &locals)?
                    && !written.contains(&relative)
                {
                    written.push(relative);
                }
            }
        }
        match statement {
            StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                let replaces_local_binding = representable_alias_rebinding
                    || (include_shared
                        && reference_subjects::replaces_binding(program, machine, statement)?);
                let replacement = stored_origins::assignment_origins(
                    program,
                    machine,
                    state,
                    statement,
                    assignment,
                    &local_alias_origins,
                    &stored,
                    symbols,
                    inference,
                    include_shared,
                    replaces_local_binding,
                )?;
                let direct_target = coarse_place_path(program, assignment.target);
                if let Some(relative) = direct_target.as_deref()
                    && rebind_stable_local_mutable_alias_origin(
                        program,
                        machine,
                        state,
                        relative,
                        assignment.value,
                        &mut local_alias_origins,
                        |aliases| {
                            let origin = if include_shared {
                                reference_subjects::backing_origin(
                                    program,
                                    machine,
                                    assignment.value,
                                    symbols,
                                    inference,
                                    aliases,
                                    &stored,
                                )
                            } else {
                                stable_alias_initializer_origin(
                                    program,
                                    machine,
                                    &machine_symbols,
                                    inference,
                                    assignment.value,
                                    parameters,
                                    &isolated_local_roots,
                                    aliases,
                                    &divergent_alias_origins,
                                    symbols,
                                    true,
                                    &stored,
                                )
                            };
                            origin.or_else(|| {
                                if !include_shared {
                                    return None;
                                }
                                let reference =
                                    crate::value_custody::places::declared_place_type_raw(
                                        program,
                                        machine,
                                        Some(state),
                                        assignment.target,
                                    )?;
                                reference_subjects::unknown_readonly_origin(
                                    program, reference, relative,
                                )
                            })
                        },
                    )?
                {
                    if queried_assignment {
                        let target = AssignmentWriteTarget::LocalBindingReplacement {
                            path: relative.to_owned(),
                        };
                        let Some(record) = assignments.as_deref_mut() else {
                            return Some(StateWritePrefix {
                                written,
                                stored,
                                assignment: Some(target),
                            });
                        };
                        record.push((
                            statement_index,
                            AssignmentPrefix {
                                target,
                                aliases: local_alias_origins.clone(),
                                divergent: divergent_alias_origins.clone(),
                                stored: stored.clone(),
                            },
                        ));
                    }
                    continue;
                }
                let relative = stable_assignment_target_path(
                    program,
                    machine,
                    &machine_symbols,
                    inference,
                    assignment.target,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )?;
                let paths = expand_write_path(&relative, &local_alias_origins, &stored)?;
                if queried_assignment {
                    let Some(record) = assignments.as_deref_mut() else {
                        return Some(StateWritePrefix {
                            written,
                            stored,
                            assignment: Some(AssignmentWriteTarget::Storage { paths }),
                        });
                    };
                    record.push((
                        statement_index,
                        AssignmentPrefix {
                            target: AssignmentWriteTarget::Storage {
                                paths: paths.clone(),
                            },
                            aliases: local_alias_origins.clone(),
                            divergent: divergent_alias_origins.clone(),
                            stored: stored.clone(),
                        },
                    ));
                }
                for path in paths {
                    push_visible_frame_path(&mut written, path, parameters, &locals)?;
                }
                // The write itself expanded through the pre-state evidence;
                // only later statements see the replacement's own origins.
                if let Some(replacement) = replacement {
                    inference.record_local(&replacement);
                    if let Some(existing) = stored
                        .iter_mut()
                        .find(|local| local.local_symbol == replacement.local_symbol)
                    {
                        *existing = replacement;
                    } else {
                        stored.push(replacement);
                    }
                }
            }
            StatementNode::Call(nested_call) => {
                let nested_receiver_members = program
                    .statement_table
                    .name_path_members(nested_call.receiver)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect::<Vec<_>>();
                let arguments = program
                    .statement_table
                    .expression_handles(nested_call.arguments);
                let divergent_receiver_origins = nested_receiver_members
                    .first()
                    .and_then(|root| {
                        divergent_alias_origins
                            .iter()
                            .find(|(name, _)| name == root)
                    })
                    .map(|(_, origins)| origins.as_slice());
                // The recorded origins replace one whole local receiver. A
                // projection through that local still needs a compositional
                // projected-origin proof; do not guess a suffix for each arm.
                if divergent_receiver_origins.is_some() && nested_receiver_members.len() != 1 {
                    return None;
                }
                let has_divergent_receiver = divergent_receiver_origins.is_some();
                let receiver_members = if has_divergent_receiver {
                    &[]
                } else {
                    nested_receiver_members.as_slice()
                };
                let receiver_origins = divergent_receiver_origins.unwrap_or_default();
                // A synthesized wire codec has no body to summarize and no
                // parameters to instantiate; its frame is its exclusively
                // borrowed arguments. The type-name receiver must never reach
                // the ownership floor, which would poison it as a place.
                let codec = wire_codecs::is_wire_codec_call(program, nested_call);
                let nested_writes = if codec {
                    if has_divergent_receiver {
                        return None;
                    }
                    // A divergent argument is resolved rather than refused:
                    // the resolver below takes the candidate set and unions
                    // every proven referent, and keeps an unproven binding
                    // opaque on its own.
                    wire_codecs::known_wire_codec_call_written_paths(
                        program,
                        machine,
                        nested_call,
                        parameters,
                        &isolated_local_roots,
                        &local_alias_origins,
                        &divergent_alias_origins,
                        symbols,
                        inference,
                    )
                } else {
                    let argument_types = super::call_targets::call_argument_types(
                        program,
                        machine,
                        nested_call.target_symbol,
                        nested_call.target.as_str(),
                        receiver_members,
                        CallerWriteSite::Call(nested_call),
                        symbols,
                    );
                    let argument_origins = origins.argument_origins(
                        program,
                        machine,
                        &machine_symbols,
                        symbols,
                        inference,
                        arguments,
                        &argument_types,
                    )?;
                    let has_divergent_actual = arguments
                        .iter()
                        .any(|argument| origins.mentions_divergent(program, *argument));
                    known_call_written_paths_for_parts_with_origins(
                        program,
                        nested_call.target_symbol,
                        nested_call.target.as_str(),
                        receiver_members,
                        receiver_origins,
                        None,
                        arguments,
                        machine,
                        &machine_symbols,
                        symbols,
                        inference,
                        Some(&argument_origins),
                    )
                    .or_else(|| {
                        if has_divergent_receiver {
                            return None;
                        }
                        (!arguments.iter().any(|argument| {
                            expression_is_effectful_indexed_place(program, *argument)
                        }))
                        .then(|| {
                            known_boundary_call_written_paths_for_parts(
                                program,
                                machine,
                                &machine_symbols,
                                symbols,
                                receiver_members,
                                nested_call.target.as_str(),
                                CallerWriteSite::Call(nested_call),
                                arguments,
                                inference,
                            )
                        })
                        .flatten()
                    })
                    .or_else(|| {
                        if has_divergent_receiver {
                            return None;
                        }
                        (!arguments.iter().any(|argument| {
                            expression_is_effectful_indexed_place(program, *argument)
                        }))
                        .then(|| {
                            known_requirement_call_written_paths_for_parts(
                                program,
                                machine,
                                &machine_symbols,
                                symbols,
                                receiver_members,
                                nested_call.target.as_str(),
                                None,
                                CallerWriteSite::Call(nested_call),
                                arguments,
                                inference,
                            )
                        })
                        .flatten()
                    })
                    .or_else(|| {
                        if has_divergent_receiver || has_divergent_actual {
                            return None;
                        }
                        syntactic_call_written_paths(
                            program,
                            machine,
                            receiver_members,
                            arguments,
                            &machine_symbols,
                            symbols,
                        )
                    })
                }?;
                let codec_written_roots: Vec<String> = if codec {
                    nested_writes
                        .iter()
                        .map(|path| split_place_root(path))
                        .filter(|(_, suffix)| suffix.is_empty())
                        .map(|(root, _)| root.to_string())
                        .collect()
                } else {
                    Vec::new()
                };
                for relative in expand_write_paths(
                    &origins.close_paths(nested_writes),
                    &local_alias_origins,
                    &stored,
                )? {
                    if relative_state_path_is_visible(&relative, parameters, &locals)?
                        && !written.contains(&relative)
                    {
                        written.push(relative);
                    }
                }
                // The codec writes every exclusive-borrow target wholesale,
                // so rows it does not re-declare are dead evidence: retire a
                // fully written local's stored origins rather than let later
                // statements read stale referents from them.
                stored.retain(|local| {
                    !codec_written_roots
                        .iter()
                        .any(|root| program.symbols.name(local.local_symbol) == root)
                });
                if let Some(effects) = &stored_call_effects {
                    super::stored_calls::apply(program, &mut stored, effects);
                    for record in &stored {
                        inference.record_local(record);
                    }
                }
            }
            StatementNode::Transition(transition) => {
                if matches!(query, Some(StateWriteQuery::StoredReferenceTransfer)) {
                    continue;
                }
                for target in [transition.target, transition.continuation] {
                    if (!local_alias_origins.is_empty()
                        || !stored.is_empty()
                        || !divergent_alias_origins.is_empty())
                        && target.is_valid()
                        && matches!(
                            program.statement_table.transition_target(target),
                            TransitionTargetNode::Named { .. }
                        )
                        && !named_transition_subgraph_is_acyclic(program, machine, state, target)
                    {
                        // Alias origins compose positionally through acyclic
                        // named graphs. A reachable named SCC needs equation-
                        // level alias substitution, so keep it opaque here.
                        return None;
                    }
                    let transition_writes = summarize_transition_target_written_paths(
                        program,
                        machine,
                        state,
                        target,
                        symbols,
                        inference,
                        &locals,
                        matches!(query, Some(StateWriteQuery::Complete)),
                        &origins,
                        &machine_symbols,
                    )?;
                    for relative in
                        expand_write_paths(&transition_writes, &local_alias_origins, &stored)?
                    {
                        if relative_state_path_is_visible(&relative, parameters, &locals)?
                            && !written.contains(&relative)
                        {
                            written.push(relative);
                        }
                    }
                }
            }
            StatementNode::Expression(_) => {}
            StatementNode::LocalData(local) => {
                if (type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference))
                    || (include_shared
                        && type_reference_is_reference(program, local.type_reference))
                {
                    // An unknown read-only origin is local to this query. Add
                    // it only after the exposure/effect checks: retaining a
                    // binding is not admission to expose another alias slot.
                    let origin = declared_local_alias_origin.or_else(|| {
                        if !include_shared {
                            return None;
                        }
                        reference_subjects::unknown_readonly_origin(
                            program,
                            local.type_reference,
                            local.name.as_str(),
                        )
                    });
                    if let Some(origin) = origin {
                        local_alias_origins.push((local.name.as_str().to_owned(), origin));
                    } else if let Some(origins) = declared_stored_origins {
                        inference.record_local(&origins);
                        stored.push(origins);
                    } else if let Some(origins) = (!include_shared)
                        .then(|| {
                            stable_local_reference_alias_origins(
                                program,
                                machine,
                                &machine_symbols,
                                inference,
                                local,
                                parameters,
                                &isolated_local_roots,
                                &local_alias_origins,
                                &divergent_alias_origins,
                                symbols,
                                &stored,
                            )
                        })
                        .flatten()
                    {
                        // A divergent exclusive-reference binding keeps its
                        // whole proven referent set for later assignment and
                        // call substitution, including copies of this binding.
                        divergent_alias_origins.push((local.name.as_str().to_owned(), origins));
                    } else {
                        let origins = stored_origins::unresolved_origins(
                            program,
                            machine,
                            local,
                            include_shared,
                        )?;
                        inference.record_local(&origins);
                        stored.push(origins);
                    }
                } else if stored_origins::has_aggregate_case_shape(program, local.type_reference)
                    && let Some(origins) = stored_origins::declaration_origins(
                        program,
                        machine,
                        local,
                        &local_alias_origins,
                        &stored,
                        symbols,
                        inference,
                    )
                {
                    // Failure to recover a no-write value's cases does not
                    // invalidate its writes. A later payload projection still
                    // requires positive case evidence from this same transfer.
                    inference.record_local(&origins);
                    stored.push(origins);
                }
                if type_is_caller_isolated_local(program, local.type_reference)
                    && !local_alias_origins
                        .iter()
                        .any(|(name, _)| name == local.name.as_str())
                    && !divergent_alias_origins
                        .iter()
                        .any(|(name, _)| name == local.name.as_str())
                {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    (query.is_none()
        || matches!(
            query,
            Some(
                StateWriteQuery::Complete
                    | StateWriteQuery::ReferenceResult
                    | StateWriteQuery::StoredReferenceTransfer
            )
        ))
    .then_some(StateWritePrefix {
        written,
        stored,
        assignment: None,
    })
}
