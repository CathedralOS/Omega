use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
use psi_checked_trees::{
    BorrowCompatibilityPlaceSide, BorrowCompatibilitySelectorPosition,
    BorrowCompatibilitySelectorSnapshot, BorrowCompatibilitySelectorValue,
};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NormalizedBound {
    Integer(i64),
    Symbol(SymbolHandle),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SelectorLocation {
    pub side: BorrowCompatibilityPlaceSide,
    pub segment_index: usize,
}

pub(super) struct SelectorSnapshotEvaluation<'a> {
    frozen: Option<&'a [BorrowCompatibilitySelectorSnapshot]>,
    snapshot: Vec<BorrowCompatibilitySelectorSnapshot>,
    next_frozen: usize,
    invalid: bool,
}

impl SelectorSnapshotEvaluation<'_> {
    pub(super) fn capture() -> Self {
        Self {
            frozen: None,
            snapshot: Vec::new(),
            next_frozen: 0,
            invalid: false,
        }
    }

    pub(super) fn replay(
        snapshot: &[BorrowCompatibilitySelectorSnapshot],
    ) -> SelectorSnapshotEvaluation<'_> {
        SelectorSnapshotEvaluation {
            frozen: Some(snapshot),
            snapshot: Vec::new(),
            next_frozen: 0,
            invalid: false,
        }
    }

    pub(super) fn finish(self) -> Option<Vec<BorrowCompatibilitySelectorSnapshot>> {
        if self.invalid
            || self
                .frozen
                .is_some_and(|frozen| self.next_frozen != frozen.len())
        {
            None
        } else if let Some(frozen) = self.frozen {
            Some(frozen.to_vec())
        } else {
            Some(self.snapshot)
        }
    }

    fn bound(
        &mut self,
        location: SelectorLocation,
        position: BorrowCompatibilitySelectorPosition,
        current: impl FnOnce() -> Option<NormalizedBound>,
    ) -> Option<NormalizedBound> {
        if let Some(frozen) = self.frozen {
            let current = current();
            let Some(row) = frozen.get(self.next_frozen) else {
                self.invalid = true;
                return None;
            };
            if row.side != location.side
                || row.segment_index != location.segment_index
                || row.position != position
            {
                self.invalid = true;
                return None;
            }
            let current_value = current.map(|value| match value {
                NormalizedBound::Integer(value) => BorrowCompatibilitySelectorValue::Integer(value),
                NormalizedBound::Symbol(symbol) => BorrowCompatibilitySelectorValue::Symbol(symbol),
            });
            if row.value != current_value {
                self.invalid = true;
                return None;
            }
            self.next_frozen += 1;
            return match row.value {
                None => None,
                Some(BorrowCompatibilitySelectorValue::Integer(value)) => {
                    Some(NormalizedBound::Integer(value))
                }
                Some(BorrowCompatibilitySelectorValue::Symbol(symbol)) if symbol.is_valid() => {
                    Some(NormalizedBound::Symbol(symbol))
                }
                Some(BorrowCompatibilitySelectorValue::Symbol(_)) => {
                    self.invalid = true;
                    None
                }
            };
        }

        let value = current();
        self.snapshot.push(BorrowCompatibilitySelectorSnapshot {
            side: location.side,
            segment_index: location.segment_index,
            position,
            value: value.map(|value| match value {
                NormalizedBound::Integer(value) => BorrowCompatibilitySelectorValue::Integer(value),
                NormalizedBound::Symbol(symbol) => BorrowCompatibilitySelectorValue::Symbol(symbol),
            }),
        });
        value
    }
}

#[cfg(test)]
pub(super) fn index_expressions_may_overlap(
    program: &psi_typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let mut selectors = SelectorSnapshotEvaluation::capture();

    index_expressions_may_overlap_with_selectors(
        program,
        left,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index: 0,
        },
        right,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Active,
            segment_index: 0,
        },
        &mut selectors,
    )
}

pub(super) fn index_expressions_may_overlap_with_selectors(
    program: &psi_typed_trees::TypedTrees,
    left: ExpressionHandle,
    left_location: SelectorLocation,
    right: ExpressionHandle,
    right_location: SelectorLocation,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    if left == right {
        return true;
    }

    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Integer(left_value), ExpressionNode::Integer(right_value)) => {
            // Compare by VALUE through the i64 window; an oversize literal
            // conservatively MAY overlap (never claim disjointness on a
            // spelling difference -- 5 vs 0x5 must still alias).
            match (
                selectors.bound(
                    left_location,
                    BorrowCompatibilitySelectorPosition::Index,
                    || left_value.value_i64().map(NormalizedBound::Integer),
                ),
                selectors.bound(
                    right_location,
                    BorrowCompatibilitySelectorPosition::Index,
                    || right_value.value_i64().map(NormalizedBound::Integer),
                ),
            ) {
                (
                    Some(NormalizedBound::Integer(left_value)),
                    Some(NormalizedBound::Integer(right_value)),
                ) => left_value == right_value,
                _ => true,
            }
        }
        (ExpressionNode::Range(left_range), ExpressionNode::Integer(right_value)) => {
            match selectors.bound(
                right_location,
                BorrowCompatibilitySelectorPosition::Index,
                || right_value.value_i64().map(NormalizedBound::Integer),
            ) {
                Some(NormalizedBound::Integer(right_value)) => range_may_contain_integer(
                    program,
                    left_range,
                    left_location,
                    right_value,
                    selectors,
                ),
                None => true,
                Some(NormalizedBound::Symbol(_)) => true,
            }
        }
        (ExpressionNode::Integer(left_value), ExpressionNode::Range(right_range)) => {
            match selectors.bound(
                left_location,
                BorrowCompatibilitySelectorPosition::Index,
                || left_value.value_i64().map(NormalizedBound::Integer),
            ) {
                Some(NormalizedBound::Integer(left_value)) => range_may_contain_integer(
                    program,
                    right_range,
                    right_location,
                    left_value,
                    selectors,
                ),
                None => true,
                Some(NormalizedBound::Symbol(_)) => true,
            }
        }
        (ExpressionNode::Range(left_range), ExpressionNode::Range(right_range)) => {
            ranges_may_overlap(
                program,
                left_range,
                left_location,
                right_range,
                right_location,
                selectors,
            )
        }
        _ => true,
    }
}

#[cfg(test)]
pub(super) fn index_expression_may_contain_fixed(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    index: usize,
) -> bool {
    let mut selectors = SelectorSnapshotEvaluation::capture();
    index_expression_may_contain_fixed_with_selectors(
        program,
        expression,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index: 0,
        },
        index,
        &mut selectors,
    )
}

pub(super) fn index_expression_may_contain_fixed_with_selectors(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    location: SelectorLocation,
    index: usize,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    let Ok(index) = i64::try_from(index) else {
        return true;
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => selectors
            .bound(location, BorrowCompatibilitySelectorPosition::Index, || {
                value.value_i64().map(NormalizedBound::Integer)
            })
            .is_none_or(|value| matches!(value, NormalizedBound::Integer(value) if value == index)),
        ExpressionNode::Range(range) => {
            range_may_contain_integer(program, range, location, index, selectors)
        }
        _ => true,
    }
}

pub(super) fn index_expression_may_overlap_fixed_range_with_selectors(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    location: SelectorLocation,
    start: usize,
    end: usize,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    selectors
        .bound(location, BorrowCompatibilitySelectorPosition::Index, || {
            program
                .expression_table
                .constant_integer_value(expression)
                .map(NormalizedBound::Integer)
        })
        .and_then(|value| match value {
            NormalizedBound::Integer(value) => usize::try_from(value).ok(),
            NormalizedBound::Symbol(_) => None,
        })
        .is_none_or(|index| start < end && start <= index && index < end)
}

fn range_may_contain_integer(
    program: &psi_typed_trees::TypedTrees,
    range: &TableRangeExpression,
    location: SelectorLocation,
    value: i64,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    let (start, end) = range_integer_bounds(program, range, location, selectors);
    // An empty half-open window `[a, a)` contains nothing, so it is disjoint
    // from every index even when the index itself is unknown.
    if range_is_provably_empty(start, end) {
        return false;
    }
    if start.is_some_and(|start| matches!(start, NormalizedBound::Integer(start) if value < start))
    {
        return false;
    }
    if end.is_some_and(|end| matches!(end, NormalizedBound::Integer(end) if value >= end)) {
        return false;
    }
    true
}

fn ranges_may_overlap(
    program: &psi_typed_trees::TypedTrees,
    left: &TableRangeExpression,
    left_location: SelectorLocation,
    right: &TableRangeExpression,
    right_location: SelectorLocation,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    let (left_start, left_end) = range_integer_bounds(program, left, left_location, selectors);
    let (right_start, right_end) = range_integer_bounds(program, right, right_location, selectors);

    // Either window being provably empty makes the pair disjoint regardless of
    // the other window's bounds.
    if range_is_provably_empty(left_start, left_end)
        || range_is_provably_empty(right_start, right_end)
    {
        return false;
    }

    // Two half-open windows `[ls, le)` and `[rs, re)` are disjoint when one ends
    // at or before the other starts.
    if let (Some(left_end), Some(right_start)) = (left_end, right_start)
        && bound_is_at_or_before(left_end, right_start)
    {
        return false;
    }
    if let (Some(right_end), Some(left_start)) = (right_end, left_start)
        && bound_is_at_or_before(right_end, left_start)
    {
        return false;
    }
    true
}

/// A half-open window `[start, end)` with `end <= start` is empty and therefore
/// overlaps nothing. Exact symbolic identity proves the equality case without
/// claiming an order between distinct runtime values.
fn range_is_provably_empty(start: Option<NormalizedBound>, end: Option<NormalizedBound>) -> bool {
    matches!((start, end), (Some(start), Some(end)) if bound_is_at_or_before(end, start))
}

fn range_integer_bounds(
    program: &psi_typed_trees::TypedTrees,
    range: &TableRangeExpression,
    location: SelectorLocation,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> (Option<NormalizedBound>, Option<NormalizedBound>) {
    (
        selectors.bound(
            location,
            BorrowCompatibilitySelectorPosition::RangeStart,
            || normalized_bound(program, range.start),
        ),
        exclusive_end_bound(program, range, location, selectors),
    )
}

fn bound_is_at_or_before(left: NormalizedBound, right: NormalizedBound) -> bool {
    match (left, right) {
        (NormalizedBound::Integer(left), NormalizedBound::Integer(right)) => left <= right,
        (NormalizedBound::Symbol(left), NormalizedBound::Symbol(right)) => left == right,
        _ => false,
    }
}

/// The half-open (exclusive) upper bound of a range window.
///
/// All overlap reasoning here is in terms of half-open windows `[start, end)`.
/// An inclusive range `a..=b` covers index `b`, so its exclusive end is `b + 1`.
/// Normalizing here keeps `range_may_contain_integer`/`ranges_may_overlap` sound:
/// without it, `view[0..=3]` would be read as `[0, 3)` and a borrow of element 3
/// (or window `3..5`) would be mis-classified as disjoint.
///
/// A `b + 1` that overflows `i64` (the `..=i64::MAX` edge) cannot be represented
/// as an exclusive bound, so the end is reported as unknown (`None`), which the
/// overlap checks treat conservatively as possibly-overlapping.
fn exclusive_end_bound(
    program: &psi_typed_trees::TypedTrees,
    range: &TableRangeExpression,
    location: SelectorLocation,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> Option<NormalizedBound> {
    let end = selectors.bound(
        location,
        BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
        || {
            let end = normalized_bound(program, range.end)?;
            if range.end_inclusive {
                let NormalizedBound::Integer(end) = end else {
                    return None;
                };
                end.checked_add(1).map(NormalizedBound::Integer)
            } else {
                Some(end)
            }
        },
    );
    if range.end_inclusive && matches!(end, Some(NormalizedBound::Symbol(_))) {
        selectors.invalid = true;
        None
    } else {
        end
    }
}

fn normalized_bound(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    let expression =
        psi_validation::normalize_immutable_integer_bound_expression(program, expression)?;
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64().map(NormalizedBound::Integer),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 1 && path.symbol.is_valid() && path.head_symbol == path.symbol)
                .then_some(NormalizedBound::Symbol(path.symbol))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_typed_trees::expression::{
        BinaryOperator, Expression, NamePath, TableBinaryExpression,
    };
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::state::State;
    use psi_typed_trees::statement::{StatementNode, TableLocalData};

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn integer(program: &mut psi_typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(value),
        ))
    }

    fn range(
        program: &mut psi_typed_trees::TypedTrees,
        start: i64,
        end: i64,
        end_inclusive: bool,
    ) -> ExpressionHandle {
        let start = integer(program, start);
        let end = integer(program, end);
        program
            .expression_table
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end,
                end_inclusive,
            }))
    }

    fn named_bound(
        program: &mut psi_typed_trees::TypedTrees,
        name: &'static str,
        symbol: SymbolHandle,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated_static(name)],
                symbol,
                symbol,
            )))
    }

    fn range_bounds(
        program: &mut psi_typed_trees::TypedTrees,
        start: ExpressionHandle,
        end: ExpressionHandle,
        end_inclusive: bool,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end,
                end_inclusive,
            }))
    }

    fn install_locals(
        program: &mut psi_typed_trees::TypedTrees,
        locals: impl IntoIterator<Item = (SymbolHandle, &'static str, ExpressionHandle, bool)>,
    ) {
        let mut machine = Machine::default();
        let mut state = State::default();
        for (symbol, name, initial_value, is_mutable) in locals {
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol,
                    name: Identifier::generated_static(name),
                    initial_value,
                    is_mutable,
                    ..Default::default()
                }),
            );
        }
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
    }

    #[test]
    fn exclusive_range_disjoint_from_index_at_end() {
        let mut program = psi_typed_trees::TypedTrees::default();
        // `[0, 3)` does not contain index 3.
        let window = range(&mut program, 0, 3, false);
        let index = integer(&mut program, 3);
        assert!(!index_expressions_may_overlap(&program, window, index));
    }

    #[test]
    fn inclusive_range_overlaps_index_at_end() {
        let mut program = psi_typed_trees::TypedTrees::default();
        // `0..=3` covers index 3 -- must overlap (soundness).
        let window = range(&mut program, 0, 3, true);
        let index = integer(&mut program, 3);
        assert!(index_expressions_may_overlap(&program, window, index));
    }

    #[test]
    fn inclusive_range_overlaps_adjacent_window() {
        let mut program = psi_typed_trees::TypedTrees::default();
        // `0..=3` = `[0, 4)` overlaps `3..5` = `[3, 5)` at index 3.
        let left = range(&mut program, 0, 3, true);
        let right = range(&mut program, 3, 5, false);
        assert!(index_expressions_may_overlap(&program, left, right));
    }

    #[test]
    fn tail_range_excludes_fixed_index_before_its_start() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let one = integer(&mut program, 1);
        let tail = program
            .expression_table
            .insert(ExpressionNode::Range(TableRangeExpression {
                start: one,
                end: ExpressionHandle::invalid(),
                end_inclusive: false,
            }));

        assert!(!index_expression_may_contain_fixed(&program, tail, 0));
        assert!(index_expression_may_contain_fixed(&program, tail, 1));
    }

    #[test]
    fn exclusive_adjacent_windows_are_disjoint() {
        let mut program = psi_typed_trees::TypedTrees::default();
        // `[0, 3)` and `[3, 5)` share no index.
        let left = range(&mut program, 0, 3, false);
        let right = range(&mut program, 3, 5, false);
        assert!(!index_expressions_may_overlap(&program, left, right));
    }

    #[test]
    fn symbolic_exclusive_adjacency_requires_the_exact_resolved_boundary() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let zero = integer(&mut program, 0);
        let four = integer(&mut program, 4);
        let left_mid = named_bound(&mut program, "mid", symbol(1));
        let right_mid = named_bound(&mut program, "mid", symbol(1));
        let other = named_bound(&mut program, "other", symbol(2));
        let left = range_bounds(&mut program, zero, left_mid, false);
        let right = range_bounds(&mut program, right_mid, four, false);
        let mutated_right = range_bounds(&mut program, other, four, false);

        assert!(!index_expressions_may_overlap(&program, left, right));
        let left_place = psi_checked_trees::CapturedPlace {
            root_symbol: symbol(20),
            segments: vec![psi_facts::PlaceSegment::Index { expression: left }],
        };
        let right_place = psi_checked_trees::CapturedPlace {
            root_symbol: symbol(20),
            segments: vec![psi_facts::PlaceSegment::Index { expression: right }],
        };
        let compatibility = super::super::captured_place_compatibility(
            &program,
            &left_place,
            &psi_checked_trees::BorrowAccessKind::Mutable,
            &right_place,
            &psi_checked_trees::BorrowAccessKind::Mutable,
        );
        assert!(compatibility.disjoint);
        assert!(compatibility.non_interfering);
        assert_eq!(
            compatibility.containment,
            psi_checked_trees::CapturedPlaceContainment::None
        );
        assert!(
            index_expressions_may_overlap(&program, left, mutated_right),
            "changing the shared boundary symbol must restore conservative overlap"
        );
        let changed_place = psi_checked_trees::CapturedPlace {
            root_symbol: symbol(20),
            segments: vec![psi_facts::PlaceSegment::Index {
                expression: mutated_right,
            }],
        };
        let changed = super::super::captured_place_compatibility(
            &program,
            &left_place,
            &psi_checked_trees::BorrowAccessKind::Mutable,
            &changed_place,
            &psi_checked_trees::BorrowAccessKind::Mutable,
        );
        assert!(!changed.disjoint);
        assert!(!changed.non_interfering);
    }

    #[test]
    fn selector_snapshot_retains_exact_symbol_values_and_ordered_locations() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let zero = integer(&mut program, 0);
        let four = integer(&mut program, 4);
        let left_mid = named_bound(&mut program, "mid", symbol(1));
        let right_mid = named_bound(&mut program, "mid", symbol(1));
        let left = range_bounds(&mut program, zero, left_mid, false);
        let right = range_bounds(&mut program, right_mid, four, false);
        let left_location = SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index: 3,
        };
        let right_location = SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Active,
            segment_index: 5,
        };
        let mut capture = SelectorSnapshotEvaluation::capture();
        assert!(!index_expressions_may_overlap_with_selectors(
            &program,
            left,
            left_location,
            right,
            right_location,
            &mut capture,
        ));
        let snapshot = capture.finish().expect("closed captured snapshot");
        assert_eq!(
            snapshot
                .iter()
                .map(|row| (row.side, row.segment_index, row.position, row.value))
                .collect::<Vec<_>>(),
            vec![
                (
                    BorrowCompatibilityPlaceSide::Forming,
                    3,
                    BorrowCompatibilitySelectorPosition::RangeStart,
                    Some(BorrowCompatibilitySelectorValue::Integer(0)),
                ),
                (
                    BorrowCompatibilityPlaceSide::Forming,
                    3,
                    BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
                    Some(BorrowCompatibilitySelectorValue::Symbol(symbol(1))),
                ),
                (
                    BorrowCompatibilityPlaceSide::Active,
                    5,
                    BorrowCompatibilitySelectorPosition::RangeStart,
                    Some(BorrowCompatibilitySelectorValue::Symbol(symbol(1))),
                ),
                (
                    BorrowCompatibilityPlaceSide::Active,
                    5,
                    BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
                    Some(BorrowCompatibilitySelectorValue::Integer(4)),
                ),
            ]
        );

        let mut replay = SelectorSnapshotEvaluation::replay(&snapshot);
        assert!(!index_expressions_may_overlap_with_selectors(
            &program,
            left,
            left_location,
            right,
            right_location,
            &mut replay,
        ));
        assert_eq!(replay.finish(), Some(snapshot.clone()));

        let mut reordered = snapshot;
        reordered.swap(0, 1);
        let mut replay = SelectorSnapshotEvaluation::replay(&reordered);
        let _ = index_expressions_may_overlap_with_selectors(
            &program,
            left,
            left_location,
            right,
            right_location,
            &mut replay,
        );
        assert!(
            replay.finish().is_none(),
            "selector rows cannot be transposed across ordered path positions",
        );
    }

    #[test]
    fn unknown_selector_positions_close_replay_shape_without_positive_evidence() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let computed_left = named_bound(&mut program, "seed", symbol(8));
        let computed_right = integer(&mut program, 0);
        let computed_initial =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: computed_left,
                    operator: BinaryOperator::Add,
                    right: computed_right,
                }));
        install_locals(
            &mut program,
            [(symbol(9), "computed", computed_initial, false)],
        );
        let zero = integer(&mut program, 0);
        let four = integer(&mut program, 4);
        let left_end = named_bound(&mut program, "computed", symbol(9));
        let right_start = named_bound(&mut program, "computed", symbol(9));
        let left = range_bounds(&mut program, zero, left_end, false);
        let right = range_bounds(&mut program, right_start, four, false);
        let left_location = SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index: 0,
        };
        let right_location = SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Active,
            segment_index: 0,
        };
        let mut capture = SelectorSnapshotEvaluation::capture();
        assert!(index_expressions_may_overlap_with_selectors(
            &program,
            left,
            left_location,
            right,
            right_location,
            &mut capture,
        ));
        let snapshot = capture.finish().expect("closed unknown snapshot");
        assert_eq!(snapshot.len(), 4);
        assert_eq!(snapshot[1].value, None);
        assert_eq!(snapshot[2].value, None);

        let mut replay = SelectorSnapshotEvaluation::replay(&snapshot);
        assert!(index_expressions_may_overlap_with_selectors(
            &program,
            left,
            left_location,
            right,
            right_location,
            &mut replay,
        ));
        assert!(replay.finish().is_some());

        let mut incomplete = snapshot;
        incomplete.remove(1);
        let mut replay = SelectorSnapshotEvaluation::replay(&incomplete);
        let _ = index_expressions_may_overlap_with_selectors(
            &program,
            left,
            left_location,
            right,
            right_location,
            &mut replay,
        );
        assert!(
            replay.finish().is_none(),
            "omitting an unknown row must not look like an unobserved selector position",
        );
    }

    #[test]
    fn immutable_local_name_copy_chain_preserves_symbolic_adjacency() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let mid_initial = named_bound(&mut program, "mid", symbol(3));
        install_locals(&mut program, [(symbol(4), "cut", mid_initial, false)]);
        let zero = integer(&mut program, 0);
        let four = integer(&mut program, 4);
        let cut = named_bound(&mut program, "cut", symbol(4));
        let mid = named_bound(&mut program, "mid", symbol(3));
        let left = range_bounds(&mut program, zero, cut, false);
        let right = range_bounds(&mut program, mid, four, false);

        assert!(!index_expressions_may_overlap(&program, left, right));
    }

    #[test]
    fn mutable_and_computed_local_aliases_do_not_prove_symbolic_adjacency() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let mutable_initial = named_bound(&mut program, "mid", symbol(5));
        let computed_left = named_bound(&mut program, "mid", symbol(5));
        let computed_right = integer(&mut program, 0);
        let computed_initial =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: computed_left,
                    operator: BinaryOperator::Add,
                    right: computed_right,
                }));
        install_locals(
            &mut program,
            [
                (symbol(6), "mutable_cut", mutable_initial, true),
                (symbol(7), "computed_cut", computed_initial, false),
            ],
        );
        let zero = integer(&mut program, 0);
        let four = integer(&mut program, 4);
        let mutable_cut = named_bound(&mut program, "mutable_cut", symbol(6));
        let computed_cut = named_bound(&mut program, "computed_cut", symbol(7));
        let first_mid = named_bound(&mut program, "mid", symbol(5));
        let second_mid = named_bound(&mut program, "mid", symbol(5));
        let mutable_left = range_bounds(&mut program, zero, mutable_cut, false);
        let mutable_right = range_bounds(&mut program, first_mid, four, false);
        let computed_left = range_bounds(&mut program, zero, computed_cut, false);
        let computed_right = range_bounds(&mut program, second_mid, four, false);

        assert!(index_expressions_may_overlap(
            &program,
            mutable_left,
            mutable_right
        ));
        assert!(index_expressions_may_overlap(
            &program,
            computed_left,
            computed_right
        ));
    }

    #[test]
    fn inclusive_symbolic_end_and_cyclic_aliases_remain_conservative() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let first_to_second = named_bound(&mut program, "second", symbol(9));
        let second_to_first = named_bound(&mut program, "first", symbol(8));
        install_locals(
            &mut program,
            [
                (symbol(8), "first", first_to_second, false),
                (symbol(9), "second", second_to_first, false),
            ],
        );
        let zero = integer(&mut program, 0);
        let four = integer(&mut program, 4);
        let inclusive_mid = named_bound(&mut program, "mid", symbol(10));
        let adjacent_mid = named_bound(&mut program, "mid", symbol(10));
        let first = named_bound(&mut program, "first", symbol(8));
        let second = named_bound(&mut program, "second", symbol(9));
        let inclusive_left = range_bounds(&mut program, zero, inclusive_mid, true);
        let adjacent_right = range_bounds(&mut program, adjacent_mid, four, false);
        let cyclic_left = range_bounds(&mut program, zero, first, false);
        let cyclic_right = range_bounds(&mut program, second, four, false);

        assert!(index_expressions_may_overlap(
            &program,
            inclusive_left,
            adjacent_right
        ));
        assert!(index_expressions_may_overlap(
            &program,
            cyclic_left,
            cyclic_right
        ));
    }

    #[test]
    fn disjoint_windows_stay_disjoint() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let left = range(&mut program, 0, 2, false);
        let right = range(&mut program, 4, 8, false);
        assert!(!index_expressions_may_overlap(&program, left, right));
    }

    #[test]
    fn empty_inclusive_window_overlaps_nothing() {
        let mut program = psi_typed_trees::TypedTrees::default();
        // `2..=1` normalizes to `[2, 2)` -- empty, disjoint from index 2.
        let window = range(&mut program, 2, 1, true);
        let index = integer(&mut program, 2);
        assert!(!index_expressions_may_overlap(&program, window, index));
    }
}
