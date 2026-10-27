use checked_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
use symbols::SymbolHandle;

use super::premises::{
    BorrowCompatibilityPremiseRelation, StatedOrderingPremise, premise_chain_proves, premise_proves,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BorrowCompatibilityPlaceSide {
    Forming,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BorrowCompatibilitySelectorPosition {
    Index,
    RangeStart,
    RangeExclusiveEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NormalizedBound {
    Integer(i64),
    Symbol {
        symbol: SymbolHandle,
        offset: i64,
    },
    /// The value currently stored under one mutable storage symbol. The
    /// storage name is a query coordinate, never positive evidence: storage
    /// bounds never order structurally (the storage may hold a different
    /// occurrence at each statement), so stated evidence can only claim them
    /// in a scope where the establishment binding is version-pinned.
    Storage {
        symbol: SymbolHandle,
    },
    /// The value at one projected place under an immutable storage symbol
    /// (`pair.first`, `pair.items[2]`): a frozen identity of the bound
    /// value's place, equal only to the same projection path of the same
    /// symbol. Projections whose path carries unresolved segments stay
    /// outside the vocabulary.
    Projected {
        symbol: SymbolHandle,
        segments: Vec<facts::PlaceSegment>,
    },
    /// The value currently stored under one projected place of mutable
    /// storage (`mut_pair.first`, `self.pivot[2]`). Like `Storage` it is a
    /// query coordinate, never positive evidence, and needs the same
    /// version-pin evidence before stated premises can claim it.
    StorageProjected {
        symbol: SymbolHandle,
        segments: Vec<facts::PlaceSegment>,
    },
    /// `first + second + offset` over two distinct immutable symbols in
    /// canonical arena order; `offset` may be zero because a two-symbol sum
    /// has no simpler spelling.
    SymbolSum {
        first: SymbolHandle,
        second: SymbolHandle,
        offset: i64,
    },
    /// The value produced by one exact call occurrence, named by its
    /// expression handle plus the resolved segment path projected through
    /// it (`choose(seed).first` carries `[Field{first}]`; a bare result
    /// carries the empty path). A call result carries no place identity,
    /// so a nested call's `ensures` binds the occurrence itself — the
    /// intra-statement fact context for guarantees produced and consumed
    /// within one statement. Equal only to the same occurrence at the same
    /// projected path; a call result never shifts or carries a storage
    /// coordinate.
    CallResult {
        expression: ExpressionHandle,
        segments: Vec<facts::PlaceSegment>,
    },
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
        (
            NormalizedBound::Projected {
                symbol: left_symbol,
                segments: left_segments,
            },
            NormalizedBound::Projected {
                symbol: right_symbol,
                segments: right_segments,
            },
        ) => left_symbol == right_symbol && left_segments == right_segments,
        (
            NormalizedBound::CallResult {
                expression: left_expression,
                segments: left_segments,
            },
            NormalizedBound::CallResult {
                expression: right_expression,
                segments: right_segments,
            },
        ) => left_expression == right_expression && left_segments == right_segments,
        _ => false,
    }
}

/// A bound's constant coordinate on its term line, when it has one.
fn bound_offset(bound: NormalizedBound) -> Option<i64> {
    match bound {
        NormalizedBound::Integer(value) => Some(value),
        NormalizedBound::Symbol { offset, .. } => Some(offset),
        NormalizedBound::Storage { .. } => Some(0),
        NormalizedBound::Projected { .. } | NormalizedBound::StorageProjected { .. } => Some(0),
        NormalizedBound::CallResult { .. } => Some(0),
        NormalizedBound::SymbolSum { offset, .. } => Some(offset),
    }
}

/// The normalized extent of one `Index` segment expression as evaluated inside
/// a selector session. A non-range expression is a single point bound; a range
/// expression is a half-open `[start, end)` window. `None` bounds are
/// conservatively unknown, never negative evidence.
#[derive(Debug, Clone)]
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

/// One comparison's bound cache. Overlap and containment share values, not
/// serialized snapshots. Mutable storage remains a query coordinate and can
/// only be ordered with premises whose availability was established at use.
pub(super) struct SelectorEvaluation<'a> {
    recorded: Vec<(
        SelectorLocation,
        BorrowCompatibilitySelectorPosition,
        Option<NormalizedBound>,
    )>,
    premises: &'a [StatedOrderingPremise],
}

impl<'a> SelectorEvaluation<'a> {
    pub(super) fn new(premises: &'a [StatedOrderingPremise]) -> Self {
        Self {
            recorded: Vec::new(),
            premises,
        }
    }

    /// Ask only the current scope's established integer relations. Exact
    /// equations may transport a query through completed results and joins;
    /// they never rewrite ambient premises or manufacture a storage version.
    fn prove_ordering(
        &mut self,
        left: NormalizedBound,
        relation: BorrowCompatibilityPremiseRelation,
        right: NormalizedBound,
    ) -> bool {
        // Reserve the whole former single/two-premise search unchanged, then
        // bound only the additional equality exploration. Exhaustion proves
        // nothing; it cannot turn a failed compatibility query into authority.
        const EQUALITY_CONSULTATIONS: usize = 4096;
        let mut remaining = self
            .premises
            .len()
            .saturating_mul(self.premises.len())
            .saturating_add(EQUALITY_CONSULTATIONS);
        if self.prove_from_premises(left.clone(), relation, right.clone(), &mut remaining) {
            return true;
        }
        let mut candidates = Vec::new();
        for candidate in self
            .premises
            .iter()
            .filter_map(StatedOrderingPremise::equality_candidates)
            .flatten()
        {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
        let mut pending = vec![(left, right)];
        let mut visited = Vec::new();
        while let Some((left, right)) = pending.pop() {
            if visited.contains(&(left.clone(), right.clone())) {
                continue;
            }
            visited.push((left.clone(), right.clone()));
            for candidate in &candidates {
                for (subject, next) in [
                    (&left, (candidate.clone(), right.clone())),
                    (&right, (left.clone(), candidate.clone())),
                ] {
                    if subject == candidate || visited.contains(&next) || pending.contains(&next) {
                        continue;
                    }
                    if !self.prove_from_premises(
                        subject.clone(),
                        BorrowCompatibilityPremiseRelation::Equal,
                        candidate.clone(),
                        &mut remaining,
                    ) {
                        if remaining == 0 {
                            return false;
                        }
                        continue;
                    }
                    if self.prove_from_premises(
                        next.0.clone(),
                        relation,
                        next.1.clone(),
                        &mut remaining,
                    ) {
                        return true;
                    }
                    if remaining == 0 {
                        return false;
                    }
                    pending.push(next);
                }
            }
        }
        false
    }

    fn prove_from_premises(
        &self,
        left: NormalizedBound,
        relation: BorrowCompatibilityPremiseRelation,
        right: NormalizedBound,
        remaining: &mut usize,
    ) -> bool {
        for premise in self.premises {
            if *remaining == 0 {
                return false;
            }
            *remaining -= 1;
            if premise_proves(premise, left.clone(), relation, right.clone()) {
                return true;
            }
        }
        // No single premise names both bounds; a two-premise chain through
        // one shared middle bound may still derive the relation.
        for first_index in 0..self.premises.len() {
            for second_index in 0..self.premises.len() {
                if first_index == second_index {
                    continue;
                }
                if *remaining == 0 {
                    return false;
                }
                *remaining -= 1;
                if premise_chain_proves(
                    &self.premises[first_index],
                    &self.premises[second_index],
                    left.clone(),
                    relation,
                    right.clone(),
                ) {
                    return true;
                }
            }
        }
        false
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
            return value.clone();
        }
        let value = current();
        self.recorded.push((location, position, value.clone()));
        value
    }
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
pub(super) fn index_expressions_may_overlap<'p>(
    program: &'p dyn ::validation::CheckingInput,
    left: ExpressionHandle,
    left_location: SelectorLocation,
    right: ExpressionHandle,
    right_location: SelectorLocation,
    selectors: &mut SelectorEvaluation<'_>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> bool {
    if left == right {
        return true;
    }
    let left_extent =
        index_expression_extent(program, left, left_location, selectors, bound_lookup);
    let right_extent =
        index_expression_extent(program, right, right_location, selectors, bound_lookup);
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
    selectors: &mut SelectorEvaluation<'_>,
) -> bool {
    match (left, right) {
        (EvaluatedIndexExtent::Point(Some(left)), EvaluatedIndexExtent::Point(Some(right))) => {
            !bound_is_strictly_before(left.clone(), right.clone(), selectors)
                && !bound_is_strictly_before(right.clone(), left.clone(), selectors)
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
    selectors: &mut SelectorEvaluation<'_>,
) -> bool {
    if index_window_provably_empty(start.clone(), end.clone(), selectors) {
        return false;
    }
    let Some(point) = point else {
        return true;
    };
    if start.is_some_and(|start| bound_is_strictly_before(point.clone(), start, selectors)) {
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
    selectors: &mut SelectorEvaluation<'_>,
) -> bool {
    if index_window_provably_empty(left_start.clone(), left_end.clone(), selectors)
        || index_window_provably_empty(right_start.clone(), right_end.clone(), selectors)
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
    selectors: &mut SelectorEvaluation<'_>,
) -> bool {
    match (start, end) {
        (Some(start), Some(end)) => bound_is_at_or_before(end, start, selectors),
        _ => false,
    }
}

fn range_integer_bounds(
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    range: &TableRangeExpression,
    location: SelectorLocation,
    selectors: &mut SelectorEvaluation<'_>,
) -> (Option<NormalizedBound>, Option<NormalizedBound>) {
    (
        selectors.bound(
            location,
            BorrowCompatibilitySelectorPosition::RangeStart,
            || selector_bound(program, lookup, range.start),
        ),
        exclusive_end_bound(program, lookup, range, location, selectors),
    )
}

/// `left <= right` on normalized bounds. The structural order consults the
/// formation scope's stated premises only after its own same-symbol/integer
/// rule fails; a premise proves the relation but never widens it.
pub(super) fn bound_is_at_or_before(
    left: NormalizedBound,
    right: NormalizedBound,
    selectors: &mut SelectorEvaluation<'_>,
) -> bool {
    if structural_bound_is_at_or_before(left.clone(), right.clone()) {
        return true;
    }
    selectors.prove_ordering(left, BorrowCompatibilityPremiseRelation::LessOrEqual, right)
}

fn structural_bound_is_at_or_before(left: NormalizedBound, right: NormalizedBound) -> bool {
    bound_terms_equal(left.clone(), right.clone())
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
    selectors: &mut SelectorEvaluation<'_>,
) -> bool {
    if structural_bound_is_strictly_before(left.clone(), right.clone()) {
        return true;
    }
    selectors.prove_ordering(
        left,
        BorrowCompatibilityPremiseRelation::StrictlyBefore,
        right,
    )
}

fn structural_bound_is_strictly_before(left: NormalizedBound, right: NormalizedBound) -> bool {
    bound_terms_equal(left.clone(), right.clone())
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
    selectors: &mut SelectorEvaluation<'_>,
) -> bool {
    if structural_bound_equal(left.clone(), right.clone()) {
        return true;
    }
    selectors.prove_ordering(left, BorrowCompatibilityPremiseRelation::Equal, right)
}

fn structural_bound_equal(left: NormalizedBound, right: NormalizedBound) -> bool {
    bound_terms_equal(left.clone(), right.clone())
        && match (bound_offset(left), bound_offset(right)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
}

/// Evaluates an `Index` segment's normalized extent through the selector
/// session, reusing its bounds at the segment's exact path location. The bound positions match the overlap selectors: `Index` for a
/// point expression, `RangeStart`/`RangeExclusiveEnd` for a range window.
pub(super) fn index_expression_extent<'p>(
    program: &'p dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    location: SelectorLocation,
    selectors: &mut SelectorEvaluation<'_>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> EvaluatedIndexExtent {
    // The bound index scans the whole program once; the caller shares one
    // cell across every selector session this pass evaluates.
    match program.expression_table.expression(expression) {
        ExpressionNode::Range(range) => {
            let bound_lookup =
                bound_lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
            let (start, end) =
                range_integer_bounds(program, bound_lookup, range, location, selectors);
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
                    .or_else(|| {
                        selector_bound(
                            program,
                            bound_lookup.get_or_insert_with(|| {
                                validation::ImmutableBoundLookup::new(program)
                            }),
                            expression,
                        )
                    })
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
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    range: &TableRangeExpression,
    location: SelectorLocation,
    selectors: &mut SelectorEvaluation<'_>,
) -> Option<NormalizedBound> {
    selectors.bound(
        location,
        BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
        || {
            let end = selector_bound(program, lookup, range.end)?;
            if !range.end_inclusive {
                return Some(end);
            }
            match end {
                NormalizedBound::Integer(end) => end.checked_add(1).map(NormalizedBound::Integer),
                NormalizedBound::Symbol { symbol, offset } => offset
                    .checked_add(1)
                    .map(|offset| NormalizedBound::Symbol { symbol, offset }),
                // `call result + 1` is outside the result vocabulary, so
                // an inclusive call end reports unknown.
                NormalizedBound::CallResult { .. } => None,
                // `storage + 1` is outside the storage vocabulary, so an
                // inclusive mutable end reports unknown.
                NormalizedBound::Storage { .. } => None,
                // `projection + 1` is likewise outside the segmented
                // vocabulary.
                NormalizedBound::Projected { .. } | NormalizedBound::StorageProjected { .. } => {
                    None
                }
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

/// A selector position's bound. Beyond the immutable-bound vocabulary, a
/// bare mutable storage name contributes its storage bound — the bound
/// means the value currently stored there. Storage bounds are query
/// coordinates only: `bound_terms_equal` never equates them, so a claim can
/// only reach one through a guarantee whose result binding is version-pinned
/// to the same occurrence.
pub(super) fn selector_bound(
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    normalized_bound(program, lookup, expression)
        .or_else(|| {
            validation::mutable_integer_bound_storage_symbol(program, lookup, expression)
                .map(|symbol| NormalizedBound::Storage { symbol })
        })
        .or_else(|| projected_bound(program, lookup, expression))
        .or_else(|| indexed_bound(program, lookup, expression))
}

/// The bound of one receiver-rooted field projection (`pair.first`). An
/// immutable receiver contributes the frozen projection identity; a mutable
/// receiver contributes the projected storage coordinate under the same
/// pin-evidence contract as `Storage`.
pub(super) fn projected_bound(
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    let Some((symbol, field, is_mutable)) =
        validation::projected_integer_bound_root(program, lookup, expression)
    else {
        // A nested owned record path is one complete observation. A final
        // field symbol alone would conflate siblings or different roots.
        let place = validation::canonical_place_from_expression(program, expression)?;
        let symbol = place.root.as_symbol()?;
        if place.segments.is_empty()
            || !place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            })
        {
            return None;
        }
        return append_immutable_projection(
            program,
            lookup,
            NormalizedBound::Symbol { symbol, offset: 0 },
            &place.segments,
        );
    };
    // The bound names the canonical attached-field identity the write frame
    // carries; the authored member spelling can disagree for `self` members.
    let field = match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            validation::effective_member_symbol(program, member.receiver, member)
        }
        _ => field,
    };
    let segments = vec![facts::PlaceSegment::Field { symbol: field }];
    Some(if is_mutable {
        NormalizedBound::StorageProjected { symbol, segments }
    } else {
        NormalizedBound::Projected { symbol, segments }
    })
}

/// Read an exact immutable owned path in the same namespace for premise
/// subjects and compatibility queries. A shared root lends readable storage;
/// another reference inside the path begins a different epoch and refuses.
pub(super) fn append_immutable_projection(
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    bound: NormalizedBound,
    projection: &[facts::PlaceSegment],
) -> Option<NormalizedBound> {
    let (symbol, mut segments, storage) = match bound {
        NormalizedBound::Symbol { symbol, offset: 0 } => (symbol, Vec::new(), false),
        NormalizedBound::Projected { symbol, segments } => (symbol, segments, false),
        NormalizedBound::Storage { symbol } => (symbol, Vec::new(), true),
        NormalizedBound::StorageProjected { symbol, segments } => (symbol, segments, true),
        _ => return None,
    };
    let mut reference = lookup.immutable_binding_type(symbol)?;
    if let Some(shell) = validation::reference_shell(program, reference) {
        if shell.access != language_core::ReferenceAccess::Shared {
            return None;
        }
        reference = shell.referee;
    }
    segments.extend_from_slice(projection);
    for (position, segment) in segments.iter().enumerate() {
        match segment {
            facts::PlaceSegment::Field { .. } => {}
            facts::PlaceSegment::FixedIndex { index } => {
                // Type/path replay alone does not establish array bounds.
                // Each index belongs to the type at its own prefix; an outer
                // array's extent cannot authorize a nested element selection.
                let container = validation::owned_projection_type_reference(
                    program,
                    reference,
                    &segments[..position],
                )?;
                let extent = validation::fixed_array_type_length(program, container)?;
                if *index >= extent {
                    return None;
                }
            }
            _ => return None,
        }
    }
    let leaf = validation::owned_projection_type_reference(program, reference, &segments)?;
    if !program.primitive_type_reference(leaf)?.is_fixed_integer()
        || program.arithmetic_domain_for_type_reference(leaf)
            != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    Some(if storage {
        NormalizedBound::StorageProjected { symbol, segments }
    } else {
        NormalizedBound::Projected { symbol, segments }
    })
}

/// The bound of one fixed-index element place (`items[2]`). An immutable
/// collection contributes the frozen element identity; a mutable collection
/// contributes the element's storage coordinate under the same pin-evidence
/// contract as `Storage`.
pub(super) fn indexed_bound(
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    let (symbol, segments, is_mutable) =
        validation::indexed_integer_bound_root(program, lookup, expression)?;
    Some(if is_mutable {
        NormalizedBound::StorageProjected { symbol, segments }
    } else {
        NormalizedBound::Projected { symbol, segments }
    })
}

pub(super) fn normalized_bound(
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    // A call result has no place identity: the occurrence's own expression
    // handle is its bound, so a nested call's `ensures` minted in the same
    // statement names this exact production — and any resolved projection
    // through it (`choose(seed).first`) names that occurrence's path.
    if let Some(_) = program.expression_table.call_node(expression) {
        return Some(NormalizedBound::CallResult {
            expression,
            segments: Vec::new(),
        });
    }
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Borrow(_)
    ) && let Some(place) = validation::canonical_place_from_expression(program, expression)
        && let Some(root) = place.root.as_expression()
        && matches!(
            program.expression_table.expression(root),
            ExpressionNode::Call(_)
        )
    {
        return Some(NormalizedBound::CallResult {
            expression: root,
            segments: place.segments,
        });
    }
    if let Some(offset) =
        validation::immutable_integer_bound_symbol_offset(program, lookup, expression)
    {
        return Some(NormalizedBound::Symbol {
            symbol: offset.symbol,
            offset: offset.offset,
        });
    }
    if let Some(sum) = validation::immutable_integer_bound_sum(program, lookup, expression) {
        return Some(NormalizedBound::SymbolSum {
            first: sum.first,
            second: sum.second,
            offset: sum.offset,
        });
    }
    let Some(expression) =
        validation::normalize_immutable_integer_bound_expression(program, lookup, expression)
    else {
        return validation::immutable_integer_bound_value_symbol(program, lookup, expression)
            .map(|symbol| NormalizedBound::Symbol { symbol, offset: 0 });
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64().map(NormalizedBound::Integer),
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_is_single_member(path)
            .then_some(NormalizedBound::Symbol {
                symbol: path.symbol,
                offset: 0,
            }),
        _ => None,
    }
}
