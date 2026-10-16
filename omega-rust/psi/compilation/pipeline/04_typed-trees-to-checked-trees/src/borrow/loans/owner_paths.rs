use super::super::tracker::BorrowOwnerSegment;

/// Resolve the storage containing a replaced reference slot. A carried loan
/// is an address crossing only where the declaration itself is a reference;
/// the old loan held by the slot being replaced is deliberately not crossed.
/// A referent write also crosses the selected descriptor itself; rebinding
/// that descriptor crosses only its ancestors.
pub(crate) fn store_destination_places<'loan>(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    target: checked_trees::CapturedPlace,
    writes_referent: bool,
    loans: impl Iterator<
        Item = (
            facts::PlaceRoot,
            &'loan [BorrowOwnerSegment],
            arena::Handle<checked_trees::IncomingReferenceIdentity>,
            facts::PlaceRoot,
            &'loan [facts::PlaceSegment],
        ),
    >,
) -> Vec<checked_trees::CapturedPlace> {
    let owner_type = validation::canonical_place_type_reference(
        program,
        state.symbol,
        state.statement_nodes.count() as usize,
        &validation::CanonicalPlace {
            root: target.root,
            segments: Vec::new(),
        },
    );
    let mut destinations = Vec::new();
    for (owner, owner_path, incoming_reference, source, source_path) in loans {
        if owner != target.root
            || owner_path.len() > target.segments.len()
            || (owner_path.len() == target.segments.len() && !writes_referent)
            || !owner_path_matches(program, owner_path, &target.segments)
            || !owner_type
                .and_then(|reference| {
                    validation::project_type_reference_from_segments(
                        program,
                        reference,
                        &target.segments[..owner_path.len()],
                    )
                })
                .is_some_and(|reference| super::types::is_reference_type(program, reference))
        {
            continue;
        }
        let mut segments = source_path.to_vec();
        segments.extend_from_slice(&target.segments[owner_path.len()..]);
        let destination = checked_trees::CapturedPlace {
            incoming_reference,
            root: source,
            segments,
        };
        if !destinations.contains(&destination) {
            destinations.push(destination);
        }
    }
    if destinations.is_empty() {
        destinations.push(target);
    }
    destinations
}

pub(crate) fn owner_path_from_place_segments(
    program: &dyn ::validation::CheckingInput,
    segments: &[facts::PlaceSegment],
) -> Vec<BorrowOwnerSegment> {
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => BorrowOwnerSegment::Field(*symbol),
            facts::PlaceSegment::Case { variant } => BorrowOwnerSegment::Case(*variant),
            facts::PlaceSegment::FixedIndex { index } => BorrowOwnerSegment::FixedIndex(*index),
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                BorrowOwnerSegment::DynamicIndex
            }
            facts::PlaceSegment::Index { expression } => program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .map(BorrowOwnerSegment::FixedIndex)
                .unwrap_or(BorrowOwnerSegment::DynamicIndex),
        })
        .collect()
}

/// Whether one owner-path segment selects the same storage as one place
/// segment. `DynamicIndex` owners match no fixed shape here — each caller
/// decides which index spellings it accepts.
pub(crate) fn owner_segment_matches_place_segment(
    program: &dyn ::validation::CheckingInput,
    owner: &BorrowOwnerSegment,
    place: &facts::PlaceSegment,
) -> Option<bool> {
    match (owner, place) {
        (
            BorrowOwnerSegment::Field(owner_symbol),
            facts::PlaceSegment::Field {
                symbol: place_symbol,
            },
        ) => Some(
            !place_symbol.is_valid() || owner_symbol == place_symbol || {
                // This join already has one exact storage root. Prepared generic
                // members may use different symbols for that root's same schema
                // slot; compare declaration identities, never field spelling.
                program
                    .any_field_owner(*owner_symbol)
                    .zip(program.any_field_owner(*place_symbol))
                    .is_some_and(|(owner, selected)| {
                        owner.1.path_identity() == selected.1.path_identity()
                    })
            },
        ),
        (
            BorrowOwnerSegment::Case(owner_variant),
            facts::PlaceSegment::Case {
                variant: place_variant,
            },
        ) => Some(owner_variant == place_variant),
        (
            BorrowOwnerSegment::FixedIndex(owner_index),
            facts::PlaceSegment::FixedIndex { index: place_index },
        ) => Some(owner_index == place_index),
        (
            BorrowOwnerSegment::FixedIndex(owner_index),
            facts::PlaceSegment::Index { expression },
        ) => Some(
            program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .is_none_or(|place_index| *owner_index == place_index),
        ),
        _ => None,
    }
}

pub(super) fn owner_path_matches(
    program: &dyn ::validation::CheckingInput,
    owner_path: &[BorrowOwnerSegment],
    place_segments: &[facts::PlaceSegment],
) -> bool {
    owner_path.len() <= place_segments.len()
        && owner_path.iter().zip(place_segments).all(|(owner, place)| {
            owner_segment_matches_place_segment(program, owner, place).unwrap_or_else(|| {
                matches!(
                    (owner, place),
                    (
                        BorrowOwnerSegment::DynamicIndex,
                        facts::PlaceSegment::FixedIndex { .. }
                            | facts::PlaceSegment::FixedRange { .. }
                            | facts::PlaceSegment::OpaqueRange
                            | facts::PlaceSegment::Index { .. }
                    )
                )
            })
        })
}

/// A selected projection retains every carried loan it may include.
pub(super) fn place_path_matches_owner_prefix(
    program: &dyn ::validation::CheckingInput,
    place_segments: &[facts::PlaceSegment],
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    place_segments.len() <= owner_path.len()
        && owner_path_matches(program, &owner_path[..place_segments.len()], place_segments)
}

/// Retirement requires definite coverage, unlike access conflict detection.
/// One unknown element overlaps many homes but overwrites only one of them;
/// conversely, an exact element cannot retire a loan whose home may be another
/// element. Replacing an enclosing collection still covers all its children.
pub(crate) fn place_path_covers_owner(
    program: &dyn ::validation::CheckingInput,
    place_segments: &[facts::PlaceSegment],
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    place_segments.len() <= owner_path.len()
        && owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (BorrowOwnerSegment::DynamicIndex, _) => false,
                (_, facts::PlaceSegment::Index { expression }) => {
                    let BorrowOwnerSegment::FixedIndex(owner_index) = owner else {
                        return false;
                    };
                    program
                        .expression_table
                        .constant_integer_value(*expression)
                        .and_then(|value| usize::try_from(value).ok())
                        .is_some_and(|place_index| *owner_index == place_index)
                }
                (_, facts::PlaceSegment::Field { symbol }) if !symbol.is_valid() => false,
                _ => owner_segment_matches_place_segment(program, owner, place).unwrap_or(false),
            })
}
