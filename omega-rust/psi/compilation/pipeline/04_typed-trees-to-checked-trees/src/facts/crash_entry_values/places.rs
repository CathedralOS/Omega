//! Entry provenance for canonical places uses the same binding and write window
//! as authored operands. Projection identities are retained until the final
//! crash-predicate presentation; no source expression is manufactured.
use super::mutable::PlaceSegment;
use super::{entry_operand_symbol_at, field_entry_name};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(in crate::facts) fn entry_place(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    place: &crate::flow::CanonicalPlace,
) -> Option<CrashPredicateExpression> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    // Even stable whole storage cannot authenticate an unrelated declaration's
    // field. Replay the complete path from this binding's declared type first.
    crate::flow::canonical_place_type_reference(program, state_symbol, before_statement, place)?;
    let mut projection = Vec::with_capacity(place.segments.len());
    let mut members = Vec::with_capacity(place.segments.len());
    let mut selected_case = None;
    for segment in &place.segments {
        match *segment {
            facts::PlaceSegment::Case { variant } if selected_case.is_none() => {
                if !variant.is_valid() {
                    return None;
                }
                selected_case = Some(variant);
                projection.push(PlaceSegment::Case(variant));
            }
            facts::PlaceSegment::Field { symbol } => {
                if facts::payload_variant_for_field(program, symbol) != selected_case {
                    return None;
                }
                let field = program.data_definitions().iter().find_map(|data| {
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field) => {
                                (field.symbol == symbol).then_some(field)
                            }
                            typed_trees::data::DataMember::Variant(variant) => program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.symbol == symbol),
                        })
                })?;
                projection.push(PlaceSegment::Field(symbol));
                members.push(Projection::Member(field_entry_name(
                    program,
                    symbol,
                    field.name.as_str(),
                    None,
                )));
                selected_case = None;
            }
            facts::PlaceSegment::FixedIndex { index } if selected_case.is_none() => {
                projection.push(PlaceSegment::FixedIndex(index));
                members.push(Projection::Index(index));
            }
            // A case is only a qualifier of its exact payload field. Dynamic
            // indices and range descriptors are not scalar place projections.
            _ => return None,
        }
    }
    if selected_case.is_some() {
        return None;
    }
    let root = entry_operand_symbol_at(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        symbol,
        &projection,
        0,
    )?;
    Some(
        members
            .into_iter()
            .fold(root, |receiver, projection| match projection {
                Projection::Member(member) => CrashPredicateExpression::Member {
                    receiver: Box::new(receiver),
                    member,
                },
                Projection::Index(index) => CrashPredicateExpression::Indexed {
                    collection: Box::new(receiver),
                    index: Box::new(CrashPredicateExpression::Integer(index.to_string())),
                },
            }),
    )
}

/// Observation may read stable storage inside an owner whose whole value is
/// not an observation (for example, an owner with a nominal cleanup hook).
/// Every crossed descriptor validates its referee too: borrowed-storage
/// classification alone deliberately treats shared descriptors as opaque.
pub(super) fn projection_requires_write_scan(
    program: &TypedTrees,
    root_type: typed_trees::types::TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<bool> {
    let segments = path
        .iter()
        .map(|segment| match *segment {
            PlaceSegment::Field(symbol) => Some(facts::PlaceSegment::Field { symbol }),
            PlaceSegment::Case(variant) => Some(facts::PlaceSegment::Case { variant }),
            PlaceSegment::FixedIndex(index) => Some(facts::PlaceSegment::FixedIndex { index }),
            PlaceSegment::FixedRange { .. } | PlaceSegment::Opaque => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let mut mutable = false;
    for boundary in 0..segments.len() {
        // Case qualifies the following payload field; it is not another
        // storage location. Check the container before that combined hop.
        if boundary > 0 && matches!(segments[boundary - 1], facts::PlaceSegment::Case { .. }) {
            continue;
        }
        let current = crate::flow::project_type_reference_from_segments(
            program,
            root_type,
            &segments[..boundary],
        )?;
        mutable |= traversed_storage_requires_write_scan(program, current)?;
    }
    let endpoint =
        crate::flow::project_type_reference_from_segments(program, root_type, &segments)?;
    validation::has_stable_observable_contents(program, endpoint).then_some(mutable)
}

fn traversed_storage_requires_write_scan(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<bool> {
    match program.type_reference_table.type_reference(reference) {
        typed_trees::types::TypeReferenceNode::Reference {
            access, referee, ..
        } => {
            let nested = traversed_storage_requires_write_scan(program, *referee)?;
            Some(nested || *access == language_core::ReferenceAccess::Mutable)
        }
        _ => (validation::has_borrowed_storage_contents(program, reference)
            || validation::has_stable_observable_contents(program, reference))
        .then_some(false),
    }
}

enum Projection {
    Member(String),
    Index(usize),
}

#[cfg(test)]
mod tests;
