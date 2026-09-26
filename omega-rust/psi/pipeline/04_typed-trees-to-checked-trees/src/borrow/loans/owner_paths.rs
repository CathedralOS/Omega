use super::super::tracker::BorrowOwnerSegment;

pub(super) fn owner_path_from_place_segments(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    segments: &[crate::fact_plan::PlaceSegment],
) -> Vec<BorrowOwnerSegment> {
    segments
        .iter()
        .map(|segment| match segment {
            crate::fact_plan::PlaceSegment::Field { symbol } => BorrowOwnerSegment::Field(*symbol),
            crate::fact_plan::PlaceSegment::Case { variant } => BorrowOwnerSegment::Case(*variant),
            crate::fact_plan::PlaceSegment::FixedIndex { index } => {
                BorrowOwnerSegment::FixedIndex(*index)
            }
            crate::fact_plan::PlaceSegment::FixedRange { .. } => BorrowOwnerSegment::DynamicIndex,
            crate::fact_plan::PlaceSegment::Index { expression } => program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .map(BorrowOwnerSegment::FixedIndex)
                .unwrap_or(BorrowOwnerSegment::DynamicIndex),
        })
        .collect()
}

pub(super) fn owner_path_matches(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    owner_path: &[BorrowOwnerSegment],
    place_segments: &[crate::fact_plan::PlaceSegment],
) -> bool {
    owner_path.len() <= place_segments.len()
        && owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (
                    BorrowOwnerSegment::Field(owner_symbol),
                    crate::fact_plan::PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
                (
                    BorrowOwnerSegment::Case(owner_variant),
                    crate::fact_plan::PlaceSegment::Case {
                        variant: place_variant,
                    },
                ) => owner_variant == place_variant,
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    crate::fact_plan::PlaceSegment::FixedIndex { index: place_index },
                ) => owner_index == place_index,
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    crate::fact_plan::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|place_index| *owner_index == place_index),
                (
                    BorrowOwnerSegment::DynamicIndex,
                    crate::fact_plan::PlaceSegment::FixedIndex { .. }
                    | crate::fact_plan::PlaceSegment::FixedRange { .. }
                    | crate::fact_plan::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}

pub(super) fn place_path_matches_owner_prefix(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    place_segments: &[crate::fact_plan::PlaceSegment],
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    place_segments.len() <= owner_path.len()
        && owner_path_matches(program, &owner_path[..place_segments.len()], place_segments)
}
