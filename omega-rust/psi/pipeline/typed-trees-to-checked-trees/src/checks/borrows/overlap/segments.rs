use super::indexes::{
    CompatibilityReplayDrift, EvaluatedIndexExtent, NormalizedBound, SelectorLocation,
    SelectorSessionClosure, SelectorSnapshotEvaluation, bound_equal, bound_is_at_or_before,
    bound_is_strictly_before, index_expression_extent_with_selectors,
    index_expression_may_contain_fixed_with_selectors,
    index_expression_may_overlap_fixed_range_with_selectors,
    index_expressions_may_overlap_with_selectors,
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
            return *extent;
        }
        let extent = index_expression_extent_with_selectors(
            self.program,
            expression,
            location,
            self.selectors,
        );
        self.extents.push((location, extent));
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
            matches!((container, contained),
                (Some(container), Some(contained)) if bound_equal(container, contained, selectors))
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
            matches!((left, right), (Some(left), Some(right)) if bound_equal(left, right, selectors))
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
        ) => {
            matches!((left_start, left_end, right_start, right_end),
                (Some(left_start), Some(left_end), Some(right_start), Some(right_end))
                    if bound_equal(left_start, right_start, selectors)
                        && bound_equal(left_end, right_end, selectors))
        }
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
    matches!((container_start, container_end, point),
        (Some(start), Some(end), Some(point))
            if bound_is_at_or_before(start, point, selectors)
                && bound_is_strictly_before(point, end, selectors))
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
    matches!((container_start, container_end, contained_start, contained_end),
        (Some(container_start), Some(container_end), Some(contained_start), Some(contained_end))
            if bound_is_strictly_before(contained_start, contained_end, selectors)
                && bound_is_at_or_before(container_start, contained_start, selectors)
                && bound_is_at_or_before(contained_end, container_end, selectors))
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
        ) => index_expression_may_overlap_fixed_range_with_selectors(
            program,
            expression,
            right_location,
            start,
            end,
            selectors,
        ),
        (
            facts::PlaceSegment::Index { expression },
            facts::PlaceSegment::FixedRange { start, end },
        ) => index_expression_may_overlap_fixed_range_with_selectors(
            program,
            expression,
            left_location,
            start,
            end,
            selectors,
        ),
        (facts::PlaceSegment::FixedIndex { index }, facts::PlaceSegment::Index { expression }) => {
            index_expression_may_contain_fixed_with_selectors(
                program,
                expression,
                right_location,
                index,
                selectors,
            )
        }
        (facts::PlaceSegment::Index { expression }, facts::PlaceSegment::FixedIndex { index }) => {
            index_expression_may_contain_fixed_with_selectors(
                program,
                expression,
                left_location,
                index,
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
mod tests {
    use super::*;
    use checked_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    };
    use typed_trees::expression::{NamePath, TableRangeExpression};
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TableLocalData};

    fn symbol(index: u32) -> symbols::SymbolHandle {
        symbols::SymbolHandle::from_arena_index(index)
    }

    fn integer_expression(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(value),
        ))
    }

    fn named_bound(
        program: &mut typed_trees::TypedTrees,
        name: &'static str,
        symbol: symbols::SymbolHandle,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert_tree(&typed_trees::expression::Expression::Name(
                NamePath::resolved(vec![Identifier::generated_static(name)], symbol, symbol),
            ))
    }

    fn install_locals(
        program: &mut typed_trees::TypedTrees,
        locals: impl IntoIterator<Item = (symbols::SymbolHandle, &'static str, ExpressionHandle, bool)>,
    ) {
        let type_reference =
            program
                .type_reference_table
                .insert(typed_trees::types::TypeReferenceNode::Named {
                    symbol: symbols::SymbolHandle::invalid(),
                    name: Identifier::generated_static("u64"),
                });
        let mut machine = Machine::default();
        let mut state = State::default();
        for (symbol, name, initial_value, is_mutable) in locals {
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol,
                    name: Identifier::generated_static(name),
                    type_reference,
                    initial_value,
                    is_mutable,
                    ..Default::default()
                }),
            );
        }
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
    }

    fn offset_bound(
        program: &mut typed_trees::TypedTrees,
        base: ExpressionHandle,
        operator: BinaryOperator,
        offset: i64,
    ) -> ExpressionHandle {
        let literal = integer_expression(program, offset);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: base,
                operator,
                right: literal,
            }))
    }

    fn range_bounds(
        program: &mut typed_trees::TypedTrees,
        start: ExpressionHandle,
        end: ExpressionHandle,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end,
                end_inclusive: false,
            }))
    }

    #[test]
    fn unresolved_field_does_not_prove_disjointness() {
        let program = typed_trees::TypedTrees::default();
        let unresolved = facts::PlaceSegment::default();
        let resolved = facts::PlaceSegment::Field {
            symbol: symbols::SymbolHandle::from_arena_index(1),
        };
        assert!(place_segments_may_overlap(
            &program,
            &[unresolved],
            &[resolved]
        ));
    }

    #[test]
    fn unresolved_field_does_not_prove_containment() {
        let program = typed_trees::TypedTrees::default();
        let unresolved = facts::PlaceSegment::default();
        assert_eq!(
            place_segments_containment(&program, &[unresolved], &[unresolved]),
            CapturedPlaceContainment::None
        );
    }

    #[test]
    fn fixed_indices_overlap_only_the_same_element() {
        let program = typed_trees::TypedTrees::default();

        assert!(place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::FixedIndex { index: 1 }],
        ));
    }

    #[test]
    fn fixed_and_legacy_literal_indices_compare_by_value() {
        let mut program = typed_trees::TypedTrees::default();
        let zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);

        assert!(place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::Index { expression: zero }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::Index { expression: one }],
        ));
    }

    #[test]
    fn fixed_range_preserves_pure_constant_index_folding() {
        let mut program = typed_trees::TypedTrees::default();
        let one = integer_expression(&mut program, 1);
        let two = integer_expression(&mut program, 2);
        let three =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: one,
                    operator: BinaryOperator::Add,
                    right: two,
                }));

        assert!(!place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedRange { start: 0, end: 3 }],
            &[facts::PlaceSegment::Index { expression: three }],
        ));
        assert!(place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
            &[facts::PlaceSegment::Index { expression: three }],
        ));
    }

    #[test]
    fn fixed_index_before_tail_range_is_disjoint() {
        let mut program = typed_trees::TypedTrees::default();
        let one = integer_expression(&mut program, 1);
        let tail = program.expression_table.insert(ExpressionNode::Range(
            typed_trees::expression::TableRangeExpression {
                start: one,
                end: ExpressionHandle::invalid(),
                end_inclusive: false,
            },
        ));

        assert!(!place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::Index { expression: tail }],
        ));
        assert!(place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 1 }],
            &[facts::PlaceSegment::Index { expression: tail }],
        ));
    }

    #[test]
    fn fixed_ranges_use_half_open_overlap() {
        let program = typed_trees::TypedTrees::default();
        let range = |start, end| facts::PlaceSegment::FixedRange { start, end };

        assert!(place_segments_may_overlap(
            &program,
            &[range(0, 2)],
            &[range(1, 3)],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[range(0, 2)],
            &[range(2, 4)],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[range(1, 1)],
            &[range(0, 2)],
        ));
    }

    #[test]
    fn fixed_structural_containment_is_directional() {
        let program = typed_trees::TypedTrees::default();
        let range = |start, end| facts::PlaceSegment::FixedRange { start, end };
        let index = |index| facts::PlaceSegment::FixedIndex { index };

        assert_eq!(
            place_segments_containment(&program, &[range(0, 4)], &[index(2)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(&program, &[index(2)], &[range(0, 4)]),
            CapturedPlaceContainment::RightContainsLeft
        );
        assert_eq!(
            place_segments_containment(&program, &[range(0, 8)], &[range(2, 4)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(&program, &[range(0, 4)], &[range(4, 8)]),
            CapturedPlaceContainment::None
        );
    }

    #[test]
    fn immutable_copy_chain_proves_same_dynamic_extent() {
        let mut program = typed_trees::TypedTrees::default();
        // `cut` immutably copies the `mid` boundary; both selectors therefore
        // normalize to the same `mid` symbol.
        let cut_initial = named_bound(&mut program, "mid", symbol(3));
        install_locals(&mut program, [(symbol(4), "cut", cut_initial, false)]);
        let mid = named_bound(&mut program, "mid", symbol(3));
        let cut = named_bound(&mut program, "cut", symbol(4));
        let other = named_bound(&mut program, "other", symbol(5));
        let index = |expression| facts::PlaceSegment::Index { expression };

        assert_eq!(
            place_segments_containment(&program, &[index(cut)], &[index(mid)]),
            CapturedPlaceContainment::Same
        );
        assert_eq!(
            place_segments_containment(&program, &[index(cut)], &[index(other)]),
            CapturedPlaceContainment::None
        );
    }

    #[test]
    fn shared_symbol_offsets_prove_window_containment() {
        let mut program = typed_trees::TypedTrees::default();
        // `mid` is an immutable binding whose initializer is computed, so its
        // name and `mid ± k` offsets normalize to the same symbol identity.
        let one = integer_expression(&mut program, 1);
        let mid_initial =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: one,
                    operator: BinaryOperator::Add,
                    right: one,
                }));
        install_locals(&mut program, [(symbol(3), "mid", mid_initial, false)]);
        let mid = named_bound(&mut program, "mid", symbol(3));
        let other = named_bound(&mut program, "other", symbol(5));
        let before_mid = offset_bound(&mut program, mid, BinaryOperator::Subtract, 1);
        let after_mid = offset_bound(&mut program, mid, BinaryOperator::Add, 2);
        let at_mid_end = offset_bound(&mut program, mid, BinaryOperator::Add, 1);
        let outer = range_bounds(&mut program, before_mid, after_mid);
        let inner = range_bounds(&mut program, mid, at_mid_end);
        let index = |expression| facts::PlaceSegment::Index { expression };

        // `[mid - 1, mid + 2)` provably contains `[mid, mid + 1)` and the
        // `mid` element, in either direction along the path.
        assert_eq!(
            place_segments_containment(&program, &[index(outer)], &[index(inner)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(&program, &[index(inner)], &[index(outer)]),
            CapturedPlaceContainment::RightContainsLeft
        );
        assert_eq!(
            place_segments_containment(&program, &[index(outer)], &[index(mid)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        // Distinct symbols remain unordered: no containment either way.
        assert_eq!(
            place_segments_containment(&program, &[index(outer)], &[index(other)]),
            CapturedPlaceContainment::None
        );
        // The point sits inside the window, so containment names the window
        // side: a bare point never contains a window.
        assert_eq!(
            place_segments_containment(&program, &[index(mid)], &[index(outer)]),
            CapturedPlaceContainment::RightContainsLeft
        );
        let before_other = offset_bound(&mut program, other, BinaryOperator::Subtract, 1);
        let after_other = offset_bound(&mut program, other, BinaryOperator::Add, 1);
        let other_window = range_bounds(&mut program, before_other, after_other);
        assert_eq!(
            place_segments_containment(&program, &[index(mid)], &[index(other_window)]),
            CapturedPlaceContainment::None
        );
    }

    #[test]
    fn literal_window_expressions_prove_containment_and_equality() {
        let mut program = typed_trees::TypedTrees::default();
        let zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);
        let three = integer_expression(&mut program, 3);
        let four = integer_expression(&mut program, 4);
        let outer = range_bounds(&mut program, zero, four);
        let inner = range_bounds(&mut program, one, three);
        let index = |expression| facts::PlaceSegment::Index { expression };

        assert_eq!(
            place_segments_containment(&program, &[index(outer)], &[index(inner)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(&program, &[index(outer)], &[index(three)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(&program, &[index(outer)], &[index(four)]),
            CapturedPlaceContainment::None
        );
        // Literal windows inside `FixedRange` selectors compare by value too.
        assert_eq!(
            place_segments_containment(
                &program,
                &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
                &[index(inner)]
            ),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(
                &program,
                &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
                &[index(three)]
            ),
            CapturedPlaceContainment::LeftContainsRight
        );
    }

    #[test]
    fn containment_bounds_survive_the_selector_snapshot_round_trip() {
        let mut program = typed_trees::TypedTrees::default();
        let cut_initial = named_bound(&mut program, "mid", symbol(3));
        install_locals(&mut program, [(symbol(4), "cut", cut_initial, false)]);
        let mid = named_bound(&mut program, "mid", symbol(3));
        let cut = named_bound(&mut program, "cut", symbol(4));
        let index = |expression| facts::PlaceSegment::Index { expression };
        let left = [index(cut)];
        let right = [index(mid)];

        let (may_overlap, containment, closure) =
            place_segments_compatibility_with_snapshot(&program, &left, &right, &[]);
        let snapshot = closure.snapshot;
        assert!(may_overlap);
        assert_eq!(containment, CapturedPlaceContainment::Same);
        assert_eq!(
            snapshot
                .iter()
                .map(|row| (row.side, row.segment_index, row.position))
                .collect::<Vec<_>>(),
            vec![
                (
                    BorrowCompatibilityPlaceSide::Forming,
                    0,
                    checked_trees::BorrowCompatibilitySelectorPosition::Index,
                ),
                (
                    BorrowCompatibilityPlaceSide::Active,
                    0,
                    checked_trees::BorrowCompatibilitySelectorPosition::Index,
                ),
            ],
            "both point bounds are frozen at their exact selector positions"
        );
        assert_eq!(
            place_segments_compatibility_from_snapshot(
                &program,
                &left,
                &right,
                &snapshot,
                &[],
                &[]
            ),
            Ok((may_overlap, containment))
        );

        // A dropped or reordered row cannot replay the same verdicts.
        assert_eq!(
            place_segments_compatibility_from_snapshot(
                &program,
                &left,
                &right,
                &snapshot[..1],
                &[],
                &[],
            ),
            Err(CompatibilityReplayDrift::SelectorSnapshot)
        );
        let mut reordered = snapshot.clone();
        reordered.swap(0, 1);
        assert_eq!(
            place_segments_compatibility_from_snapshot(
                &program,
                &left,
                &right,
                &reordered,
                &[],
                &[],
            ),
            Err(CompatibilityReplayDrift::SelectorSnapshot)
        );
    }
}
