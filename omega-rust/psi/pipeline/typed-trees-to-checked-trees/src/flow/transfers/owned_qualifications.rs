//! Owned structural moves preserve exact existing qualifications. This is
//! separate from copied predicates: no declaration, equal carrier geometry,
//! or matching label creates an authority fact. Call facts remain provisional
//! until the call/custody checker validates their exact producing invocation.

use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceHandle, ProgramPoint};

pub(super) fn append_owned_qualification_transfer(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    contexts: &crate::flow::FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    source: PlaceHandle,
    destination: PlaceHandle,
    destination_type: typed_trees::types::TypeReferenceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<facts::FactRef>,
) {
    let source = *semantic.places.get(source);
    let destination = *semantic.places.get(destination);
    let source_segments = semantic
        .place_segments
        .span_or_empty(source.segments)
        .to_vec();
    let destination_segments = semantic
        .place_segments
        .span_or_empty(destination.segments)
        .to_vec();
    let candidates = contexts
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .flat_map(|reference| {
            semantic
                .context_view(semantic.contexts.get(reference.context))
                .facts()
        })
        .copied()
        .collect::<Vec<_>>();
    for fact in candidates {
        let (FactPayload::DomainMembership {
            domain,
            domain_symbol,
            semantic_domain,
            ..
        }
        | FactPayload::ContractDomainMembership {
            domain,
            domain_symbol,
            semantic_domain,
            ..
        }) = fact.payload
        else {
            continue;
        };
        if !semantic_domain.is_valid()
            || !crate::facts::field_domain::domain_requires_provenance(program, domain_symbol)
        {
            continue;
        }
        let FactPlace::Place(place) = fact.place else {
            continue;
        };
        let place = semantic.places.get(place);
        let segments = semantic.place_segments.span_or_empty(place.segments);
        if place.root != source.root || !segments.starts_with(&source_segments) {
            continue;
        }
        let relative_path = &segments[source_segments.len()..];
        // A copied reference is not a copy of its referent's qualifications.
        // Those remain tied to the independently checked reference origins;
        // freezing them under the new slot would survive source invalidation.
        if !(0..=relative_path.len()).all(|length| {
            let Some(mut reference) = crate::flow::project_type_reference_from_segments(
                program,
                destination_type,
                &relative_path[..length],
            ) else {
                return false;
            };
            while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(reference)
            {
                reference = *base_type;
            }
            !matches!(
                program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            )
        }) {
            continue;
        }
        if !segments.iter().chain(&destination_segments).all(|segment| {
            matches!(segment,
            facts::PlaceSegment::Field { symbol } if symbol.is_valid())
                || matches!(segment, facts::PlaceSegment::FixedIndex { .. })
        }) {
            continue;
        }
        let mut rebased = destination_segments.clone();
        rebased.extend_from_slice(relative_path);
        let place = crate::semantic_places::append_place_with_segments(
            semantic,
            destination.root,
            &rebased,
        );
        let transferred = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: fact.evidence,
            payload: FactPayload::DomainMembership {
                value: typed_trees::expression::ExpressionHandle::invalid(),
                domain,
                domain_symbol,
                semantic_domain,
            },
        });
        semantic.append_ref(references, transferred);
    }
}
