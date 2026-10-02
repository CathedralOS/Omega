use super::super::tracker::BorrowOwnerSegment;

/// Resolve the storage containing a replaced reference slot. A carried loan
/// is an address crossing only where the declaration itself is a reference;
/// the old loan held by the slot being replaced is deliberately not crossed.
pub(crate) fn store_destination_places<'loan>(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    target: checked_trees::CapturedPlace,
    loans: impl Iterator<
        Item = (
            symbols::SymbolHandle,
            &'loan [BorrowOwnerSegment],
            symbols::SymbolHandle,
            &'loan [facts::PlaceSegment],
        ),
    >,
) -> Vec<checked_trees::CapturedPlace> {
    let owner_type = program
        .state_local_declaration(state, target.root_symbol)
        .map(|(_, local)| local.type_reference)
        .or_else(|| {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == target.root_symbol)
                .map(|parameter| parameter.type_reference)
        });
    let mut destinations = Vec::new();
    for (owner, owner_path, source, source_path) in loans {
        if owner != target.root_symbol
            || owner_path.len() >= target.segments.len()
            || !owner_path_matches(program, owner_path, &target.segments)
            || !owner_type
                .and_then(|reference| {
                    crate::flow::project_type_reference_from_segments(
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
            root_symbol: source,
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

pub(super) fn owner_path_from_place_segments(
    program: &typed_trees::TypedTrees,
    segments: &[facts::PlaceSegment],
) -> Vec<BorrowOwnerSegment> {
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => BorrowOwnerSegment::Field(*symbol),
            facts::PlaceSegment::Case { variant } => BorrowOwnerSegment::Case(*variant),
            facts::PlaceSegment::FixedIndex { index } => BorrowOwnerSegment::FixedIndex(*index),
            facts::PlaceSegment::FixedRange { .. } => BorrowOwnerSegment::DynamicIndex,
            facts::PlaceSegment::Index { expression } => program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .map(BorrowOwnerSegment::FixedIndex)
                .unwrap_or(BorrowOwnerSegment::DynamicIndex),
        })
        .collect()
}

pub(super) fn owner_path_matches(
    program: &typed_trees::TypedTrees,
    owner_path: &[BorrowOwnerSegment],
    place_segments: &[facts::PlaceSegment],
) -> bool {
    owner_path.len() <= place_segments.len()
        && owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (
                    BorrowOwnerSegment::Field(owner_symbol),
                    facts::PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
                (
                    BorrowOwnerSegment::Case(owner_variant),
                    facts::PlaceSegment::Case {
                        variant: place_variant,
                    },
                ) => owner_variant == place_variant,
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::FixedIndex { index: place_index },
                ) => owner_index == place_index,
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|place_index| *owner_index == place_index),
                (
                    BorrowOwnerSegment::DynamicIndex,
                    facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::FixedRange { .. }
                    | facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}

pub(super) fn place_path_matches_owner_prefix(
    program: &typed_trees::TypedTrees,
    place_segments: &[facts::PlaceSegment],
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    place_segments.len() <= owner_path.len()
        && owner_path_matches(program, &owner_path[..place_segments.len()], place_segments)
}
