use checked_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
use checked_trees::{
    BorrowCompatibilityPlaceSide, BorrowCompatibilityPremise, BorrowCompatibilityPremiseRelation,
    BorrowCompatibilitySelectorPosition, BorrowCompatibilitySelectorSnapshot,
    BorrowCompatibilitySelectorValue,
};
use symbols::SymbolHandle;

use super::premises::{StatedOrderingPremise, premise_proves};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NormalizedBound {
    Integer(i64),
    Symbol {
        symbol: SymbolHandle,
        offset: i64,
    },
    /// `first + second + offset` over two distinct immutable symbols in
    /// canonical arena order; `offset` may be zero because a two-symbol sum
    /// has no simpler spelling.
    SymbolSum {
        first: SymbolHandle,
        second: SymbolHandle,
        offset: i64,
    },
}

/// Record one normalized bound in the retained selector-value vocabulary.
/// Symbolic rows with a zero offset canonicalize to the plain `Symbol` form so
/// replay compares one spelling of each bound.
pub(super) fn selector_value(bound: NormalizedBound) -> BorrowCompatibilitySelectorValue {
    match bound {
        NormalizedBound::Integer(value) => BorrowCompatibilitySelectorValue::Integer(value),
        NormalizedBound::Symbol { symbol, offset: 0 } => {
            BorrowCompatibilitySelectorValue::Symbol(symbol)
        }
        NormalizedBound::Symbol { symbol, offset } => {
            BorrowCompatibilitySelectorValue::SymbolOffset { symbol, offset }
        }
        NormalizedBound::SymbolSum {
            first,
            second,
            offset,
        } => BorrowCompatibilitySelectorValue::SymbolSum {
            first,
            second,
            offset,
        },
    }
}

/// The term identity two bounds must share before their constant offsets can
/// order them: the integer line, one symbol's offset line, or one canonical
/// two-symbol sum. Distinct term sets stay unrelated, never negative
/// evidence.
fn bound_terms_equal(left: NormalizedBound, right: NormalizedBound) -> bool {
    match (left, right) {
        (NormalizedBound::Integer(_), NormalizedBound::Integer(_)) => true,
        (
            NormalizedBound::Symbol {
                symbol: left_symbol,
                ..
            },
            NormalizedBound::Symbol {
                symbol: right_symbol,
                ..
            },
        ) => left_symbol == right_symbol,
        (
            NormalizedBound::SymbolSum {
                first: left_first,
                second: left_second,
                ..
            },
            NormalizedBound::SymbolSum {
                first: right_first,
                second: right_second,
                ..
            },
        ) => left_first == right_first && left_second == right_second,
        _ => false,
    }
}

/// A bound's constant coordinate on its term line, when it has one.
fn bound_offset(bound: NormalizedBound) -> Option<i64> {
    match bound {
        NormalizedBound::Integer(value) => Some(value),
        NormalizedBound::Symbol { offset, .. } => Some(offset),
        NormalizedBound::SymbolSum { offset, .. } => Some(offset),
    }
}

/// Which recorded ledger an independent replay could not reproduce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityReplayDrift {
    /// A frozen selector row drifted from its re-derived value or position.
    SelectorSnapshot,
    /// A recorded premise token is missing, reordered, or no longer
    /// re-derives from the formation scope's stated contracts.
    Premise,
}

/// The normalized extent of one `Index` segment expression as evaluated inside
/// a selector session. A non-range expression is a single point bound; a range
/// expression is a half-open `[start, end)` window. `None` bounds are
/// conservatively unknown, never negative evidence.
#[derive(Debug, Clone, Copy)]
pub(super) enum EvaluatedIndexExtent {
    Point(Option<NormalizedBound>),
    Window {
        start: Option<NormalizedBound>,
        end: Option<NormalizedBound>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectorLocation {
    pub side: BorrowCompatibilityPlaceSide,
    pub segment_index: usize,
}

/// The closed replay ledger of one selector session: every normalized bound
/// row the judgments recorded or consumed, plus every stated premise token in
/// consult order.
pub struct SelectorSessionClosure {
    pub snapshot: Vec<BorrowCompatibilitySelectorSnapshot>,
    pub premises: Vec<BorrowCompatibilityPremise>,
}

pub(super) struct SelectorSnapshotEvaluation<'a> {
    frozen: Option<&'a [BorrowCompatibilitySelectorSnapshot]>,
    snapshot: Vec<BorrowCompatibilitySelectorSnapshot>,
    next_frozen: usize,
    /// The first drift observed while replaying either recorded ledger.
    /// Capture sessions never set it.
    drift: Option<CompatibilityReplayDrift>,
    /// Values already produced inside this session, keyed by exact selector
    /// position. Every selector position has exactly one producer -- the
    /// extent evaluator normalizes the same segment's expression at a
    /// `RangeStart`/`RangeExclusiveEnd`/`Index` coordinate -- so a repeated
    /// consult reuses the recorded row as the canonical evidence for both
    /// the overlap and containment judgments instead of recording a
    /// duplicate.
    recorded: Vec<(
        SelectorLocation,
        BorrowCompatibilitySelectorPosition,
        Option<NormalizedBound>,
    )>,
    /// Ordering premises the formation scope's stated contracts make
    /// available to this judgment. Consults are re-derived during replay, not
    /// trusted from the certificate.
    premises: &'a [StatedOrderingPremise],
    /// Premise tokens this capture consumed, in consult order.
    used_premises: Vec<BorrowCompatibilityPremise>,
    /// The recorded premise ledger a replay must reproduce positionally.
    frozen_premises: Option<&'a [BorrowCompatibilityPremise]>,
    next_frozen_premise: usize,
}

impl<'a> SelectorSnapshotEvaluation<'a> {
    pub(super) fn capture(premises: &'a [StatedOrderingPremise]) -> Self {
        Self {
            frozen: None,
            snapshot: Vec::new(),
            next_frozen: 0,
            drift: None,
            recorded: Vec::new(),
            premises,
            used_premises: Vec::new(),
            frozen_premises: None,
            next_frozen_premise: 0,
        }
    }

    pub(super) fn replay(
        snapshot: &'a [BorrowCompatibilitySelectorSnapshot],
        premises: &'a [StatedOrderingPremise],
        frozen_premises: &'a [BorrowCompatibilityPremise],
    ) -> SelectorSnapshotEvaluation<'a> {
        SelectorSnapshotEvaluation {
            frozen: Some(snapshot),
            snapshot: Vec::new(),
            next_frozen: 0,
            drift: None,
            recorded: Vec::new(),
            premises,
            used_premises: Vec::new(),
            frozen_premises: Some(frozen_premises),
            next_frozen_premise: 0,
        }
    }

    /// Close the session. Replay succeeds only when every recorded selector
    /// row and every recorded premise token was consumed exactly once.
    pub(super) fn finish(self) -> Result<SelectorSessionClosure, CompatibilityReplayDrift> {
        if let Some(drift) = self.drift {
            return Err(drift);
        }
        if self
            .frozen
            .is_some_and(|frozen| self.next_frozen != frozen.len())
        {
            return Err(CompatibilityReplayDrift::SelectorSnapshot);
        }
        if self
            .frozen_premises
            .is_some_and(|frozen| self.next_frozen_premise != frozen.len())
        {
            return Err(CompatibilityReplayDrift::Premise);
        }
        Ok(SelectorSessionClosure {
            snapshot: self.frozen.map_or(self.snapshot, |frozen| frozen.to_vec()),
            premises: self
                .frozen_premises
                .map_or(self.used_premises, |frozen| frozen.to_vec()),
        })
    }

    /// Record the first replay drift observed; later failures preserve the
    /// earliest diagnostic so it names the first unreproducible evidence.
    fn mark_drift(&mut self, drift: CompatibilityReplayDrift) {
        if self.drift.is_none() {
            self.drift = Some(drift);
        }
    }

    /// Consult the formation scope's stated ordering premises for one bound
    /// relation the structural order could not prove. The first premise that
    /// proves the query is recorded (capture) or matched positionally against
    /// the recorded ledger (replay); a consult the recorded ledger does not
    /// reproduce marks the session drifted and stays unproven.
    fn prove_ordering(
        &mut self,
        left: NormalizedBound,
        relation: BorrowCompatibilityPremiseRelation,
        right: NormalizedBound,
    ) -> bool {
        let Some(premise) = self
            .premises
            .iter()
            .find(|premise| premise_proves(premise, left, relation, right))
        else {
            return false;
        };
        let token = premise.token();
        if let Some(frozen) = self.frozen_premises {
            match frozen.get(self.next_frozen_premise) {
                Some(recorded) if *recorded == token => {
                    self.next_frozen_premise += 1;
                }
                _ => {
                    self.mark_drift(CompatibilityReplayDrift::Premise);
                    return false;
                }
            }
        } else {
            self.used_premises.push(token);
        }
        true
    }

    fn bound(
        &mut self,
        location: SelectorLocation,
        position: BorrowCompatibilitySelectorPosition,
        current: impl FnOnce() -> Option<NormalizedBound>,
    ) -> Option<NormalizedBound> {
        if let Some((_, _, value)) =
            self.recorded
                .iter()
                .find(|(recorded, recorded_position, _)| {
                    *recorded == location && *recorded_position == position
                })
        {
            return *value;
        }
        if let Some(frozen) = self.frozen {
            let current = current();
            let Some(row) = frozen.get(self.next_frozen) else {
                self.mark_drift(CompatibilityReplayDrift::SelectorSnapshot);
                return None;
            };
            if row.side != location.side
                || row.segment_index != location.segment_index
                || row.position != position
            {
                self.mark_drift(CompatibilityReplayDrift::SelectorSnapshot);
                return None;
            }
            let current_value = current.map(selector_value);
            if row.value != current_value {
                self.mark_drift(CompatibilityReplayDrift::SelectorSnapshot);
                return None;
            }
            self.next_frozen += 1;
            let value = match row.value {
                None => None,
                Some(BorrowCompatibilitySelectorValue::Integer(value)) => {
                    Some(NormalizedBound::Integer(value))
                }
                Some(BorrowCompatibilitySelectorValue::Symbol(symbol)) if symbol.is_valid() => {
                    Some(NormalizedBound::Symbol { symbol, offset: 0 })
                }
                Some(BorrowCompatibilitySelectorValue::Symbol(_)) => {
                    self.mark_drift(CompatibilityReplayDrift::SelectorSnapshot);
                    None
                }
                Some(BorrowCompatibilitySelectorValue::SymbolOffset { symbol, offset })
                    if symbol.is_valid() && offset != 0 =>
                {
                    Some(NormalizedBound::Symbol { symbol, offset })
                }
                Some(BorrowCompatibilitySelectorValue::SymbolOffset { .. }) => {
                    self.mark_drift(CompatibilityReplayDrift::SelectorSnapshot);
                    None
                }
                Some(BorrowCompatibilitySelectorValue::SymbolSum {
                    first,
                    second,
                    offset,
                }) if first.is_valid()
                    && second.is_valid()
                    && (first.arena_index(), first.generation())
                        < (second.arena_index(), second.generation()) =>
                {
                    Some(NormalizedBound::SymbolSum {
                        first,
                        second,
                        offset,
                    })
                }
                Some(BorrowCompatibilitySelectorValue::SymbolSum { .. }) => {
                    self.mark_drift(CompatibilityReplayDrift::SelectorSnapshot);
                    None
                }
            };
            if self.drift.is_none() {
                self.recorded.push((location, position, value));
            }
            return value;
        }

        let value = current();
        self.snapshot.push(BorrowCompatibilitySelectorSnapshot {
            side: location.side,
            segment_index: location.segment_index,
            position,
            value: value.map(selector_value),
        });
        self.recorded.push((location, position, value));
        value
    }
}

#[cfg(test)]
pub(super) fn index_expressions_may_overlap(
    program: &typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let mut selectors = SelectorSnapshotEvaluation::capture(&[]);

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

/// Whether two `Index` segment expressions may select the same element.
///
/// Both expressions are evaluated to their normalized extent inside the
/// selector session -- a point bound or a half-open window -- and compared by
/// the shared bound-ordering layer, so symbolic points order against window
/// bounds the same way window bounds order against each other. Unknown bounds
/// stay conservatively overlapping. Disjointness needs ordering or singleton
/// disequality, and a stated premise is consulted only when structural
/// relations cannot settle it.
pub(super) fn index_expressions_may_overlap_with_selectors(
    program: &typed_trees::TypedTrees,
    left: ExpressionHandle,
    left_location: SelectorLocation,
    right: ExpressionHandle,
    right_location: SelectorLocation,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    if left == right {
        return true;
    }
    let left_extent =
        index_expression_extent_with_selectors(program, left, left_location, selectors);
    let right_extent =
        index_expression_extent_with_selectors(program, right, right_location, selectors);
    index_extents_may_overlap(left_extent, right_extent, selectors)
}

/// Whether two evaluated `Index` extents may select the same element. Two
/// points are disjoint when strict ordering or disequality separates them; a
/// point and a window are disjoint only when the point provably sits outside
/// or the window is provably empty; two windows are disjoint when one ends at
/// or before the other starts. Every unproven ordering stays overlapping.
pub(super) fn index_extents_may_overlap(
    left: EvaluatedIndexExtent,
    right: EvaluatedIndexExtent,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    match (left, right) {
        (EvaluatedIndexExtent::Point(Some(left)), EvaluatedIndexExtent::Point(Some(right))) => {
            !bound_is_strictly_before(left, right, selectors)
                && !bound_is_strictly_before(right, left, selectors)
                && !selectors.prove_ordering(
                    left,
                    BorrowCompatibilityPremiseRelation::NotEqual,
                    right,
                )
        }
        (EvaluatedIndexExtent::Point(_), EvaluatedIndexExtent::Point(_)) => true,
        (EvaluatedIndexExtent::Window { start, end }, EvaluatedIndexExtent::Point(point))
        | (EvaluatedIndexExtent::Point(point), EvaluatedIndexExtent::Window { start, end }) => {
            index_window_may_contain_point(start, end, point, selectors)
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
        ) => index_windows_may_overlap(left_start, left_end, right_start, right_end, selectors),
    }
}

#[cfg(test)]
pub(super) fn index_expression_may_contain_fixed(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    index: usize,
) -> bool {
    let mut selectors = SelectorSnapshotEvaluation::capture(&[]);
    let extent = index_expression_extent_with_selectors(
        program,
        expression,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index: 0,
        },
        &mut selectors,
    );
    index_extents_may_overlap(extent, fixed_index_extent(index), &mut selectors)
}

/// The point extent of a fixed literal index segment. An index above the i64
/// window stays unknown rather than borrowing a different bound's identity.
pub(super) fn fixed_index_extent(index: usize) -> EvaluatedIndexExtent {
    EvaluatedIndexExtent::Point(i64::try_from(index).ok().map(NormalizedBound::Integer))
}

/// The half-open window extent of a fixed literal range segment.
pub(super) fn fixed_range_extent(start: usize, end: usize) -> EvaluatedIndexExtent {
    EvaluatedIndexExtent::Window {
        start: i64::try_from(start).ok().map(NormalizedBound::Integer),
        end: i64::try_from(end).ok().map(NormalizedBound::Integer),
    }
}

/// `[start, end)` may contain `point` unless the point provably precedes the
/// window start, sits at or beyond the window end, or the window is provably
/// empty. An unknown point or an unordered bound pair stays contained --
/// disjointness needs positive evidence.
fn index_window_may_contain_point(
    start: Option<NormalizedBound>,
    end: Option<NormalizedBound>,
    point: Option<NormalizedBound>,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    if index_window_provably_empty(start, end, selectors) {
        return false;
    }
    let Some(point) = point else {
        return true;
    };
    if start.is_some_and(|start| bound_is_strictly_before(point, start, selectors)) {
        return false;
    }
    if end.is_some_and(|end| bound_is_at_or_before(end, point, selectors)) {
        return false;
    }
    true
}

/// Two half-open windows `[ls, le)` and `[rs, re)` may overlap unless one is
/// provably empty or one provably ends at or before the other starts.
fn index_windows_may_overlap(
    left_start: Option<NormalizedBound>,
    left_end: Option<NormalizedBound>,
    right_start: Option<NormalizedBound>,
    right_end: Option<NormalizedBound>,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    if index_window_provably_empty(left_start, left_end, selectors)
        || index_window_provably_empty(right_start, right_end, selectors)
    {
        return false;
    }

    if let (Some(left_end), Some(right_start)) = (left_end, right_start)
        && bound_is_at_or_before(left_end, right_start, selectors)
    {
        return false;
    }
    if let (Some(right_end), Some(left_start)) = (right_end, left_start)
        && bound_is_at_or_before(right_end, left_start, selectors)
    {
        return false;
    }
    true
}

/// A half-open window `[start, end)` with `end <= start` is empty and therefore
/// overlaps nothing. Shared-symbol offsets order as mathematical integers;
/// distinct runtime symbols remain unordered unless a stated premise orders
/// them.
fn index_window_provably_empty(
    start: Option<NormalizedBound>,
    end: Option<NormalizedBound>,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    matches!((start, end), (Some(start), Some(end)) if bound_is_at_or_before(end, start, selectors))
}

fn range_integer_bounds(
    program: &typed_trees::TypedTrees,
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

/// `left <= right` on normalized bounds. The structural order consults the
/// formation scope's stated premises only after its own same-symbol/integer
/// rule fails; a premise proves the relation but never widens it.
pub(super) fn bound_is_at_or_before(
    left: NormalizedBound,
    right: NormalizedBound,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    if structural_bound_is_at_or_before(left, right) {
        return true;
    }
    selectors.prove_ordering(left, BorrowCompatibilityPremiseRelation::LessOrEqual, right)
}

fn structural_bound_is_at_or_before(left: NormalizedBound, right: NormalizedBound) -> bool {
    bound_terms_equal(left, right)
        && match (bound_offset(left), bound_offset(right)) {
            (Some(left), Some(right)) => left <= right,
            _ => false,
        }
}

/// Strict `<` ordering for bounds; used where a window must be provably
/// non-empty or a point must sit strictly below an exclusive end.
pub(super) fn bound_is_strictly_before(
    left: NormalizedBound,
    right: NormalizedBound,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    if structural_bound_is_strictly_before(left, right) {
        return true;
    }
    selectors.prove_ordering(
        left,
        BorrowCompatibilityPremiseRelation::StrictlyBefore,
        right,
    )
}

fn structural_bound_is_strictly_before(left: NormalizedBound, right: NormalizedBound) -> bool {
    bound_terms_equal(left, right)
        && match (bound_offset(left), bound_offset(right)) {
            (Some(left), Some(right)) => left < right,
            _ => false,
        }
}

/// Exact bound equality: literal values, the same symbol at the same offset,
/// or a stated `==` premise that fixes both bounds to one offset line.
/// Anything else stays unproven rather than assumed distinct.
pub(super) fn bound_equal(
    left: NormalizedBound,
    right: NormalizedBound,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    if structural_bound_equal(left, right) {
        return true;
    }
    selectors.prove_ordering(left, BorrowCompatibilityPremiseRelation::Equal, right)
}

fn structural_bound_equal(left: NormalizedBound, right: NormalizedBound) -> bool {
    bound_terms_equal(left, right)
        && match (bound_offset(left), bound_offset(right)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
}

/// Evaluates an `Index` segment's normalized extent through the selector
/// session, recording or replaying its bounds at the segment's exact path
/// location. The bound positions match the overlap selectors: `Index` for a
/// point expression, `RangeStart`/`RangeExclusiveEnd` for a range window.
pub(super) fn index_expression_extent_with_selectors(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    location: SelectorLocation,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> EvaluatedIndexExtent {
    match program.expression_table.expression(expression) {
        ExpressionNode::Range(range) => {
            let (start, end) = range_integer_bounds(program, range, location, selectors);
            EvaluatedIndexExtent::Window { start, end }
        }
        _ => EvaluatedIndexExtent::Point(selectors.bound(
            location,
            BorrowCompatibilitySelectorPosition::Index,
            // A pure constant subtree folds to its exact integer before the
            // immutable-bound vocabulary gets a chance -- `1 + 2` denotes the
            // point 3 even though it normalizes to no symbol identity.
            || {
                program
                    .expression_table
                    .constant_integer_value(expression)
                    .map(NormalizedBound::Integer)
                    .or_else(|| normalized_bound(program, expression))
            },
        )),
    }
}

/// The half-open (exclusive) upper bound of a range window.
///
/// All overlap reasoning here is in terms of half-open windows `[start, end)`.
/// An inclusive range `a..=b` covers index `b`, so its exclusive end is `b + 1`.
/// Normalizing here keeps the window overlap and point-containment checks
/// sound: without it, `view[0..=3]` would be read as `[0, 3)` and a borrow of
/// element 3 (or window `3..5`) would be mis-classified as disjoint.
///
/// A `b + 1` that overflows `i64` (the `..=i64::MAX` edge), including a
/// symbolic offset, cannot be represented as an exclusive bound, so the end is
/// reported as unknown (`None`), which the overlap checks treat conservatively
/// as possibly-overlapping.
fn exclusive_end_bound(
    program: &typed_trees::TypedTrees,
    range: &TableRangeExpression,
    location: SelectorLocation,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> Option<NormalizedBound> {
    selectors.bound(
        location,
        BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
        || {
            let end = normalized_bound(program, range.end)?;
            if !range.end_inclusive {
                return Some(end);
            }
            match end {
                NormalizedBound::Integer(end) => end.checked_add(1).map(NormalizedBound::Integer),
                NormalizedBound::Symbol { symbol, offset } => offset
                    .checked_add(1)
                    .map(|offset| NormalizedBound::Symbol { symbol, offset }),
                NormalizedBound::SymbolSum {
                    first,
                    second,
                    offset,
                } => offset
                    .checked_add(1)
                    .map(|offset| NormalizedBound::SymbolSum {
                        first,
                        second,
                        offset,
                    }),
            }
        },
    )
}

pub(super) fn normalized_bound(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    if let Some(offset) = validation::immutable_integer_bound_symbol_offset(program, expression) {
        return Some(NormalizedBound::Symbol {
            symbol: offset.symbol,
            offset: offset.offset,
        });
    }
    if let Some(sum) = validation::immutable_integer_bound_sum(program, expression) {
        return Some(NormalizedBound::SymbolSum {
            first: sum.first,
            second: sum.second,
            offset: sum.offset,
        });
    }
    let Some(expression) =
        validation::normalize_immutable_integer_bound_expression(program, expression)
    else {
        return validation::immutable_integer_bound_value_symbol(program, expression)
            .map(|symbol| NormalizedBound::Symbol { symbol, offset: 0 });
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64().map(NormalizedBound::Integer),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 1 && path.symbol.is_valid() && path.head_symbol == path.symbol)
                .then_some(NormalizedBound::Symbol {
                    symbol: path.symbol,
                    offset: 0,
                })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
