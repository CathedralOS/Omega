/// A copied observation may retain only selectors fixed at its capture.
/// Runtime selectors need separate dependency/capture evidence.
pub(crate) fn place_segment_has_static_identity(segment: &facts::PlaceSegment) -> bool {
    match segment {
        facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
        facts::PlaceSegment::Case { variant } => variant.is_valid(),
        facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::FixedRange { .. } => true,
        facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange => false,
    }
}

/// A missing field or case identity gives no information about its descendants.
/// Runtime indexes retain their separate conservative selector semantics.
pub(crate) fn place_segment_has_unresolved_identity(segment: facts::PlaceSegment) -> bool {
    match segment {
        facts::PlaceSegment::Field { symbol } => !symbol.is_valid(),
        facts::PlaceSegment::Case { variant } => !variant.is_valid(),
        _ => false,
    }
}

/// Equality of the segment kinds with statically-comparable identity: `Field`
/// symbols, `Case` variants, and `FixedIndex` values. `FixedRange` and `Index`
/// pairs defer to each caller — equality, overlap, and containment compare
/// them differently.
pub(crate) fn static_place_segment_pair_equal(
    left: facts::PlaceSegment,
    right: facts::PlaceSegment,
) -> Option<bool> {
    match (left, right) {
        (
            facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => Some(left_symbol == right_symbol),
        (
            facts::PlaceSegment::Case {
                variant: left_variant,
            },
            facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => Some(left_variant == right_variant),
        (
            facts::PlaceSegment::FixedIndex { index: left_index },
            facts::PlaceSegment::FixedIndex { index: right_index },
        ) => Some(left_index == right_index),
        _ => None,
    }
}

/// Whether two segment spellings select the same storage at one program point.
/// Runtime `Index` operands compare by structural expression identity — a
/// `buffer[a..b]` spelled inside a `requires` clause selects the same window the
/// body's re-spelled `buffer[a..b]` selects, even though the arena nodes differ.
pub(crate) fn canonical_place_segments_equal(
    program: &dyn ::validation::CheckingInput,
    left: facts::PlaceSegment,
    right: facts::PlaceSegment,
) -> bool {
    if place_segment_has_unresolved_identity(left) || place_segment_has_unresolved_identity(right) {
        return false;
    }
    static_place_segment_pair_equal(left, right).unwrap_or_else(|| match (left, right) {
        (
            facts::PlaceSegment::FixedRange {
                start: left_start,
                end: left_end,
            },
            facts::PlaceSegment::FixedRange {
                start: right_start,
                end: right_end,
            },
        ) => left_start == right_start && left_end == right_end,
        (facts::PlaceSegment::FixedIndex { index }, facts::PlaceSegment::Index { expression })
        | (facts::PlaceSegment::Index { expression }, facts::PlaceSegment::FixedIndex { index }) => {
            expression_static_index(program, expression).is_some_and(|value| value == index)
        }
        (
            facts::PlaceSegment::Index {
                expression: left_expression,
            },
            facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => program
            .expression_table
            .expressions_structurally_equal(left_expression, right_expression),
        _ => false,
    })
}
use checked_trees::expression::ExpressionHandle;

/// Whether two canonical places name the same storage at one program point:
/// the event-normalized roots match and every segment pair selects the same
/// storage. The one place-equality operation fact lookups share — a fact read
/// must only match a place provably identical to its subject.
pub(crate) fn canonical_places_agree(
    program: &dyn ::validation::CheckingInput,
    left: &validation::CanonicalPlace,
    right: &validation::CanonicalPlace,
) -> bool {
    left.root == right.root
        && left.segments.len() == right.segments.len()
        && left
            .segments
            .iter()
            .zip(right.segments.iter())
            .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
}

pub(crate) fn canonical_place_segments_may_overlap(
    program: &dyn ::validation::CheckingInput,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    for (&left_segment, &right_segment) in left.iter().zip(right) {
        if place_segment_has_unresolved_identity(left_segment)
            || place_segment_has_unresolved_identity(right_segment)
        {
            return true;
        }
        if !canonical_place_segment_pair_may_overlap(program, left_segment, right_segment) {
            return false;
        }
    }
    true
}

pub(crate) fn canonical_place_joined_segments_may_overlap(
    program: &dyn ::validation::CheckingInput,
    prefix: &[facts::PlaceSegment],
    suffix: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    for (&left_segment, &right_segment) in prefix.iter().chain(suffix).zip(right) {
        if place_segment_has_unresolved_identity(left_segment)
            || place_segment_has_unresolved_identity(right_segment)
        {
            return true;
        }
        if !canonical_place_segment_pair_may_overlap(program, left_segment, right_segment) {
            return false;
        }
    }
    true
}

fn canonical_place_segment_pair_may_overlap(
    program: &dyn ::validation::CheckingInput,
    left: facts::PlaceSegment,
    right: facts::PlaceSegment,
) -> bool {
    static_place_segment_pair_equal(left, right).unwrap_or_else(|| match (left, right) {
        (facts::PlaceSegment::OpaqueRange, _) | (_, facts::PlaceSegment::OpaqueRange) => true,
        (
            facts::PlaceSegment::FixedRange {
                start: left_start,
                end: left_end,
            },
            facts::PlaceSegment::FixedRange {
                start: right_start,
                end: right_end,
            },
        ) => fixed_ranges_overlap(left_start, left_end, right_start, right_end),
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::FixedIndex { index },
        )
        | (
            facts::PlaceSegment::FixedIndex { index },
            facts::PlaceSegment::FixedRange { start, end },
        ) => fixed_range_contains(start, end, index),
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::Index { expression },
        )
        | (
            facts::PlaceSegment::Index { expression },
            facts::PlaceSegment::FixedRange { start, end },
        ) => expression_static_index(program, expression)
            .is_none_or(|index| fixed_range_contains(start, end, index)),
        (facts::PlaceSegment::FixedIndex { index }, facts::PlaceSegment::Index { expression })
        | (facts::PlaceSegment::Index { expression }, facts::PlaceSegment::FixedIndex { index }) => {
            expression_static_index(program, expression).is_none_or(|value| value == index)
        }
        (
            facts::PlaceSegment::Index {
                expression: left_expression,
            },
            facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => index_expressions_may_overlap(program, left_expression, right_expression),
        _ => false,
    })
}

fn fixed_range_contains(start: usize, end: usize, index: usize) -> bool {
    start < end && start <= index && index < end
}

fn fixed_ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < left_end
        && right_start < right_end
        && left_start < right_end
        && right_start < left_end
}

fn expression_static_index(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<usize> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
}

/// Decide whether two index expressions may select the same element.
///
/// Soundness requires defaulting to `true` (overlap) whenever we cannot prove the
/// indices are distinct, so a genuinely-overlapping or unknown-index mutation always
/// invalidates a dependent domain fact. We can only prove *disjointness* when both
/// sides are literal integers with different values; e.g. a domain fact over
/// `self.entries[0]` survives a mutation of `self.entries[1]`.
///
/// Dynamic indices (including repeated `self.index` reads that produce distinct
/// expression handles) are treated conservatively as possibly-overlapping. Disjoint
/// dynamic-indexed proofs are instead preserved by trailing-segment divergence — a
/// mutation of a different field of the same indexed element does not overlap the
/// fact's dependency path — which the joined-segment matcher handles independently of
/// this index comparison.
fn index_expressions_may_overlap(
    program: &dyn ::validation::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    // Cross-call frames may erase a selector handle while retaining its
    // collection dimension. The arena's dummy expression is not index zero.
    if !program.expression_table.expression_is_valid(left)
        || !program.expression_table.expression_is_valid(right)
    {
        return true;
    }
    if left == right {
        return true;
    }

    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (
            checked_trees::expression::ExpressionNode::Integer(left_value),
            checked_trees::expression::ExpressionNode::Integer(right_value),
        ) => left_value == right_value,
        _ => true,
    }
}
