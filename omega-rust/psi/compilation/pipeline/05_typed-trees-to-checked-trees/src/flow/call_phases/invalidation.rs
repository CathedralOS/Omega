use typed_trees::machine::Machine;
use typed_trees::state::State;
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
use facts::{FactPlan, PlaceRoot};

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
    // Observation aliases can name a pointee through its descriptor. Keep the
    // physical store list before adding those aliases: they cannot replace slots.
    binding_places: Option<Vec<CanonicalPlace>>,
}

impl CallStorageWrites {
    /// Authenticated separately backed content does not store into its inline
    /// owner. Retain its observation invalidation separately; an unresolved
    /// content capability still supplies no complete inline-store footprint.
    pub(in crate::flow) fn descriptor_stores(&self) -> Option<&[CanonicalPlace]> {
        self.binding_places.as_deref()
    }

    fn physical_binding_places(
        contributions: &[ObservedCallMutation],
    ) -> Option<Vec<CanonicalPlace>> {
        let mut stores = Vec::new();
        for contribution in contributions {
            match &contribution.write.region {
                crate::flow::mutation::StorageRegion::Declared(place) => stores.push(place.clone()),
                crate::flow::mutation::StorageRegion::ResidentContent { .. }
                | crate::flow::mutation::StorageRegion::NonresidentContent { .. } => {
                    // The owner is a descriptor, but other captured aliases can
                    // designate the written cells themselves. Retain those
                    // writes, including stored reference slots inside a view.
                    stores.extend(
                        contribution
                            .observations
                            .iter()
                            .filter(|place| *place != contribution.write.region.anchor())
                            .cloned(),
                    );
                }
                crate::flow::mutation::StorageRegion::UnresolvedContent { .. } => return None,
            }
        }
        Some(stores)
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
            // A source summary can still name a selector that changed after
            // capture. Bound that writer by its evaluated pointer, preserving
            // other aliases and the original effect when any actual is unknown.
            observed.contributions = observed
                .contributions
                .into_iter()
                .flat_map(|contribution| {
                    let Some(enclosures) = arguments.captured_write_enclosures(&contribution.write)
                    else {
                        return vec![contribution];
                    };
                    enclosures
                        .into_iter()
                        .map(|(region, enclosure)| {
                            let mut refined = contribution.clone();
                            refined.write.region =
                                crate::flow::mutation::StorageRegion::Declared(region);
                            for observation in &mut refined.observations {
                                if let Some(place) =
                                    super::copied_guarantees::intersect_captured_enclosure(
                                        observation,
                                        &enclosure,
                                    )
                                {
                                    *observation = place;
                                }
                            }
                            refined
                        })
                        .collect()
                })
                .collect();
            observed.binding_places = Self::physical_binding_places(&observed.contributions);
            for contribution in &mut observed.contributions {
                for alias in arguments.mutation_observation_aliases(&contribution.write) {
                    crate::shared::collections::push_unique(&mut contribution.observations, alias);
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
    machine: &Machine,
    state: &State,
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
                binding_places: CallStorageWrites::physical_binding_places(&contributions),
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
    machine: &Machine,
    state: &State,
    borrow_call: &BorrowCallFact,
) -> Option<Vec<ObservedCallMutation>> {
    let writes = call_mutation_writes(
        program,
        machine.symbol,
        state.symbol,
        borrow,
        borrow_call,
        build.state_mutation_summary_cache,
        build.call_frames,
    )?;
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
            if matches!(write.region.anchor().root, PlaceRoot::InvocationOutput(_)) {
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
        let mut observations = {
            close_storage_places_over_aliases(
                program,
                machine.symbol,
                state.symbol,
                borrow_call.statement_index,
                vec![write.region.anchor().clone()],
                build.call_frames,
            )
            .or_else(|| match &write.region {
                crate::flow::mutation::StorageRegion::Declared(place) => Some(vec![place.clone()]),
                // Do not lose content aliases when the inventory cannot be
                // reconstructed. Unknown remains full invalidation.
                crate::flow::mutation::StorageRegion::ResidentContent { .. } => None,
                crate::flow::mutation::StorageRegion::NonresidentContent { .. } => None,
                crate::flow::mutation::StorageRegion::UnresolvedContent { .. } => None,
            })
        }?;
        // Keep the current state's observation coordinates even when a
        // transported view proves that the physical store is content, not
        // descriptor replacement. Both namespaces must see invalidation.
        {
            if let crate::flow::mutation::StorageRegion::Declared(place) = &write.region
                && let Some(region) = crate::flow::entry_origins::nonresident_content_region(
                    program,
                    build,
                    machine,
                    state,
                    borrow_call.statement_index,
                    place,
                )
            {
                observations.push(region.anchor().clone());
                write.region = region;
            }
            crate::flow::entry_origins::extend_invocation_observations(
                program,
                build,
                machine,
                state,
                borrow_call.statement_index,
                &mut observations,
            );
        };
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
