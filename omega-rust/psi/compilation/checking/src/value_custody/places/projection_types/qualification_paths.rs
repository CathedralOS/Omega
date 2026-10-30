//! Translate qualification coordinates through their exact applied carrier.
//! Terminal member identities are scoped by the selected declaration; equal
//! spellings in another record or case never select its source symbol.
use super::{ObservationTypeCursor, TypeReferenceHandle};

/// Resolve an already checked membership's root-relative storage coordinate.
/// Pointer crossings describe the actual stored presentation; they confer no
/// access, lifetime, membership or ownership. The publisher retains the exact
/// checked occurrence and Core authenticates its live restoration window.
/// A final reference remains the selected descriptor home. If the membership
/// qualifies its referent, `result_membership_carrier` supplies that separate,
/// domain-sensitive suffix; the place alone cannot choose the endpoint.
pub fn qualification_storage_path(
    program: &dyn crate::CheckingInput,
    root_type: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<Vec<checked_trees::CheckedUnitStructuralPathSegment>, &'static str> {
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    use typed_trees::types::TypeReferenceNode;

    if !root_type.is_valid() {
        return Err("qualification storage has no exact root type");
    }
    let mut cursor = ObservationTypeCursor::new(root_type);
    let mut path = Vec::with_capacity(segments.len());
    for segment in segments {
        let mut crossed = Vec::new();
        loop {
            let carrier = cursor.carrier(program)?;
            if !matches!(
                program.type_reference_table.type_reference(carrier),
                TypeReferenceNode::Reference { .. }
            ) {
                break;
            }
            if crossed.contains(&carrier) {
                return Err("qualification storage crosses a cyclic reference carrier");
            }
            crossed.push(carrier);
            let stored_pointer =
                crate::stores_reference_shape(program, carrier, &cursor.projection.substitutions);
            let _ = cursor.advance(program, &Segment::Referent)?;
            // Slice views already store their referent presentation. They
            // are not an additional ordinary pointer wrapper.
            if stored_pointer {
                path.push(Segment::Referent);
            }
        }
        let (checked, symbol) = match segment {
            facts::PlaceSegment::Field { symbol } => (
                Segment::Field(
                    super::super::structural_member_identity(program, *symbol)
                        .ok_or("qualification field has no exact member identity")?,
                ),
                Some(*symbol),
            ),
            facts::PlaceSegment::Case { variant } => (
                Segment::Case(
                    super::super::structural_member_identity(program, *variant)
                        .ok_or("qualification case has no exact member identity")?,
                ),
                Some(*variant),
            ),
            facts::PlaceSegment::FixedIndex { index } => (
                Segment::FixedIndex(
                    u64::try_from(*index).map_err(|_| "qualification index exceeds u64")?,
                ),
                None,
            ),
            facts::PlaceSegment::Index { .. } => {
                return Err("qualification storage requires a statically captured index");
            }
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                return Err("qualification storage has an unsupported collection window");
            }
        };
        if cursor.advance(program, &checked)? != symbol {
            return Err("qualification member differs from its exact typed owner");
        }
        path.push(checked);
    }
    if cursor.selected_case.is_some() {
        return Err("qualification storage ends in an unprojected case");
    }
    Ok(path)
}

/// Inverse qualification geometry, retaining exact source member identities.
/// This selects declarations only; it creates no membership or access evidence.
pub fn canonical_qualification_path(
    program: &dyn crate::CheckingInput,
    root_type: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Result<Vec<facts::PlaceSegment>, &'static str> {
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    use typed_trees::types::TypeReferenceNode;
    if !root_type.is_valid() {
        return Err("qualification source has no exact carrier");
    }
    let mut cursor = ObservationTypeCursor::new(root_type);
    let mut result = Vec::with_capacity(path.len());
    for segment in path {
        // A borrowed parameter or slice presents its referent without a stored
        // pointer hop. Explicit stored-pointer hops remain in the input path.
        if !matches!(segment, Segment::Referent) {
            let mut crossed = Vec::new();
            loop {
                let carrier = cursor.carrier(program)?;
                if !matches!(
                    program.type_reference_table.type_reference(carrier),
                    TypeReferenceNode::Reference { .. }
                ) {
                    break;
                }
                if crossed.contains(&carrier) {
                    return Err("qualification source crosses a cyclic reference");
                }
                crossed.push(carrier);
                cursor.advance(program, &Segment::Referent)?;
            }
        }
        let member = if matches!(segment, Segment::AllElements) {
            cursor.advance_element(program)?
        } else {
            cursor.advance(program, segment)?
        };
        match segment {
            Segment::Field(_) => result.push(facts::PlaceSegment::Field {
                symbol: member.ok_or("qualification field lost its selected symbol")?,
            }),
            Segment::Case(_) => result.push(facts::PlaceSegment::Case {
                variant: member.ok_or("qualification case lost its selected symbol")?,
            }),
            Segment::FixedIndex(index) => result.push(facts::PlaceSegment::FixedIndex {
                index: usize::try_from(*index).map_err(|_| "qualification index exceeds usize")?,
            }),
            Segment::AllElements => result.push(crate::field_domains::WHOLE_ELEMENT_EXTENT),
            Segment::Referent => {}
            _ => return Err("qualification coordinate has no static source projection"),
        }
    }
    if cursor.selected_case.is_some() {
        return Err("qualification source ends in an unprojected case");
    }
    Ok(result)
}
