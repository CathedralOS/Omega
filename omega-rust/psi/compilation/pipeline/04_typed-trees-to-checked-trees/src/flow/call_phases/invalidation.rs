use validation::CanonicalPlace;

use crate::flow::FlowBuildContext;
use crate::flow::appended_span_since;
use crate::flow::mutation::close_storage_places_over_aliases;
use crate::flow::mutation::{CallMutationWrite, call_mutation_writes};
use crate::flow::project_constraint_refs_to_active_contexts;
use arena::HandleSpan;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowConstraintRef, FlowInvalidationFact,
    FlowInvalidationSource, FlowSemanticContextRef,
};
use facts::FactPlan;

pub(in crate::flow) struct CallInvalidationResult {
    pub(in crate::flow) post_contexts: HandleSpan<FlowSemanticContextRef>,
    pub(in crate::flow) post_constraints: HandleSpan<FlowConstraintRef>,
    pub(in crate::flow) invalidations: HandleSpan<FlowInvalidationFact>,
}

/// One immutable invocation footprint supplies both overlap invalidation and
/// attributed handback. Retaining them together avoids recomputing the same
/// alias closure while preserving which parameter contributed each write.
#[derive(Clone)]
pub(in crate::flow) struct CallStorageWrites {
    /// Conservative dependencies of observations, not writable descriptor
    /// storage. A resident content write includes its owner here because an
    /// observer of that owner can read the content.
    pub(in crate::flow) observation_places: Vec<CanonicalPlace>,
    pub(in crate::flow) contributions: Vec<ObservedCallMutation>,
}

impl CallStorageWrites {
    /// A content region has a dependency anchor, not a descriptor store. Until
    /// physical storage coordinates are available for that region, descriptor
    /// association remains unknown rather than treating the anchor as a write.
    pub(in crate::flow) fn descriptor_stores(&self) -> Option<Vec<CanonicalPlace>> {
        self.contributions
            .iter()
            .map(|contribution| match &contribution.write.region {
                crate::flow::mutation::StorageRegion::Declared(place) => Some(place.clone()),
                crate::flow::mutation::StorageRegion::ResidentContent { .. }
                | crate::flow::mutation::StorageRegion::NonresidentContent { .. }
                | crate::flow::mutation::StorageRegion::UnresolvedContent { .. } => None,
            })
            .collect()
    }
}

#[derive(Clone)]
pub(in crate::flow) struct ObservedCallMutation {
    pub(in crate::flow) write: CallMutationWrite,
    pub(in crate::flow) observations: Vec<CanonicalPlace>,
}

impl CallStorageWrites {
    /// These aliases depend on actual evaluation captures, so they remain
    /// local to this flow build rather than entering the source-only cache.
    pub(in crate::flow) fn with_captured_observations(
        &self,
        arguments: Option<&super::ArgumentSources<'_, '_>>,
    ) -> Self {
        let mut observed = self.clone();
        if let Some(arguments) = arguments {
            for contribution in &mut observed.contributions {
                for alias in arguments.mutation_observation_aliases(&contribution.write) {
                    crate::collections::push_unique(&mut contribution.observations, alias);
                }
            }
            observed.observation_places = observed
                .contributions
                .iter()
                .flat_map(|contribution| contribution.observations.iter().cloned())
                .collect();
        }
        observed
    }
}

pub(in crate::flow) fn call_storage_writes(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> Option<std::rc::Rc<CallStorageWrites>> {
    let occurrence = (
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    if let Some(writes) = build
        .call_sites
        .get(&occurrence)
        .and_then(|prepared| prepared.storage_writes.as_ref())
    {
        return writes.clone();
    }
    // The footprint is source-derived, not a judgment about the current
    // incoming values. Retain it beside the existing occurrence lookup across
    // sweeps; each consumer still filters its own live contexts below.
    let writes = call_storage_mutations(program, borrow, build, machine, state, borrow_call).map(
        |contributions| {
            std::rc::Rc::new(CallStorageWrites {
                observation_places: contributions
                    .iter()
                    .flat_map(|contribution| contribution.observations.iter().cloned())
                    .collect(),
                contributions,
            })
        },
    );
    build
        .call_sites
        .entry(occurrence)
        .or_default()
        .storage_writes = Some(writes.clone());
    writes
}

fn call_storage_mutations(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> Option<Vec<ObservedCallMutation>> {
    let writes = phase_profile::scope("05.d.3.b.flow.invoke.1a_mutation_writes", || {
        call_mutation_writes(
            program,
            machine.symbol,
            state.symbol,
            borrow,
            borrow_call,
            build.state_mutation_summary_cache,
            build.call_frames,
        )
    })?;
    let mut result = Vec::new();
    for mut write in writes {
        // Unresolved content names the carrier the capability is attached to.
        // Facts that overlap that carrier are stale. A sibling field of the
        // same owner is a different place, so this footprint stays partial
        // instead of withdrawing every caller fact.
        if let crate::flow::mutation::StorageRegion::UnresolvedContent { .. } = &write.region {
            // An output namespace identifies a performed reference, not a
            // spatially separate allocation. Its anchor alone cannot close
            // aliases in caller storage: retain full unknown invalidation.
            if matches!(
                write.region.anchor().root,
                facts::PlaceRoot::InvocationOutput(_)
            ) {
                return None;
            }
            let carrier = write.region.anchor().clone();
            result.push(ObservedCallMutation {
                write,
                observations: vec![carrier],
            });
            continue;
        }
        // When the resolver cannot enumerate this state's local origins (a
        // reference local of finite candidate origins is such a state),
        // borrow exclusivity already forbids a second live alias of the
        // written storage, so the places stand without the closure.
        let mut observations =
            phase_profile::scope("05.d.3.b.flow.invoke.1b_close_aliases", || {
                close_storage_places_over_aliases(
                    program,
                    machine.symbol,
                    state.symbol,
                    borrow_call.statement_index,
                    vec![write.region.anchor().clone()],
                    build.call_frames,
                )
                .or_else(|| match &write.region {
                    crate::flow::mutation::StorageRegion::Declared(place) => {
                        Some(vec![place.clone()])
                    }
                    // Do not lose content aliases when the inventory cannot be
                    // reconstructed. Unknown remains full invalidation.
                    crate::flow::mutation::StorageRegion::ResidentContent { .. } => None,
                    crate::flow::mutation::StorageRegion::NonresidentContent { .. } => None,
                    crate::flow::mutation::StorageRegion::UnresolvedContent { .. } => None,
                })
            })?;
        // Keep the current state's observation coordinates even when a
        // transported view proves that the physical store is content, not
        // descriptor replacement. Both namespaces must see invalidation.
        phase_profile::scope("05.d.3.b.flow.invoke.1c_entry_origins", || {
            if let crate::flow::mutation::StorageRegion::Declared(place) = &write.region {
                if let Some(region) = crate::flow::entry_origins::nonresident_content_region(
                    program,
                    build,
                    machine,
                    state,
                    borrow_call.statement_index,
                    place,
                ) {
                    observations.push(region.anchor().clone());
                    write.region = region;
                }
            }
            crate::flow::entry_origins::extend_invocation_observations(
                program,
                build,
                machine,
                state,
                borrow_call.statement_index,
                &mut observations,
            );
        });
        result.push(ObservedCallMutation {
            write,
            observations,
        });
    }
    Some(result)
}

pub(in crate::flow) fn apply_call_invalidations(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
    writes: Option<&CallStorageWrites>,
) -> CallInvalidationResult {
    // Flow construction keeps every semantic constraint in the active context
    // selection. With a known empty footprint neither selection nor its facts
    // changes, so there is nothing to project or invalidate. Unknown writes
    // remain distinct: they still withdraw all semantic constraints below.
    if writes.is_some_and(|writes| writes.contributions.is_empty()) {
        return CallInvalidationResult {
            post_contexts: active_contexts,
            post_constraints: active_constraints,
            invalidations: HandleSpan::empty(),
        };
    }
    let invalidations_start = build.invalidations.events.frontier();
    let post_contexts = match writes {
        None => HandleSpan::empty(),
        Some(writes) => crate::flow::domain::filter_contexts_after_mutations(
            semantic,
            &mut build.contexts.semantic_context_refs,
            &mut build.invalidations.segments,
            &mut build.invalidations.events,
            active_contexts,
            FlowInvalidationSource::Call {
                statement_index: borrow_call.statement_index,
                call_ordinal: borrow_call.call_ordinal,
                target_symbol: borrow_call.target_symbol,
            },
            |plan, fact, place| {
                writes.contributions.iter().find_map(|contribution| {
                    if contribution
                        .write
                        .region
                        .preserves_owner_qualification(program, plan, fact, place)
                    {
                        return None;
                    }
                    crate::flow::domain::matching_mutation_for_fact_place(
                        program,
                        plan,
                        domains,
                        fact,
                        place,
                        &contribution.observations,
                    )
                })
            },
        ),
    };
    let post_constraints = project_constraint_refs_to_active_contexts(
        &mut build.contexts.constraint_refs,
        active_constraints,
        post_contexts,
        &build.contexts.semantic_context_refs,
    );
    let invalidations = appended_span_since(&build.invalidations.events, invalidations_start);

    CallInvalidationResult {
        post_contexts,
        post_constraints,
        invalidations,
    }
}
