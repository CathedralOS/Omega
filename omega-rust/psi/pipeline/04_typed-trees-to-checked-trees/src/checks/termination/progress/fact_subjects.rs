//! Fact domains, subjects and their labels.

use crate::fact_plan::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use language_semantics::ProgressSubject;
use symbols::SymbolHandle;

pub(crate) fn fact_domain(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    payload: FactPayload,
) -> Option<SymbolHandle> {
    let (domain_symbol, semantic_domain) = match payload {
        FactPayload::DomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        }
        | FactPayload::ContractDomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        } => (domain_symbol, semantic_domain),
        _ => return None,
    };
    // Progress premises carry declaration identities, not application indices.
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
        .filter(|domain| {
            symbol_resolved_trees_to_typed_trees::typed_trees::domain::index_parameters(
                program, domain,
            )
            .is_empty()
                && (!semantic_domain.is_valid() || semantic_domain == domain.semantic_id)
        })
        .map(|domain| domain.symbol)
}

pub(crate) fn fact_subject(
    semantic: &crate::fact_plan::FactPlan,
    place: FactPlace,
) -> Option<ProgressSubject> {
    match place {
        FactPlace::Place(handle) => {
            let place = *semantic.places.get(handle);
            subject_from_place(
                place.root,
                semantic.place_segments.span_or_empty(place.segments),
            )
        }
        FactPlace::Symbol(symbol) => Some(ProgressSubject {
            root: symbol,
            projections: Vec::new(),
        }),
        _ => None,
    }
}

pub(crate) fn subject_from_place(
    root: PlaceRoot,
    segments: &[PlaceSegment],
) -> Option<ProgressSubject> {
    let PlaceRoot::Symbol(root) = root else {
        return None;
    };
    let mut projections = Vec::new();
    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => projections.push(*symbol),
            // The field symbol already carries exact variant identity, matching
            // authored member-path normalization.
            PlaceSegment::Case { .. } => {}
            PlaceSegment::FixedIndex { .. }
            | PlaceSegment::FixedRange { .. }
            | PlaceSegment::Index { .. } => return None,
        }
    }
    Some(ProgressSubject { root, projections })
}

pub(crate) fn profile_label(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    profile: language_semantics::SemanticDomainId,
) -> String {
    program
        .semantic_domains
        .name(profile)
        .unwrap_or("<unknown-progress-profile>")
        .to_owned()
}

pub(crate) fn subject_label(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    subject: &ProgressSubject,
) -> String {
    let mut label = program.symbols.display_path(subject.root, "::");
    for projection in &subject.projections {
        label.push('.');
        label.push_str(&program.symbols.display_path(*projection, "::"));
    }
    label
}
