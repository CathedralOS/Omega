//! Consume a normal-result promise on its captured value, independently of
//! address precision. Its dependency roster and the complete intervening write
//! interval remain separate obligations. No annotation or matching carrier
//! creates membership, and missing chronology fails closed.

use super::CapturedValue;
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::{FactOrigin, FactPayload, PlaceRoot};

pub(crate) fn captured_result_membership(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    value: &CapturedValue<'_>,
    observation: &FlowOperandFact,
    domain: symbols::SymbolHandle,
    identity: language_semantics::SemanticDomainId,
) -> bool {
    let source = value.operand.row();
    let Some(writes) =
        crate::flow::operand_contents_writes_between(program, &facts.flow, source, observation)
    else {
        return false;
    };
    let contexts = facts
        .flow
        .contexts
        .semantic_constraint_contexts(source.constraints);
    for context in contexts {
        let references = facts
            .semantic
            .refs
            .span_or_empty(facts.semantic.contexts.get(context).facts);
        for reference in references {
            let promise = facts.semantic.facts.get(reference.fact);
            if promise.origin != FactOrigin::CallEnsures {
                continue;
            }
            let Some((_, _, actual_domain, actual_identity)) = promise.payload.domain_membership()
            else {
                continue;
            };
            if actual_domain != domain || actual_identity != identity {
                continue;
            }
            let Some(place) = promise.place.as_place() else {
                continue;
            };
            let place = facts.semantic.places.get(place);
            if place.root != PlaceRoot::Expression(source.expression)
                || facts.semantic.place_segments.span_or_empty(place.segments)
                    != value.projection.remaining.as_slice()
            {
                continue;
            }
            let dependencies = references
                .iter()
                .filter_map(|candidate| {
                    let dependency = facts.semantic.facts.get(candidate.fact);
                    matches!(dependency.payload, FactPayload::StorageDependency { dependent }
                    if dependent == reference.fact)
                    .then_some(dependency.place)
                })
                .collect::<Vec<_>>();
            // Owned snapshots do not assert reference-content preservation.
            // Borrowed completion always retains at least an unknown dependency.
            if dependencies.is_empty() {
                continue;
            }
            if dependencies.iter().all(|dependency| {
                writes.iter().all(|(_, write)| {
                    let Some(place) = dependency.as_place() else {
                        return false;
                    };
                    let place = facts.semantic.places.get(place);
                    place.root != write.root
                        || !crate::flow::canonical_place_segments_may_overlap(
                            program,
                            facts.semantic.place_segments.span_or_empty(place.segments),
                            facts
                                .flow
                                .control
                                .operand_referent_segments
                                .span_or_empty(write.segments),
                        )
                })
            }) {
                return true;
            }
        }
    }
    false
}
