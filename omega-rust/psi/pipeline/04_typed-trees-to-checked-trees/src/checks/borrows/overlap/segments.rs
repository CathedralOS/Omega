use super::indexes::{
    CompatibilityReplayDrift, EvaluatedIndexExtent, NormalizedBound, SelectorLocation,
    SelectorSessionClosure, SelectorSnapshotEvaluation, bound_equal, bound_is_at_or_before,
    bound_is_strictly_before, fixed_index_extent, fixed_range_extent,
    index_expression_extent_with_selectors, index_expressions_may_overlap_with_selectors,
    index_extents_may_overlap,
};
use super::premises::StatedOrderingPremise;
use crate::flow::place_segment_has_unresolved_identity;
use checked_trees::{
    BorrowCompatibilityPlaceSide, BorrowCompatibilityPremise, BorrowCompatibilitySelectorSnapshot,
    CapturedPlaceContainment,
};

/// One segment containment traversal. Evaluated `Index` extents are cached by
/// exact selector location so the equality check and both containment
/// directions observe each selector's bound through one recorded value.
struct SegmentContainmentEvaluation<'program, 'session, 'frozen> {
    program: &'program typed_trees::TypedTrees,
    selectors: &'session mut SelectorSnapshotEvaluation<'frozen>,
    extents: Vec<(SelectorLocation, EvaluatedIndexExtent)>,
}

impl SegmentContainmentEvaluation<'_, '_, '_> {
    fn index_extent(
        &mut self,
        expression: typed_trees::expression::ExpressionHandle,
        location: SelectorLocation,
    ) -> EvaluatedIndexExtent {
        if let Some((_, extent)) = self
            .extents
            .iter()
            .find(|(recorded, _)| *recorded == location)
        {
            return extent.clone();
        }
        let extent = index_expression_extent_with_selectors(
            self.program,
            expression,
            location,
            self.selectors,
        );
        self.extents.push((location, extent.clone()));
        extent
    }

    fn segment_location(
        side: BorrowCompatibilityPlaceSide,
        segment_index: usize,
    ) -> SelectorLocation {
        SelectorLocation {
            side,
            segment_index,
        }
    }

    /// Whether two segments at the same path position provably denote the same
    /// extent. Structural identities compare directly; `Index` selectors
    /// compare their normalized bounds.
    fn segments_equal(
        &mut self,
        left: facts::PlaceSegment,
        right: facts::PlaceSegment,
        segment_index: usize,
    ) -> bool {
        if structural_segment_equal(left, right) {
            return true;
        }
        let (
            facts::PlaceSegment::Index {
                expression: left_expression,
            },
            facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) = (left, right)
        else {
            return false;
        };
        let left_extent = self.index_extent(
            left_expression,
            Self::segment_location(BorrowCompatibilityPlaceSide::Forming, segment_index),
        );
        let right_extent = self.index_extent(
            right_expression,
            Self::segment_location(BorrowCompatibilityPlaceSide::Active, segment_index),
        );
        index_extents_equal(left_extent, right_extent, self.selectors)
    }

    /// Whether `container` is provably a prefix selector of `contained`:
    /// every container segment contains the segment at the same position.
    fn path_contains(
        &mut self,
        container: &[facts::PlaceSegment],
        container_side: BorrowCompatibilityPlaceSide,
        contained: &[facts::PlaceSegment],
        contained_side: BorrowCompatibilityPlaceSide,
    ) -> bool {
        container.len() <= contained.len()
            && container.iter().zip(contained).enumerate().all(
                |(segment_index, (container, contained))| {
                    self.segment_contains(
                        *container,
                        Self::segment_location(container_side, segment_index),
                        *contained,
                        Self::segment_location(contained_side, segment_index),
                    )
                },
            )
    }

    fn segment_contains(
        &mut self,
        container: facts::PlaceSegment,
        container_location: SelectorLocation,
        contained: facts::PlaceSegment,
        contained_location: SelectorLocation,
    ) -> bool {
        if structural_segment_equal(container, contained) {
            return true;
        }
        match (container, contained) {
            (
                facts::PlaceSegment::FixedRange { start, end },
                facts::PlaceSegment::FixedIndex { index },
            ) => start < end && start <= index && index < end,
            (
                facts::PlaceSegment::FixedRange {
                    start: outer_start,
                    end: outer_end,
                },
                facts::PlaceSegment::FixedRange {
                    start: inner_start,
                    end: inner_end,
                },
            ) => {
                outer_start < outer_end
                    && inner_start < inner_end
                    && outer_start <= inner_start
                    && inner_end <= outer_end
            }
            (
                facts::PlaceSegment::Index {
                    expression: container_expression,
                },
                facts::PlaceSegment::Index {
                    expression: contained_expression,
                },
            ) => {
                let container_extent = self.index_extent(container_expression, container_location);
                let contained_extent = self.index_extent(contained_expression, contained_location);
                index_extent_contains(container_extent, contained_extent, self.selectors)
            }
            (
                facts::PlaceSegment::Index {
                    expression: container_expression,
                },
                facts::PlaceSegment::FixedIndex { index },
            ) => {
                let Ok(index) = i64::try_from(index) else {
                    return false;
                };
                match self.index_extent(container_expression, container_location) {
                    EvaluatedIndexExtent::Window { start, end } => index_window_contains_point(
                        start,
                        end,
                        Some(NormalizedBound::Integer(index)),
                        self.selectors,
                    ),
                    EvaluatedIndexExtent::Point(point) => point.is_some_and(|point| {
                        bound_equal(point, NormalizedBound::Integer(index), self.selectors)
                    }),
                }
            }
            (
                facts::PlaceSegment::Index {
                    expression: container_expression,
                },
                facts::PlaceSegment::FixedRange { start, end },
            ) => {
                let (Ok(start), Ok(end)) = (i64::try_from(start), i64::try_from(end)) else {
                    return false;
                };
                match self.index_extent(container_expression, container_location) {
                    EvaluatedIndexExtent::Window {
                        start: container_start,
                        end: container_end,
                    } => {
                        start < end
                            && index_window_contains_window(
                                container_start,
                                container_end,
                                Some(NormalizedBound::Integer(start)),
                                Some(NormalizedBound::Integer(end)),
                                self.selectors,
                            )
                    }
                    // A single selected element cannot contain a window.
                    EvaluatedIndexExtent::Point(_) => false,
                }
            }
            (
                facts::PlaceSegment::FixedIndex { index },
                facts::PlaceSegment::Index {
                    expression: contained_expression,
                },
            ) => {
                let Ok(index) = i64::try_from(index) else {
                    return false;
                };
                match self.index_extent(contained_expression, contained_location) {
                    EvaluatedIndexExtent::Point(point) => point.is_some_and(|point| {
                        bound_equal(NormalizedBound::Integer(index), point, self.selectors)
                    }),
                    // A fixed element cannot contain a whole window.
                    EvaluatedIndexExtent::Window { .. } => false,
                }
            }
            (
                facts::PlaceSegment::FixedRange { start, end },
                facts::PlaceSegment::Index {
                    expression: contained_expression,
                },
            ) => {
                let (Ok(start), Ok(end)) = (i64::try_from(start), i64::try_from(end)) else {
                    return false;
                };
                match self.index_extent(contained_expression, contained_location) {
                    EvaluatedIndexExtent::Point(point) => {
                        matches!(point, Some(NormalizedBound::Integer(value)) if start < end && start <= value && value < end)
                    }
                    EvaluatedIndexExtent::Window {
                        start: contained_start,
                        end: contained_end,
                    } => index_window_contains_window(
                        Some(NormalizedBound::Integer(start)),
                        Some(NormalizedBound::Integer(end)),
                        contained_start,
                        contained_end,
                        self.selectors,
                    ),
                }
            }
            _ => false,
        }
    }
}

/// Whether one evaluated `Index` extent provably contains another: a window
/// contains a nested window or a bounded point, while a point only contains
/// an equal point. Unknown bounds stay unproven in both directions.
fn index_extent_contains(
    container: EvaluatedIndexExtent,
    contained: EvaluatedIndexExtent,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    match (container, contained) {
        (
            EvaluatedIndexExtent::Window {
                start: container_start,
                end: container_end,
            },
            EvaluatedIndexExtent::Window {
                start: contained_start,
                end: contained_end,
            },
        ) => index_window_contains_window(
            container_start,
            container_end,
            contained_start,
            contained_end,
            selectors,
        ),
        (EvaluatedIndexExtent::Window { start, end }, EvaluatedIndexExtent::Point(point)) => {
            index_window_contains_point(start, end, point, selectors)
        }
        (EvaluatedIndexExtent::Point(container), EvaluatedIndexExtent::Point(contained)) => {
            match (container, contained) {
                (Some(container), Some(contained)) => bound_equal(container, contained, selectors),
                _ => false,
            }
        }
        // A single selected element cannot contain a whole window.
        (EvaluatedIndexExtent::Point(_), EvaluatedIndexExtent::Window { .. }) => false,
    }
}

/// Evaluated-extent equality for `Index` segments: equal point bounds, or
/// pairwise-equal window bounds. Unlike containment, equality does not require
/// the window to be provably non-empty -- two equally unknown-empty windows
/// still denote the same extent.
fn index_extents_equal(
    left: EvaluatedIndexExtent,
    right: EvaluatedIndexExtent,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    match (left, right) {
        (EvaluatedIndexExtent::Point(left), EvaluatedIndexExtent::Point(right)) => {
            match (left, right) {
                (Some(left), Some(right)) => bound_equal(left, right, selectors),
                _ => false,
            }
        }
        (
            EvaluatedIndexExtent::Window {
                start: left_start,
                end: left_end,
            },
            EvaluatedIndexExtent::Window {
                start: right_start,
                end: right_end,
            },
        ) => match (left_start, left_end, right_start, right_end) {
            (Some(left_start), Some(left_end), Some(right_start), Some(right_end)) => {
                bound_equal(left_start, right_start, selectors)
                    && bound_equal(left_end, right_end, selectors)
            }
            _ => false,
        },
        _ => false,
    }
}

/// `[container_start, container_end)` contains `point`. All three bounds must
/// be known and ordered.
fn index_window_contains_point(
    container_start: Option<NormalizedBound>,
    container_end: Option<NormalizedBound>,
    point: Option<NormalizedBound>,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    match (container_start, container_end, point) {
        (Some(start), Some(end), Some(point)) => {
            bound_is_at_or_before(start, point.clone(), selectors)
                && bound_is_strictly_before(point, end, selectors)
        }
        _ => false,
    }
}

/// `[container_start, container_end)` contains `[contained_start,
/// contained_end)`. Mirroring the `FixedRange` rule, the contained window must
/// be provably non-empty and nest inside the container. The container's own
/// non-emptiness is then implied by the ordering chain.
fn index_window_contains_window(
    container_start: Option<NormalizedBound>,
    container_end: Option<NormalizedBound>,
    contained_start: Option<NormalizedBound>,
    contained_end: Option<NormalizedBound>,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    match (
        container_start,
        container_end,
        contained_start,
        contained_end,
    ) {
        (
            Some(container_start),
            Some(container_end),
            Some(contained_start),
            Some(contained_end),
        ) => {
            bound_is_strictly_before(contained_start.clone(), contained_end.clone(), selectors)
                && bound_is_at_or_before(container_start, contained_start, selectors)
                && bound_is_at_or_before(contained_end, container_end, selectors)
        }
        _ => false,
    }
}

fn place_segments_containment_evaluated(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> CapturedPlaceContainment {
    if left
        .iter()
        .chain(right)
        .any(|segment| place_segment_has_unresolved_identity(*segment))
    {
        return CapturedPlaceContainment::None;
    }
    let mut evaluation = SegmentContainmentEvaluation {
        program,
        selectors,
        extents: Vec::new(),
    };
    if left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .enumerate()
            .all(|(segment_index, (left, right))| {
                evaluation.segments_equal(*left, *right, segment_index)
            })
    {
        return CapturedPlaceContainment::Same;
    }
    if evaluation.path_contains(
        left,
        BorrowCompatibilityPlaceSide::Forming,
        right,
        BorrowCompatibilityPlaceSide::Active,
    ) {
        return CapturedPlaceContainment::LeftContainsRight;
    }
    if evaluation.path_contains(
        right,
        BorrowCompatibilityPlaceSide::Active,
        left,
        BorrowCompatibilityPlaceSide::Forming,
    ) {
        return CapturedPlaceContainment::RightContainsLeft;
    }
    CapturedPlaceContainment::None
}

fn structural_segment_equal(left: facts::PlaceSegment, right: facts::PlaceSegment) -> bool {
    match (left, right) {
        (
            facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            facts::PlaceSegment::Case {
                variant: left_variant,
            },
            facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => left_variant == right_variant,
        (
            facts::PlaceSegment::FixedIndex { index: left_index },
            facts::PlaceSegment::FixedIndex { index: right_index },
        ) => left_index == right_index,
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
        // `Index` selectors are compared through their evaluated extents in
        // `segments_equal`/`segment_contains`, which normalize runtime and
        // symbolic bounds through the selector session.
        _ => false,
    }
}

#[cfg(test)]
pub(super) fn place_segments_may_overlap(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let mut selectors = SelectorSnapshotEvaluation::capture(&[]);
    place_segments_may_overlap_evaluated(program, left, right, &mut selectors)
}

#[cfg(test)]
fn place_segments_containment(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> CapturedPlaceContainment {
    let mut selectors = SelectorSnapshotEvaluation::capture(&[]);
    place_segments_containment_evaluated(program, left, right, &mut selectors)
}

/// Runs the disjointness and containment judgments inside one selector
/// session so every normalized bound either judgment consumed is recorded in
/// the same snapshot. Containment only runs when the segments may overlap:
/// contained places overlap by construction, so a disjoint verdict already
/// fixes containment to `None` without evaluating more selectors.
pub(super) fn place_segments_compatibility_with_snapshot(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
    premises: &[StatedOrderingPremise],
) -> (bool, CapturedPlaceContainment, SelectorSessionClosure) {
    let mut selectors = SelectorSnapshotEvaluation::capture(premises);
    let may_overlap = place_segments_may_overlap_evaluated(program, left, right, &mut selectors);
    let containment = if may_overlap {
        place_segments_containment_evaluated(program, left, right, &mut selectors)
    } else {
        CapturedPlaceContainment::None
    };
    (
        may_overlap,
        containment,
        selectors
            .finish()
            .expect("captured selector evaluation is always complete"),
    )
}

/// Replays both judgments against the frozen snapshot. Every selector value
/// the capture consumed is re-derived and positionally verified; a missing,
/// reordered, or drifted row rejects the replay.
pub(super) fn place_segments_compatibility_from_snapshot(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
    snapshot: &[BorrowCompatibilitySelectorSnapshot],
    premises: &[StatedOrderingPremise],
    recorded_premises: &[BorrowCompatibilityPremise],
) -> Result<(bool, CapturedPlaceContainment), CompatibilityReplayDrift> {
    let mut selectors = SelectorSnapshotEvaluation::replay(snapshot, premises, recorded_premises);
    let may_overlap = place_segments_may_overlap_evaluated(program, left, right, &mut selectors);
    let containment = if may_overlap {
        place_segments_containment_evaluated(program, left, right, &mut selectors)
    } else {
        CapturedPlaceContainment::None
    };
    selectors.finish().map(|_| (may_overlap, containment))
}

fn place_segments_may_overlap_evaluated(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    for (segment_index, (&left_segment, &right_segment)) in left.iter().zip(right).enumerate() {
        // A known prefix may already have proved disjointness. Once nominal
        // identity is missing, later children cannot establish a divergence.
        if place_segment_has_unresolved_identity(left_segment)
            || place_segment_has_unresolved_identity(right_segment)
        {
            return true;
        }
        if !place_segment_pair_may_overlap(
            program,
            left_segment,
            right_segment,
            segment_index,
            selectors,
        ) {
            return false;
        }
    }
    true
}

fn place_segment_pair_may_overlap(
    program: &typed_trees::TypedTrees,
    left: facts::PlaceSegment,
    right: facts::PlaceSegment,
    segment_index: usize,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    let left_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Forming,
        segment_index,
    };
    let right_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Active,
        segment_index,
    };
    match (left, right) {
        (
            facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            facts::PlaceSegment::Case {
                variant: left_variant,
            },
            facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => left_variant == right_variant,
        (
            facts::PlaceSegment::FixedIndex { index: left_index },
            facts::PlaceSegment::FixedIndex { index: right_index },
        ) => left_index == right_index,
        (
            facts::PlaceSegment::FixedRange {
                start: left_start,
                end: left_end,
            },
            facts::PlaceSegment::FixedRange {
                start: right_start,
                end: right_end,
            },
        ) => {
            left_start < left_end
                && right_start < right_end
                && left_start < right_end
                && right_start < left_end
        }
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::FixedIndex { index },
        )
        | (
            facts::PlaceSegment::FixedIndex { index },
            facts::PlaceSegment::FixedRange { start, end },
        ) => start < end && start <= index && index < end,
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::Index { expression },
        ) => index_extents_may_overlap(
            fixed_range_extent(start, end),
            index_expression_extent_with_selectors(program, expression, right_location, selectors),
            selectors,
        ),
        (
            facts::PlaceSegment::Index { expression },
            facts::PlaceSegment::FixedRange { start, end },
        ) => index_extents_may_overlap(
            index_expression_extent_with_selectors(program, expression, left_location, selectors),
            fixed_range_extent(start, end),
            selectors,
        ),
        (facts::PlaceSegment::FixedIndex { index }, facts::PlaceSegment::Index { expression }) => {
            index_extents_may_overlap(
                fixed_index_extent(index),
                index_expression_extent_with_selectors(
                    program,
                    expression,
                    right_location,
                    selectors,
                ),
                selectors,
            )
        }
        (facts::PlaceSegment::Index { expression }, facts::PlaceSegment::FixedIndex { index }) => {
            index_extents_may_overlap(
                index_expression_extent_with_selectors(
                    program,
                    expression,
                    left_location,
                    selectors,
                ),
                fixed_index_extent(index),
                selectors,
            )
        }
        (
            facts::PlaceSegment::Index {
                expression: left_expression,
            },
            facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => index_expressions_may_overlap_with_selectors(
            program,
            left_expression,
            left_location,
            right_expression,
            right_location,
            selectors,
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
