//! Fact domains, subjects and their labels.

use facts::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use language_semantics::ProgressSubject;
use symbols::SymbolHandle;

pub(crate) fn fact_domain(
    program: &dyn ::validation::CheckingInput,
    payload: FactPayload,
) -> Option<SymbolHandle> {
    let (domain_symbol, semantic_domain) = match payload {
        facts::domain_membership_symbol_and_instance_pattern!(domain_symbol, semantic_domain) => {
            (domain_symbol, semantic_domain)
        }
        _ => return None,
    };
    // Progress premises carry declaration identities, not application indices.
    program
        .domain_definition(domain_symbol)
        .filter(|domain| {
            typed_trees::domain::index_parameters(program, domain).is_empty()
                && (!semantic_domain.is_valid() || semantic_domain == domain.semantic_id)
        })
        .map(|domain| domain.symbol)
}

pub(crate) fn fact_subject(
    semantic: &facts::FactPlan,
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
    let root = root.as_symbol()?;
    let mut projections = Vec::new();
    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => projections.push(*symbol),
            // The field symbol already carries exact variant identity, matching
            // authored member-path normalization.
            PlaceSegment::Case { .. } => {}
            PlaceSegment::FixedIndex { .. }
            | PlaceSegment::FixedRange { .. }
            | PlaceSegment::OpaqueRange
            | PlaceSegment::Index { .. } => return None,
        }
    }
    Some(ProgressSubject { root, projections })
}

pub(crate) fn profile_label(
    program: &dyn ::validation::CheckingInput,
    profile: language_semantics::SemanticDomainId,
) -> String {
    program
        .semantic_domains
        .name(profile)
        .unwrap_or("<unknown-progress-profile>")
        .to_owned()
}

pub(crate) fn subject_label(
    program: &dyn ::validation::CheckingInput,
    subject: &ProgressSubject,
) -> String {
    let mut label = program.symbols.display_path(subject.root, "::");
    for projection in &subject.projections {
        label.push('.');
        label.push_str(&program.symbols.display_path(*projection, "::"));
    }
    label
}
