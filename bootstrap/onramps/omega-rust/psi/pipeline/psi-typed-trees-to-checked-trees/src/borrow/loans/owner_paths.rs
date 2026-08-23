use super::super::tracker::BorrowOwnerSegment;

pub(super) fn owner_path_from_place_segments(
    program: &psi_typed_trees::TypedTrees,
    segments: &[psi_facts::PlaceSegment],
) -> Vec<BorrowOwnerSegment> {
    segments
        .iter()
        .map(|segment| match segment {
            psi_facts::PlaceSegment::Field { symbol } => BorrowOwnerSegment::Field(*symbol),
            psi_facts::PlaceSegment::Case { variant } => BorrowOwnerSegment::Case(*variant),
            psi_facts::PlaceSegment::FixedIndex { index } => BorrowOwnerSegment::FixedIndex(*index),
            psi_facts::PlaceSegment::FixedRange { .. } => BorrowOwnerSegment::DynamicIndex,
            psi_facts::PlaceSegment::Index { expression } => program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .map(BorrowOwnerSegment::FixedIndex)
                .unwrap_or(BorrowOwnerSegment::DynamicIndex),
        })
        .collect()
}

pub(super) fn owner_path_matches(
    program: &psi_typed_trees::TypedTrees,
    owner_path: &[BorrowOwnerSegment],
    place_segments: &[psi_facts::PlaceSegment],
) -> bool {
    owner_path.len() <= place_segments.len()
        && owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (
                    BorrowOwnerSegment::Field(owner_symbol),
                    psi_facts::PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
                (
                    BorrowOwnerSegment::Case(owner_variant),
                    psi_facts::PlaceSegment::Case {
                        variant: place_variant,
                    },
                ) => owner_variant == place_variant,
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::FixedIndex { index: place_index },
                ) => owner_index == place_index,
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|place_index| *owner_index == place_index),
                (
                    BorrowOwnerSegment::DynamicIndex,
                    psi_facts::PlaceSegment::FixedIndex { .. }
                    | psi_facts::PlaceSegment::FixedRange { .. }
                    | psi_facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}

pub(super) fn place_path_matches_owner_prefix(
    program: &psi_typed_trees::TypedTrees,
    place_segments: &[psi_facts::PlaceSegment],
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    place_segments.len() <= owner_path.len()
        && owner_path_matches(program, &owner_path[..place_segments.len()], place_segments)
}
