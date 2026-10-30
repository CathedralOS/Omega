//! Normal-result promises on one immutable captured operand inventory.
//!
//! A nominal argument can owe thousands of field qualifications. Prepare its
//! source promises once, rather than scanning every captured fact per field.
//! Each promise keeps only dependencies from its own original context; contexts
//! and capture epochs are never combined. This is an invocation-local evidence
//! index, not a proof cache: each query still checks its exact projection, domain
//! instance, observation chronology and complete intervening write footprint.

use super::CapturedValue;
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use std::collections::BTreeMap;

struct ResultPromise<'facts> {
    path: &'facts [PlaceSegment],
    domain: symbols::SymbolHandle,
    instance: language_semantics::SemanticDomainId,
    dependencies: Vec<FactPlace>,
}

pub(crate) struct CapturedResultMemberships<'facts> {
    facts: &'facts CheckFacts,
    // The borrowed owner fixes all rows and generations for this invocation.
    sources: BTreeMap<(u32, u32), Vec<ResultPromise<'facts>>>,
    scratch: Vec<ResultPromise<'facts>>,
}

impl<'facts> CapturedResultMemberships<'facts> {
    pub(crate) fn new(facts: &'facts CheckFacts) -> Self {
        Self {
            facts,
            sources: BTreeMap::new(),
            scratch: Vec::new(),
        }
    }

    pub(crate) fn contains(
        &mut self,
        program: &dyn validation::CheckingInput,
        value: &CapturedValue<'facts>,
        observation: &FlowOperandFact,
        domain: symbols::SymbolHandle,
        identity: language_semantics::SemanticDomainId,
    ) -> bool {
        let _profile = phase_profile::guard("05.captured_result_membership");
        if !value.operand.belongs_to(&self.facts.flow.control) {
            return false;
        }
        let source = value.operand.row();
        let Some(writes) = phase_profile::scope("05.captured_result_membership.writes", || {
            crate::flow::operand_contents_writes_between(
                program,
                &self.facts.flow,
                source,
                observation,
            )
        }) else {
            return false;
        };
        let prepare = || {
            phase_profile::scope("05.captured_result_membership.prepare", || {
                prepare_promises(self.facts, source)
            })
        };
        let promises = if let Some(handle) = value.operand.recorded_in(&self.facts.flow.control) {
            self.sources
                .entry((handle.arena_index(), handle.generation()))
                .or_insert_with(prepare)
        } else {
            // Scratch rows have no durable arena association. Never reuse them.
            self.scratch = prepare();
            &self.scratch
        };
        promises.iter().any(|promise| {
            promise.domain == domain
                && promise.instance == identity
                && promise.path == value.projection.remaining.as_slice()
                && !promise.dependencies.is_empty()
                && promise.dependencies.iter().all(|dependency| {
                    writes.iter().all(|(_, write)| {
                        let Some(place) = dependency.as_place() else {
                            return false;
                        };
                        let place = self.facts.semantic.places.get(place);
                        place.root != write.root
                            || !crate::flow::canonical_place_segments_may_overlap(
                                program,
                                self.facts
                                    .semantic
                                    .place_segments
                                    .span_or_empty(place.segments),
                                self.facts
                                    .flow
                                    .control
                                    .operand_referent_segments
                                    .span_or_empty(write.segments),
                            )
                    })
                })
        })
    }
}

fn prepare_promises<'facts>(
    facts: &'facts CheckFacts,
    source: &FlowOperandFact,
) -> Vec<ResultPromise<'facts>> {
    let mut promises = Vec::new();
    for context in facts
        .flow
        .contexts
        .semantic_constraint_contexts(source.constraints)
    {
        let references = facts
            .semantic
            .refs
            .span_or_empty(facts.semantic.contexts.get(context).facts);
        let mut candidates = Vec::new();
        let mut dependencies = BTreeMap::<(u32, u32), Vec<FactPlace>>::new();
        for reference in references {
            let fact = facts.semantic.facts.get(reference.fact);
            if let FactPayload::StorageDependency { dependent } = fact.payload {
                dependencies
                    .entry((dependent.arena_index(), dependent.generation()))
                    .or_default()
                    .push(fact.place);
            }
            if fact.origin != FactOrigin::CallEnsures {
                continue;
            }
            let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                continue;
            };
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let place = facts.semantic.places.get(place);
            if place.root == PlaceRoot::Expression(source.expression) {
                candidates.push((reference.fact, place.segments, domain, instance));
            }
        }
        // Keep the join inside this context. A dependency in a different
        // namespace must not qualify a promise that never carried it.
        for (fact, path, domain, instance) in candidates {
            promises.push(ResultPromise {
                path: facts.semantic.place_segments.span_or_empty(path),
                domain,
                instance,
                dependencies: dependencies
                    .get(&(fact.arena_index(), fact.generation()))
                    .cloned()
                    .unwrap_or_default(),
            });
        }
    }
    promises
}
