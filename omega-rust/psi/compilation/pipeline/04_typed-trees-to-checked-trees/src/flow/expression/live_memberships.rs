//! Meet live storage memberships by their semantic subject and evidence.
//!
//! Distinct calls can hand back the same field promise in distinct fact records.
//! Context-handle intersection alone loses that promise. Every incoming branch
//! must carry it on the exact place and domain instance with the same evidence;
//! this transports qualification, never establishes content or equates lineage.

use super::Execution;
use arena::HandleSpan;
use checked_trees::{FlowConstraintRef, FlowSemanticContextRef};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint, QualificationEvidence};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LiveMembership {
    place: validation::CanonicalPlace,
    domain: symbols::SymbolHandle,
    instance: language_semantics::SemanticDomainId,
    evidence: QualificationEvidence,
}

// Compute each structural path key once per batch. A bucket is only a
// candidate set: full membership equality below still checks root/path,
// domain instance and qualification evidence, including on hash collisions.
fn membership_buckets(memberships: &[LiveMembership]) -> facts::places::PlaceBucketIndex {
    facts::places::PlaceBucketIndex::new(memberships.iter().map(|membership| {
        facts::places::place_bucket_hash(membership.place.root, &membership.place.segments)
    }))
}

fn deduplicate(memberships: &mut Vec<LiveMembership>) {
    let buckets = membership_buckets(memberships);
    let first =
        buckets.group_equal_positions(|left, right| memberships[left] == memberships[right]);
    let mut position = 0;
    memberships.retain(|_| {
        let retained = first[position] == position;
        position += 1;
        retained
    });
}

pub(super) fn retain_common(memberships: &mut Vec<LiveMembership>, incoming: &[LiveMembership]) {
    let buckets = membership_buckets(incoming);
    memberships.retain(|membership| {
        let key =
            facts::places::place_bucket_hash(membership.place.root, &membership.place.segments);
        buckets
            .candidates(key)
            .any(|position| incoming[position] == *membership)
    });
}

impl Execution<'_, '_, '_> {
    pub(super) fn live_memberships(
        &self,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Vec<LiveMembership> {
        let mut memberships = Vec::new();
        for fact in crate::flow::active_context_facts(self.context, self.semantic, contexts) {
            let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                continue;
            };
            let Some(place) = fact.place.as_place().and_then(|place| {
                crate::flow::canonical_place_from_semantic_place(
                    self.program,
                    self.semantic,
                    self.semantic.places.get(place),
                )
            }) else {
                continue;
            };
            let membership = LiveMembership {
                place,
                domain,
                instance,
                evidence: fact.evidence,
            };
            memberships.push(membership);
        }
        deduplicate(&mut memberships);
        memberships
    }

    pub(super) fn publish_live_memberships(
        &mut self,
        memberships: Vec<LiveMembership>,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let retained = self.live_memberships(*contexts);
        let retained_buckets = membership_buckets(&retained);
        let point = ProgramPoint::Statement {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
        };
        for membership in memberships {
            let key =
                facts::places::place_bucket_hash(membership.place.root, &membership.place.segments);
            if retained_buckets
                .candidates(key)
                .any(|position| retained[position] == membership)
            {
                continue;
            }
            let place = crate::semantic::places::append_place(
                self.semantic,
                membership.place.root,
                &membership.place.segments,
            );
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: membership.evidence,
                payload: FactPayload::DomainMembership {
                    value: typed_trees::expression::ExpressionHandle::invalid(),
                    domain: HandleSpan::empty(),
                    domain_symbol: membership.domain,
                    semantic_domain: membership.instance,
                },
            });
            let mut references = HandleSpan::empty();
            self.semantic.append_ref(&mut references, fact);
            // Separate contexts retain place-granular invalidation after the join.
            let context = self.semantic.append_context(point, references);
            crate::flow::reference_spans::append_flow_reference(
                &mut self.context.contexts.semantic_context_refs,
                contexts,
                FlowSemanticContextRef { context },
            );
            crate::flow::append_constraint_ref(
                &mut self.context.contexts.constraint_refs,
                constraints,
                checked_trees::FlowConstraintKind::SemanticContext { context },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn membership(index: usize) -> LiveMembership {
        LiveMembership {
            place: validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(symbols::SymbolHandle::invalid()),
                segments: vec![facts::PlaceSegment::FixedIndex { index }],
            },
            domain: symbols::SymbolHandle::invalid(),
            instance: language_semantics::SemanticDomainId::NULL,
            evidence: QualificationEvidence::default(),
        }
    }

    #[test]
    fn membership_batches_preserve_order_instances_and_evidence() {
        let originals: Vec<_> = (0..128).rev().map(membership).collect();
        let mut values = originals.clone();
        values.extend(originals.iter().rev().cloned());
        let mut changed = membership(64);
        changed.evidence.origin = language_semantics::QualificationEvidenceOrigin::Propagated;
        values.push(changed.clone());
        let mut expected = originals.clone();
        expected.push(changed);
        let mut different_instance = membership(64);
        different_instance.instance = language_semantics::SemanticDomainId(1);
        values.push(different_instance.clone());
        expected.push(different_instance);
        deduplicate(&mut values);
        assert_eq!(values, expected);
        retain_common(&mut values, &originals);
        assert_eq!(values, originals);
        let incoming: Vec<_> = originals.iter().step_by(2).cloned().collect();
        retain_common(&mut values, &incoming);
        assert_eq!(values, incoming);
        retain_common(&mut values, &[]);
        assert!(values.is_empty());
        deduplicate(&mut values);
        assert!(values.is_empty());
    }
}
