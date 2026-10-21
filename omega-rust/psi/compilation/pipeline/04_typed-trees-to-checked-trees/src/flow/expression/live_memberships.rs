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

#[derive(PartialEq, Eq)]
pub(super) struct LiveMembership {
    place: validation::CanonicalPlace,
    domain: symbols::SymbolHandle,
    instance: language_semantics::SemanticDomainId,
    evidence: QualificationEvidence,
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
            if !memberships.contains(&membership) {
                memberships.push(membership);
            }
        }
        memberships
    }

    pub(super) fn publish_live_memberships(
        &mut self,
        memberships: Vec<LiveMembership>,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let retained = self.live_memberships(*contexts);
        let point = ProgramPoint::Statement {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
        };
        for membership in memberships {
            if retained.contains(&membership) {
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
