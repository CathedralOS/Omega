/*
    Spatial borrow judgments over captured places and their access kinds.
    Bounds are evaluated once per comparison and reused by overlap and
    containment. Stated premises supply only relations available at this use;
    unknown bounds remain conservative. No successful judgment is serialized
    for the checker to reconstruct later.
*/

mod captured_values {
    /*
        Integer selector copies retain their evaluated values, not their names.

        Immutable normalization intentionally stops at a snapshot of mutable
        storage. Two such snapshots can nevertheless agree: the retained operand
        origin and evaluated write prefixes establish whether both observed the
        same contents. Reuse flow's reaching-store and observation laws, including
        alias writes and unknown effects; never reread a mutable initializer or use
        today's premises as evidence about an earlier extraction. The same spatial
        comparison serves source windows, arm joins and executable window repair.
    */
    use crate::published::places::place_at;
    use checked_trees::{FlowConstraintKind, FlowOperandFact, FlowStateFact};
    use facts::{PlaceRoot, PlaceSegment, ScalarValue};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use validation::CanonicalPlace;
    use validation::CheckingInput;
    use validation::ImmutableBoundLookup;
    use validation::find_state_in_machine;
    use validation::has_builtin_bound_expression_meaning;
    use validation::project_type_reference_from_segments;
    use validation::reference_shell;

    use super::StorageSelectorFacts;
    use super::indexes::{NormalizedBound, normalized_bound};
    use super::premises::StatedOrderingPremise;

    pub(super) fn equalities(
        program: &dyn CheckingInput,
        captures: StorageSelectorFacts<'_>,
        left: &[PlaceSegment],
        right: &[PlaceSegment],
        lookup: &ImmutableBoundLookup<'_>,
    ) -> Vec<StatedOrderingPremise> {
        let left_symbols = bound_symbols(program, lookup, left);
        let right_symbols = bound_symbols(program, lookup, right);
        let mut premises = Vec::new();
        let mut symbols = left_symbols.clone();
        for &symbol in &right_symbols {
            crate::shared::collections::push_unique(&mut symbols, symbol);
        }
        for symbol in symbols {
            let Some(source) = stored_call_source(program, captures, symbol, lookup) else {
                continue;
            };
            let origin = source.origin.clone();
            premises.push(StatedOrderingPremise::captured_equality(
                NormalizedBound::Symbol { symbol, offset: 0 },
                origin.clone(),
            ));
            if !source.has_read_epoch
                && let Some(value) =
                    completed_scalar(program, captures, source.selector, &origin, source.operand)
            {
                premises.push(StatedOrderingPremise::captured_equality(
                    NormalizedBound::Symbol { symbol, offset: 0 },
                    NormalizedBound::Integer(value),
                ));
            }
        }
        let mut queried = Vec::new();
        for path in [left, right] {
            for segment in path {
                let PlaceSegment::Index { expression } = *segment else {
                    continue;
                };
                let selectors = match program.expression_table.expression(expression) {
                    ExpressionNode::Range(range) => {
                        vec![range.start, range.end]
                    }
                    _ => vec![expression],
                };
                for selector in selectors {
                    let Some(bound @ NormalizedBound::CallResult { .. }) =
                        normalized_bound(program, lookup, selector)
                    else {
                        continue;
                    };
                    let query = (selector, bound.clone());
                    if queried.contains(&query) {
                        continue;
                    }
                    queried.push(query);
                    let integer = exact_bound_integer(program, captures, lookup, selector, &bound);
                    if let Some(integer) = integer {
                        premises.push(StatedOrderingPremise::captured_equality(
                            bound,
                            NormalizedBound::Integer(integer),
                        ));
                    }
                }
            }
        }
        for left in left_symbols {
            for &right in &right_symbols {
                if left != right && copies_agree(program, captures, left, right) {
                    premises.push(StatedOrderingPremise::captured_equality(
                        NormalizedBound::Symbol {
                            symbol: left,
                            offset: 0,
                        },
                        NormalizedBound::Symbol {
                            symbol: right,
                            offset: 0,
                        },
                    ));
                }
            }
        }
        premises
    }

    /*
        Freeze only an immutable/call value with complete captured evidence. A
        mutable storage query is deliberately outside this value vocabulary.
    */
    pub(super) fn captured_storage_index(
        program: &dyn CheckingInput,
        captures: StorageSelectorFacts<'_>,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<usize> {
        let bound = normalized_bound(program, lookup, expression)?;
        usize::try_from(exact_bound_integer(
            program, captures, lookup, expression, &bound,
        )?)
        .ok()
    }

    struct StoredCallSource<'facts> {
        origin: NormalizedBound,
        selector: ExpressionHandle,
        operand: &'facts FlowOperandFact,
        has_read_epoch: bool,
    }

    fn stored_call_source<'facts>(
        program: &dyn CheckingInput,
        captures: StorageSelectorFacts<'facts>,
        symbol: SymbolHandle,
        lookup: &ImmutableBoundLookup<'_>,
    ) -> Option<StoredCallSource<'facts>> {
        let (state, operand) = stored_copy(program, captures, symbol)?;
        let values = crate::flow::captured_operand_values(
            program,
            captures.flow,
            captures.semantic,
            state,
            operand,
            operand.type_reference,
            &[],
        )?;
        let [value] = values.as_slice() else {
            return None;
        };
        let NormalizedBound::CallResult {
            expression,
            mut segments,
        } = normalized_bound(program, lookup, value.projection.expression)?
        else {
            return None;
        };
        segments.extend_from_slice(&value.projection.remaining);
        if !segments.iter().all(|segment| {
            matches!(
                segment,
                PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
            )
        }) {
            return None;
        }
        Some(StoredCallSource {
            origin: NormalizedBound::CallResult {
                expression,
                segments,
            },
            selector: value.projection.expression,
            operand: value.operand.row(),
            has_read_epoch: value.producer_epoch.is_some(),
        })
    }

    fn exact_bound_integer(
        program: &dyn CheckingInput,
        captures: StorageSelectorFacts<'_>,
        lookup: &ImmutableBoundLookup<'_>,
        selector: ExpressionHandle,
        bound: &NormalizedBound,
    ) -> Option<i64> {
        match bound {
            NormalizedBound::Integer(value) => Some(*value),
            NormalizedBound::Symbol { symbol, offset } => {
                let source = stored_call_source(program, captures, *symbol, lookup)?;
                if source.has_read_epoch {
                    return None;
                }
                completed_scalar(
                    program,
                    captures,
                    source.selector,
                    &source.origin,
                    source.operand,
                )?
                .checked_add(*offset)
            }
            NormalizedBound::CallResult { expression, .. } => captures
                .flow
                .control
                .with_expression_operands(*expression, |operands| {
                    let mut value = None;
                    for operand in operands {
                        let incoming = completed_scalar(
                            program,
                            captures,
                            selector,
                            bound,
                            captures.flow.control.operands.get(*operand),
                        )?;
                        if value.is_some_and(|previous| previous != incoming) {
                            return None;
                        }
                        value = Some(incoming);
                    }
                    value
                }),
            _ => None,
        }
    }

    fn bound_symbols(
        program: &dyn CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        path: &[PlaceSegment],
    ) -> Vec<SymbolHandle> {
        let mut symbols = Vec::new();
        for segment in path {
            let PlaceSegment::Index { expression } = *segment else {
                continue;
            };
            let mut collect = |expression| {
                let bound = normalized_bound(program, lookup, expression);
                let candidates = match bound {
                    Some(NormalizedBound::Symbol { symbol, .. }) => {
                        [symbol, SymbolHandle::invalid()]
                    }
                    Some(NormalizedBound::SymbolSum { first, second, .. }) => [first, second],
                    _ => [SymbolHandle::invalid(); 2],
                };
                for symbol in candidates {
                    if symbol.is_valid() && !symbols.contains(&symbol) {
                        symbols.push(symbol);
                    }
                }
            };
            if let ExpressionNode::Range(range) = program.expression_table.expression(expression) {
                collect(range.start);
                collect(range.end);
            } else {
                collect(expression);
            }
        }
        symbols
    }

    fn copies_agree(
        program: &dyn CheckingInput,
        captures: StorageSelectorFacts<'_>,
        left: SymbolHandle,
        right: SymbolHandle,
    ) -> bool {
        let (Some((left_state, left)), Some((right_state, right))) = (
            stored_copy(program, captures, left),
            stored_copy(program, captures, right),
        ) else {
            return false;
        };
        if left_state.state_symbol != right_state.state_symbol {
            return false;
        }
        let Some(left) = crate::flow::captured_operand_values(
            program,
            captures.flow,
            captures.semantic,
            left_state,
            left,
            left.type_reference,
            &[],
        ) else {
            return false;
        };
        let Some(right) = crate::flow::captured_operand_values(
            program,
            captures.flow,
            captures.semantic,
            right_state,
            right,
            right.type_reference,
            &[],
        ) else {
            return false;
        };
        let ([left], [right]) = (left.as_slice(), right.as_slice()) else {
            return false;
        };
        let place = |value: &crate::flow::CapturedValue<'_>| {
            let mut place = place_at(
                program,
                value.operand.state_symbol,
                value.operand.statement_index,
                value.projection.expression,
            )?;
            place.extend_segments(&value.projection.remaining);
            Some(place)
        };
        let (Some(left_place), Some(right_place)) = (place(left), place(right)) else {
            return false;
        };
        left_place == right_place
            && crate::flow::operand_observations_agree(
                program,
                captures.flow,
                left.operand.row(),
                right.operand.row(),
                &left_place,
            )
    }

    fn stored_copy<'facts>(
        program: &dyn CheckingInput,
        captures: StorageSelectorFacts<'facts>,
        symbol: SymbolHandle,
    ) -> Option<(&'facts FlowStateFact, crate::flow::CapturedOperand<'facts>)> {
        let state_symbol = program.symbols.get(symbol).parent;
        let state_flow = captures.flow.control.state_by_symbol(state_symbol)?;
        let state = find_state_in_machine(program, state_flow.machine_symbol, state_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let (statement_index, local) =
            statements
                .iter()
                .enumerate()
                .find_map(|(index, statement)| {
                    statement
                        .as_local_data()
                        .filter(|local| local.symbol == symbol && !local.is_mutable)
                        .map(|local| (index, local))
                })?;
        let statement = captures
            .flow
            .control
            .statements
            .span(state_flow.statements)?
            .iter()
            .find(|statement| statement.statement_index == statement_index)?;
        if !captures
            .flow
            .control
            .operands
            .is_valid(statement.stored_value)
            || !captures
                .semantic
                .places
                .is_valid(statement.stored_destination)
        {
            return None;
        }
        let destination = captures.semantic.places.get(statement.stored_destination);
        let destination = crate::flow::canonical_place_from_semantic_place(
            program,
            captures.semantic,
            destination,
        )?;
        let value = captures.flow.control.operands.get(statement.stored_value);
        (destination.root == PlaceRoot::Symbol(symbol)
            && destination.segments.is_empty()
            && value.expression == local.initial_value
            && value.type_reference == local.type_reference)
            .then(|| {
                crate::flow::CapturedOperand::recorded(
                    captures.flow.control,
                    statement.stored_value,
                )
            })
            .flatten()
            .map(|value| (state_flow, value))
    }

    /*
        The reaching-value selector above supplies one complete owned completion.
        Its typed integer observation belongs to that operand's captured epoch;
        borrowing its contents or consulting a later value cannot supply equality.
    */
    fn completed_scalar(
        program: &dyn CheckingInput,
        captures: StorageSelectorFacts<'_>,
        selector: ExpressionHandle,
        bound: &NormalizedBound,
        operand: &FlowOperandFact,
    ) -> Option<i64> {
        let NormalizedBound::CallResult {
            expression,
            segments,
        } = bound
        else {
            return None;
        };
        if operand.expression != *expression
            || !segments.iter().all(|segment| {
                matches!(
                    segment,
                    PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
                )
            })
        {
            return None;
        }
        let control = &captures.flow.control;
        let state = control.state_by_symbol(operand.state_symbol)?;
        let machine = program.machine_by_symbol(state.machine_symbol)?;
        let typed_state = program.state_by_symbol(state.state_symbol)?;
        if !control.calls.span(state.calls)?.iter().any(|call| {
            call.authored_expression == *expression
                && call.statement_index == operand.statement_index
        }) || !has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(typed_state),
            selector,
        ) {
            return None;
        }
        for depth in 0..=segments.len() {
            let prefix = project_type_reference_from_segments(
                program,
                operand.type_reference,
                &segments[..depth],
            )?;
            if reference_shell(program, prefix).is_some() {
                return None;
            }
        }
        let selected =
            project_type_reference_from_segments(program, operand.type_reference, segments)?;
        if !program
            .primitive_type_reference(selected)?
            .is_fixed_integer()
        {
            return None;
        }
        let ScalarValue::Integer(integer) = crate::values::scalar_value_at_place(
            program,
            captures.semantic,
            captures
                .flow
                .contexts
                .constraint_refs
                .span(operand.constraints)?
                .iter()
                .filter_map(|reference| {
                    let FlowConstraintKind::SemanticContext { context } = reference.kind else {
                        return None;
                    };
                    Some(captures.semantic.contexts.get(context))
                }),
            &CanonicalPlace {
                root: PlaceRoot::Expression(*expression),
                segments: segments.clone(),
            },
        )?
        else {
            return None;
        };
        integer.to_i64()
    }
}
mod indexes {
    use crate::flow::expressions;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
    use validation::{
        integer_bound_offset as bound_offset, integer_bound_terms_equal as bound_terms_equal,
    };

    use super::premises::{
        BorrowCompatibilityPremiseRelation as PremiseRelation, StatedOrderingPremise,
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

    pub(super) use validation::IntegerBound as NormalizedBound;

    /*
        The normalized extent of one `Index` segment expression as evaluated inside
        a selector session. A non-range expression is a single point bound; a range
        expression is a half-open `[start, end)` window. `None` bounds are
        conservatively unknown, never negative evidence.
    */
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

    /*
        One comparison's bound cache. Overlap and containment share values, not
        serialized snapshots. Mutable storage remains a query coordinate and can
        only be ordered with premises whose availability was established at use.
    */
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

        /*
            Ask only the current scope's established integer relations. Exact
            equations may transport a query through completed results and joins;
            they never rewrite ambient premises or manufacture a storage version.
        */
        fn prove_ordering(
            &mut self,
            left: NormalizedBound,
            relation: PremiseRelation,
            right: NormalizedBound,
        ) -> bool {
            validation::prove_integer_ordering(self.premises, left, relation, right)
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

    /*
        Whether two `Index` segment expressions may select the same element.

        Both expressions are evaluated to their normalized extent inside the
        selector session -- a point bound or a half-open window -- and compared by
        the shared bound-ordering layer, so symbolic points order against window
        bounds the same way window bounds order against each other. Unknown bounds
        stay conservatively overlapping. Disjointness needs ordering or singleton
        disequality, and a stated premise is consulted only when structural
        relations cannot settle it.
    */
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

    /*
        Whether two evaluated `Index` extents may select the same element. Two
        points are disjoint when strict ordering or disequality separates them; a
        point and a window are disjoint only when the point provably sits outside
        or the window is provably empty; two windows are disjoint when one ends at
        or before the other starts. Every unproven ordering stays overlapping.
    */
    pub(super) fn index_extents_may_overlap(
        left: EvaluatedIndexExtent,
        right: EvaluatedIndexExtent,
        selectors: &mut SelectorEvaluation<'_>,
    ) -> bool {
        match (left, right) {
            (EvaluatedIndexExtent::Point(Some(left)), EvaluatedIndexExtent::Point(Some(right))) => {
                !bound_is_strictly_before(left.clone(), right.clone(), selectors)
                    && !bound_is_strictly_before(right.clone(), left.clone(), selectors)
                    && !selectors.prove_ordering(left, PremiseRelation::NotEqual, right)
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

    /*
        The point extent of a fixed literal index segment. An index above the i64
        window stays unknown rather than borrowing a different bound's identity.
    */
    pub(super) fn fixed_index_extent(index: usize) -> EvaluatedIndexExtent {
        EvaluatedIndexExtent::Point(i64::try_from(index).ok().map(NormalizedBound::Integer))
    }

    /* The half-open window extent of a fixed literal range segment. */
    pub(super) fn fixed_range_extent(start: usize, end: usize) -> EvaluatedIndexExtent {
        EvaluatedIndexExtent::Window {
            start: i64::try_from(start).ok().map(NormalizedBound::Integer),
            end: i64::try_from(end).ok().map(NormalizedBound::Integer),
        }
    }

    /*
        `[start, end)` may contain `point` unless the point provably precedes the
        window start, sits at or beyond the window end, or the window is provably
        empty. An unknown point or an unordered bound pair stays contained --
        disjointness needs positive evidence.
    */
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

    /*
        Two half-open windows `[ls, le)` and `[rs, re)` may overlap unless one is
        provably empty or one provably ends at or before the other starts.
    */
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

    /*
        A half-open window `[start, end)` with `end <= start` is empty and therefore
        overlaps nothing. Shared-symbol offsets order as mathematical integers;
        distinct runtime symbols remain unordered unless a stated premise orders
        them.
    */
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

    /*
        `left <= right` on normalized bounds. The structural order consults the
        formation scope's stated premises only after its own same-symbol/integer
        rule fails; a premise proves the relation but never widens it.
    */
    pub(super) fn bound_is_at_or_before(
        left: NormalizedBound,
        right: NormalizedBound,
        selectors: &mut SelectorEvaluation<'_>,
    ) -> bool {
        if structural_bound_is_at_or_before(left.clone(), right.clone()) {
            return true;
        }
        selectors.prove_ordering(left, PremiseRelation::LessOrEqual, right)
    }

    fn structural_bound_is_at_or_before(left: NormalizedBound, right: NormalizedBound) -> bool {
        bound_terms_equal(left.clone(), right.clone())
            && match (bound_offset(left), bound_offset(right)) {
                (Some(left), Some(right)) => left <= right,
                _ => false,
            }
    }

    /*
        Strict `<` ordering for bounds; used where a window must be provably
        non-empty or a point must sit strictly below an exclusive end.
    */
    pub(super) fn bound_is_strictly_before(
        left: NormalizedBound,
        right: NormalizedBound,
        selectors: &mut SelectorEvaluation<'_>,
    ) -> bool {
        if structural_bound_is_strictly_before(left.clone(), right.clone()) {
            return true;
        }
        selectors.prove_ordering(left, PremiseRelation::StrictlyBefore, right)
    }

    fn structural_bound_is_strictly_before(left: NormalizedBound, right: NormalizedBound) -> bool {
        bound_terms_equal(left.clone(), right.clone())
            && match (bound_offset(left), bound_offset(right)) {
                (Some(left), Some(right)) => left < right,
                _ => false,
            }
    }

    /*
        Exact bound equality: literal values, the same symbol at the same offset,
        or a stated `==` premise that fixes both bounds to one offset line.
        Anything else stays unproven rather than assumed distinct.
    */
    pub(super) fn bound_equal(
        left: NormalizedBound,
        right: NormalizedBound,
        selectors: &mut SelectorEvaluation<'_>,
    ) -> bool {
        if structural_bound_equal(left.clone(), right.clone()) {
            return true;
        }
        selectors.prove_ordering(left, PremiseRelation::Equal, right)
    }

    fn structural_bound_equal(left: NormalizedBound, right: NormalizedBound) -> bool {
        bound_terms_equal(left.clone(), right.clone())
            && match (bound_offset(left), bound_offset(right)) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
    }

    /*
        Evaluates an `Index` segment's normalized extent through the selector
        session, reusing its bounds at the segment's exact path location. The bound positions match the overlap selectors: `Index` for a
        point expression, `RangeStart`/`RangeExclusiveEnd` for a range window.
    */
    pub(super) fn index_expression_extent<'p>(
        program: &'p dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        location: SelectorLocation,
        selectors: &mut SelectorEvaluation<'_>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> EvaluatedIndexExtent {
        match program.expression_table.expression(expression) {
            ExpressionNode::Range(range) => {
                let bound_lookup = bound_lookup
                    .get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
                let (start, end) =
                    range_integer_bounds(program, bound_lookup, range, location, selectors);
                EvaluatedIndexExtent::Window { start, end }
            }
            _ => EvaluatedIndexExtent::Point(selectors.bound(
                location,
                BorrowCompatibilitySelectorPosition::Index,
                || {
                    expressions(program)
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

    /*
        The half-open (exclusive) upper bound of a range window.

        All overlap reasoning here is in terms of half-open windows `[start, end)`.
        An inclusive range `a..=b` covers index `b`, so its exclusive end is `b + 1`.
        Normalizing here keeps the window overlap and point-containment checks
        sound: without it, `view[0..=3]` would be read as `[0, 3)` and a borrow of
        element 3 (or window `3..5`) would be mis-classified as disjoint.

        A `b + 1` that overflows `i64` (the `..=i64::MAX` edge), including a
        symbolic offset, cannot be represented as an exclusive bound, so the end is
        reported as unknown (`None`), which the overlap checks treat conservatively
        as possibly-overlapping.
    */
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
                    NormalizedBound::Integer(end) => {
                        end.checked_add(1).map(NormalizedBound::Integer)
                    }
                    NormalizedBound::Symbol { symbol, offset } => offset
                        .checked_add(1)
                        .map(|offset| NormalizedBound::Symbol { symbol, offset }),
                    NormalizedBound::CallResult { .. } | NormalizedBound::EntryRead(_) => None,
                    NormalizedBound::Storage { .. } => None,
                    NormalizedBound::Projected { .. }
                    | NormalizedBound::StorageProjected { .. } => None,
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

    /*
        A selector position's bound. Beyond the immutable-bound vocabulary, a
        bare mutable storage name contributes its storage bound — the bound
        means the value currently stored there. Storage bounds are query
        coordinates only: `bound_terms_equal` never equates them, so a claim can
        only reach one through a guarantee whose result binding is version-pinned
        to the same occurrence.
    */
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

    /*
        The bound of one receiver-rooted field projection (`pair.first`). An
        immutable receiver contributes the frozen projection identity; a mutable
        receiver contributes the projected storage coordinate under the same
        pin-evidence contract as `Storage`.
    */
    pub(super) fn projected_bound(
        program: &dyn ::validation::CheckingInput,
        lookup: &validation::ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<NormalizedBound> {
        let Some((symbol, field, is_mutable)) =
            validation::projected_integer_bound_root(program, lookup, expression)
        else {
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

    /*
        Read an exact immutable owned path in the same namespace for premise
        subjects and compatibility queries. A shared root lends readable storage;
        another reference inside the path begins a different epoch and refuses.
    */
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

    /*
        The bound of one fixed-index element place (`items[2]`). An immutable
        collection contributes the frozen element identity; a mutable collection
        contributes the element's storage coordinate under the same pin-evidence
        contract as `Storage`.
    */
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
        if expressions(program)
            .indexed_node(expression)
            .is_some_and(|indexed| {
                expressions(program)
                    .constant_integer_value(indexed.index)
                    .is_none()
            })
            && let Some(identity) =
                validation::immutable_entry_indexed_read_identity(program, expression)
        {
            return Some(NormalizedBound::EntryRead(identity));
        }
        if program.expression_table.call_node(expression).is_some() {
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
            ExpressionNode::Name(path) => expressions(program)
                .name_path_is_single_member(path)
                .then_some(NormalizedBound::Symbol {
                    symbol: path.symbol,
                    offset: 0,
                }),
            _ => None,
        }
    }
}
mod premises {
    /*
        Integer relations available at the exact compatibility judgment.

        A premised compatibility derivation consumes the same establishment point
        as the range checker: authored preconditions and surviving incoming
        guards. Guard queries are transported back through immutable scalar
        parameter bindings to the guard's evaluation scope. This direction also
        handles multiple destination parameters carrying the same source value.
        Availability and transport are established from the typed program.
        Premises prove bound ordering, equality or disequality the structural
        judgment could not. Disequality distinguishes singleton elements without
        choosing an ordering; it says nothing about overlap of wider windows.
        When no single premise names both bounds, one ordered premise pair may
        compose transitively through a shared middle bound (`i < n` with
        `n <= j` proves `i < j`); `==` legs substitute from either side and `!=`
        legs compose only with exact equality substitution.
        Checked predicate calls contribute the relations the shared symbolic body
        prover establishes on every true return, substituted onto exact actuals.
        No relation can form a loan, extend a lifetime, or widen access.
        The arithmetic-domain oracle owns pure ordering composition; this client
        establishes which exact source premises are available to that oracle.
        Required domain membership uses the same decomposition with a definition
        scope: its reserved self binds the immutable membership subject. The
        definition-owned meaning reader rejects foreign self and selected operators.
        Call guarantees join the contract checker's shared availability reader at
        each statement entry. They never enter the stable state-entry premise set.
        The exact callee contract and immutable captured result survive separately;
        both must rejoin before a reserved result can license separation.
        A `requires` row spelled as a transparent proposition application
        decomposes the instantiated formula under the same meaning gate, with each
        declared parameter bound to its actual argument's normalized bound;
        primitive and witness propositions carry no formula and supply nothing.
    */
    use checked_trees::{CheckFacts, ContractProofFactKind, FlowStateFact};
    use facts::FactContextHandle;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use symbols::SymbolHandle;
    use typed_trees::domain::DomainDefinition;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateParameter;
    use typed_trees::signature::state_parameter_position;
    use typed_trees::state::State;
    use validation::CallFrameResolver;
    use validation::ImmutableBoundLookup;
    use validation::PredicateIntegerOperand;
    use validation::canonical_place_from_expression;
    use validation::exact_domain_subject_type;
    use validation::find_state_in_machine;
    use validation::has_builtin_decomposed_guard_meaning;
    use validation::has_builtin_domain_decomposed_guard_meaning;
    use validation::instantiate_immutable_entry_indexed_read_identity;
    use validation::predicate_ordering_consequences;

    use super::indexes::{NormalizedBound, normalized_bound};
    use crate::checks::contracts::prover::call_guarantees::{
        self, AvailableGuarantee, StateGuarantees,
    };
    use crate::checks::ranges::incoming_guards::IncomingGuardIndex;
    use crate::checks::ranges::requirements::state_requires_facts;

    pub(super) use validation::IntegerOrderingRelation as BorrowCompatibilityPremiseRelation;
    pub use validation::StatedOrderingPremise;

    mod domains {
        /*
            Required membership supplies a theory for one immutable captured subject.
            The exact predicate root is checked through the common Boolean decomposer;
            finding a comparison somewhere inside a predicate is not an entailment.
            This consumes membership, including routed membership, without minting it.
        */
        use super::{PremiseScope, StatedOrderingPremise, decompose_premise_expression};
        use typed_trees::domain::{ProofMembershipFact, has_symbol_only_proof};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use validation::ImmutableBoundLookup;
        use validation::has_builtin_bound_expression_meaning;
        use validation::has_exact_integer_domain_subject;

        pub(super) fn append_membership_premises(
            program: &dyn ::validation::CheckingInput,
            lookup: &ImmutableBoundLookup<'_>,
            machine: &Machine,
            state: &State,
            membership: &ProofMembershipFact,
            premises: &mut Vec<StatedOrderingPremise>,
        ) {
            let mut definitions = program
                .domain_definitions()
                .iter()
                .filter(|definition| definition.symbol == membership.domain_symbol);
            let Some(domain) = definitions.next() else {
                return;
            };
            if definitions.next().is_some()
                || !domain.type_parameters.is_empty()
                || !domain.index_arguments.is_empty()
                || !membership.domain_arguments.is_empty()
                || domain.alias.is_some()
                || !domain.semantic_id.is_valid()
                || domain.semantic_id != membership.semantic_domain
                || !has_symbol_only_proof(program, domain.symbol)
            {
                return;
            }
            if !has_exact_integer_domain_subject(program, domain, lookup, membership.value)
                || !has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    membership.value,
                )
            {
                return;
            }
            let Some(subject) = super::normalized_bound(program, lookup, membership.value)
                .or_else(|| super::projected_immutable_bound(program, lookup, membership.value))
            else {
                return;
            };
            for predicate in domain.facts.handles() {
                let Some(expression) = program.proof_facts.get(predicate).as_expression() else {
                    continue;
                };
                decompose_premise_expression(
                    program,
                    lookup,
                    PremiseScope::Domain {
                        definition: domain,
                        subject: subject.clone(),
                    },
                    *expression,
                    false,
                    &None,
                    premises,
                );
            }
        }
    }
    mod propositions {
        /*
            Authored transparent propositions supply proof-derived ordering premises.

            `requires Distinct(i, j)` where `Distinct` is declared with a formula
            (`proposition Distinct(left: u64, right: u64) = left != right;`) decomposes
            the instantiated formula under the machine scope's builtin-meaning gate.
            Each declared proposition parameter binds the normalized bound of its
            actual argument, so `i != j` arrives as the same disequality premise an
            inline `requires i != j` would supply. Primitive and witness propositions
            own no formula and contribute nothing; an application whose argument
            cannot normalize to an immutable integer bound supplies no premise either.
        */
        use super::super::indexes::{NormalizedBound, normalized_bound};
        use super::{PremiseScope, StatedOrderingPremise, decompose_premise_expression};
        use crate::flow::expressions;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::proposition::{
            PropositionApplication, PropositionBody, PropositionFormula,
        };
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;

        /*
            Decompose a proposition-spelled requires row when the proposition owns a
            transparent Boolean formula. Every other body kind contributes nothing.
        */
        pub(super) fn append_proposition_premises(
            program: &dyn ::validation::CheckingInput,
            lookup: &validation::ImmutableBoundLookup<'_>,
            machine: &Machine,
            state: &State,
            application: &PropositionApplication,
            premises: &mut Vec<StatedOrderingPremise>,
        ) {
            let Some(definition) =
                program.proposition_definition_by_symbol(application.proposition)
            else {
                return;
            };
            let PropositionBody::Transparent { proposition } = &definition.body else {
                return;
            };
            let PropositionFormula::BooleanExpression(formula) = proposition else {
                return;
            };
            let parameters = program.proposition_parameters(definition);
            let arguments = expressions(program).expression_handles(application.arguments);
            if parameters.len() != arguments.len() {
                return;
            }
            decompose_premise_expression(
                program,
                lookup,
                PremiseScope::Proposition {
                    machine,
                    state,
                    parameters,
                    arguments,
                },
                *formula,
                false,
                &None,
                premises,
            );
        }

        /*
            Substitute proposition parameters inside a normalized formula bound with
            the actual argument's normalized bound, composing constant offsets. A
            symbol not declared as a proposition parameter, or an actual that is not a
            single symbol or integer bound, makes the bound unprovable rather than
            silently approximate.
        */
        pub(super) fn substitute_bound(
            program: &dyn ::validation::CheckingInput,
            lookup: &validation::ImmutableBoundLookup<'_>,
            bound: NormalizedBound,
            parameters: &[StateParameter],
            arguments: &[ExpressionHandle],
        ) -> Option<NormalizedBound> {
            let argument_bound = |symbol: symbols::SymbolHandle| {
                let index = typed_trees::signature::state_parameter_position(parameters, symbol)?;
                normalized_bound(program, lookup, arguments[index])
            };
            match bound {
                NormalizedBound::Integer(_) => Some(bound),
                NormalizedBound::Symbol { symbol, offset } => match argument_bound(symbol)? {
                    NormalizedBound::Symbol {
                        symbol: actual,
                        offset: actual_offset,
                    } => Some(NormalizedBound::Symbol {
                        symbol: actual,
                        offset: offset + actual_offset,
                    }),
                    NormalizedBound::Integer(value) => {
                        Some(NormalizedBound::Integer(offset + value))
                    }
                    NormalizedBound::SymbolSum { .. }
                    | NormalizedBound::Storage { .. }
                    | NormalizedBound::Projected { .. }
                    | NormalizedBound::StorageProjected { .. }
                    | NormalizedBound::CallResult { .. }
                    | NormalizedBound::EntryRead(_) => None,
                },
                NormalizedBound::Projected { symbol, segments } => match argument_bound(symbol)? {
                    NormalizedBound::Symbol {
                        symbol: actual,
                        offset: 0,
                    } => Some(NormalizedBound::Projected {
                        symbol: actual,
                        segments,
                    }),
                    _ => None,
                },
                NormalizedBound::Storage { .. }
                | NormalizedBound::StorageProjected { .. }
                | NormalizedBound::CallResult { .. }
                | NormalizedBound::EntryRead(_) => None,
                NormalizedBound::SymbolSum {
                    first,
                    second,
                    offset,
                } => {
                    let substitute = |symbol| match argument_bound(symbol)? {
                        NormalizedBound::Symbol {
                            symbol: actual,
                            offset: actual_offset,
                        } => Some((actual, actual_offset)),
                        _ => None,
                    };
                    let (first, first_offset) = substitute(first)?;
                    let (second, second_offset) = substitute(second)?;
                    if first == second {
                        return None;
                    }
                    let (first, second) = if (first.arena_index(), first.generation())
                        < (second.arena_index(), second.generation())
                    {
                        (first, second)
                    } else {
                        (second, first)
                    };
                    Some(NormalizedBound::SymbolSum {
                        first,
                        second,
                        offset: offset + first_offset + second_offset,
                    })
                }
            }
        }
    }
    mod stored_values {
        /*
            Completed assignment values supplement relational premises at their live use.

            Field constructions, scalar calls and value joins already publish scalar
            observations through flow's shared value owner. Borrow compatibility must
            consume those observations too; replaying their initializers would lose the
            captured value and could reread mutated storage. Only immutable owned paths
            have a timeless normalized bound here. Mutable locals and paths through any
            reference retain occurrence-dependent storage, so current observations must
            not become equations about a previously captured loan selector.
        */
        use facts::{FactPayload, ScalarValue};
        use typed_trees::state::State;

        use super::super::indexes::NormalizedBound;
        use super::StatedOrderingPremise;

        pub(super) fn append(
            program: &dyn validation::CheckingInput,
            facts: &checked_trees::CheckFacts,
            state: &State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            premises: &mut Vec<StatedOrderingPremise>,
        ) {
            let mut places = Vec::new();
            for fact in contexts.iter().flat_map(|context| {
                facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(*context))
                    .facts()
            }) {
                if !matches!(
                    fact.payload,
                    FactPayload::AssignedValue { .. }
                        | FactPayload::AssignedScalarValue { .. }
                        | FactPayload::AssignedIntegerBounds { .. }
                ) {
                    continue;
                }
                let Some(place) = fact.place.as_place().and_then(|place| {
                    crate::flow::canonical_place_from_semantic_place(
                        program,
                        &facts.semantic,
                        facts.semantic.places.get(place),
                    )
                }) else {
                    continue;
                };
                let ordinal = places.len();
                places.push((ordinal, place));
            }
            places.sort_unstable_by(|left, right| {
                left.1
                    .compare_identity(&right.1)
                    .then_with(|| left.0.cmp(&right.0))
            });
            places.dedup_by(|left, right| left.1 == right.1);
            places.sort_unstable_by_key(|(ordinal, _)| *ordinal);
            let mut observations = None;
            for (_, place) in places {
                let Some(symbol) =
                    crate::values::immutable_owned_place_root(program, state, statement, &place)
                else {
                    continue;
                };
                let bound = if place.segments.is_empty() {
                    NormalizedBound::Symbol { symbol, offset: 0 }
                } else {
                    NormalizedBound::Projected {
                        symbol,
                        segments: place.segments.clone(),
                    }
                };
                let observations = observations.get_or_insert_with(|| {
                    crate::values::AssignmentObservationBatch::prepare(
                        program,
                        &facts.semantic,
                        contexts
                            .iter()
                            .map(|context| facts.semantic.contexts.get(*context)),
                    )
                });
                let Some(ScalarValue::Integer(value)) = observations.scalar_value(&place) else {
                    continue;
                };
                let Some(value) = value.to_i64() else {
                    continue;
                };
                premises.push(StatedOrderingPremise::captured_equality(
                    bound,
                    NormalizedBound::Integer(value),
                ));
            }
        }
    }

    #[derive(Clone)]
    enum PremiseScope<'program> {
        State {
            machine: &'program Machine,
            state: &'program State,
        },
        Domain {
            definition: &'program DomainDefinition,
            subject: NormalizedBound,
        },
        Call {
            guarantee: &'program AvailableGuarantee<'program>,
            result: SymbolHandle,
            /*
                Whether the bound storage is mutable: premise operands then mint
                the storage bound, matching the query coordinate a mutable
                selector contributes at this scope.
            */
            result_mutable: bool,
        },
        /*
            A transparent proposition formula evaluated in the machine scope of
            the requires row: `parameters`/`arguments` pair the proposition's
            declared value parameters with the application's actual expressions.
        */
        Proposition {
            machine: &'program Machine,
            state: &'program State,
            parameters: &'program [StateParameter],
            arguments: &'program [ExpressionHandle],
        },
    }

    impl PremiseScope<'_> {
        fn has_builtin_meaning(
            &self,
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            match self {
                Self::Call { guarantee, .. } => guarantee.has_builtin_meaning(program, expression),
                Self::State { machine, state } => {
                    has_builtin_decomposed_guard_meaning(program, machine, Some(state), expression)
                }
                Self::Domain { definition, .. } => {
                    has_builtin_domain_decomposed_guard_meaning(program, definition, expression)
                }
                Self::Proposition { machine, state, .. } => {
                    has_builtin_decomposed_guard_meaning(program, machine, Some(state), expression)
                }
            }
        }

        fn projected_bound(
            &self,
            program: &dyn ::validation::CheckingInput,
            lookup: &ImmutableBoundLookup<'_>,
            expression: ExpressionHandle,
            projection: &[PlaceSegment],
        ) -> Option<NormalizedBound> {
            if projection.is_empty() {
                return self.bound(program, lookup, expression);
            }
            if let Some(base) = self.bound(program, lookup, expression) {
                return super::indexes::append_immutable_projection(
                    program, lookup, base, projection,
                );
            }
            let (expression, mut prefix) = match self {
                Self::State { .. } => (expression, Vec::new()),
                Self::Call { guarantee, .. } => guarantee.actual_projection(program, expression)?,
                Self::Proposition {
                    parameters,
                    arguments,
                    ..
                } => {
                    let place = canonical_place_from_expression(program, expression)?;
                    let PlaceRoot::Symbol(symbol) = place.root else {
                        return None;
                    };
                    let position = state_parameter_position(parameters, symbol)?;
                    (arguments[position], place.segments)
                }
                Self::Domain { .. } => return None,
            };
            let place = canonical_place_from_expression(program, expression)?;
            let PlaceRoot::Symbol(symbol) = place.root else {
                return None;
            };
            let mut segments = place.segments;
            segments.append(&mut prefix);
            let base = NormalizedBound::Projected { symbol, segments };
            super::indexes::append_immutable_projection(program, lookup, base, projection)
        }

        fn bound(
            &self,
            program: &dyn ::validation::CheckingInput,
            lookup: &ImmutableBoundLookup<'_>,
            expression: ExpressionHandle,
        ) -> Option<NormalizedBound> {
            match self {
                Self::Call {
                    guarantee,
                    result,
                    result_mutable,
                } => {
                    if let Some(segments) = guarantee.result_segments(program, expression) {
                        match segments.as_slice() {
                            [] => {
                                if result.is_valid() {
                                    Some(if *result_mutable {
                                        NormalizedBound::Storage { symbol: *result }
                                    } else {
                                        NormalizedBound::Symbol {
                                            symbol: *result,
                                            offset: 0,
                                        }
                                    })
                                } else {
                                    guarantee.call_expression().map(|expression| {
                                        NormalizedBound::CallResult {
                                            expression,
                                            segments: Vec::new(),
                                        }
                                    })
                                }
                            }
                            [_, ..]
                                if segments.iter().all(|segment| {
                                    matches!(
                                        segment,
                                        PlaceSegment::Field { .. }
                                            | PlaceSegment::FixedIndex { .. }
                                    )
                                }) =>
                            {
                                if result.is_valid() {
                                    let segments = segments.to_vec();
                                    Some(if *result_mutable {
                                        NormalizedBound::StorageProjected {
                                            symbol: *result,
                                            segments,
                                        }
                                    } else {
                                        NormalizedBound::Projected {
                                            symbol: *result,
                                            segments,
                                        }
                                    })
                                } else {
                                    guarantee.call_expression().map(|expression| {
                                        NormalizedBound::CallResult {
                                            expression,
                                            segments: segments.to_vec(),
                                        }
                                    })
                                }
                            }
                            _ => None,
                        }
                    } else {
                        let (actual, remaining) =
                            guarantee.actual_projection(program, expression)?;
                        match remaining.as_slice() {
                            [] => normalized_bound(program, lookup, actual).or_else(|| {
                                if !matches!(
                                    program.expression_table.expression(actual),
                                    ExpressionNode::Borrow(borrow)
                                        if borrow.access.is_exclusive()
                                ) {
                                    return None;
                                }
                                let place = canonical_place_from_expression(program, actual)?;
                                match (place.root, place.segments.as_slice()) {
                                    (PlaceRoot::Symbol(symbol), []) => {
                                        Some(NormalizedBound::Storage { symbol })
                                    }
                                    (PlaceRoot::Symbol(symbol), segments @ [_, ..])
                                        if segments.iter().all(|segment| {
                                            matches!(
                                                segment,
                                                PlaceSegment::Field { .. }
                                                    | PlaceSegment::FixedIndex { .. }
                                            )
                                        }) =>
                                    {
                                        Some(NormalizedBound::StorageProjected {
                                            symbol,
                                            segments: segments.to_vec(),
                                        })
                                    }
                                    _ => None,
                                }
                            }),
                            [_, ..]
                                if remaining.iter().all(|segment| {
                                    matches!(
                                        segment,
                                        PlaceSegment::Field { .. }
                                            | PlaceSegment::FixedIndex { .. }
                                    )
                                }) =>
                            {
                                match normalized_bound(program, lookup, actual)? {
                                    NormalizedBound::Symbol { symbol, offset: 0 } => {
                                        Some(NormalizedBound::Projected {
                                            symbol,
                                            segments: remaining.to_vec(),
                                        })
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                    }
                }
                Self::State { .. } => normalized_bound(program, lookup, expression)
                    .or_else(|| projected_immutable_bound(program, lookup, expression)),
                Self::Domain {
                    definition,
                    subject,
                } => {
                    if exact_domain_subject_type(program, definition, expression).is_some() {
                        Some(subject.clone())
                    } else if let ExpressionNode::Integer(literal) =
                        program.expression_table.expression(expression)
                    {
                        literal.value_i64().map(NormalizedBound::Integer)
                    } else {
                        None
                    }
                }

                Self::Proposition {
                    parameters,
                    arguments,
                    ..
                } => {
                    if let Some(path) = program.expression_table.name_path_node(expression)
                        && program.expression_table.name_path_is_single_member(path)
                        && let Some(index) = state_parameter_position(parameters, path.symbol)
                    {
                        return normalized_bound(program, lookup, arguments[index]);
                    }
                    normalized_bound(program, lookup, expression)
                        .or_else(|| projected_immutable_bound(program, lookup, expression))
                        .and_then(|bound| {
                            propositions::substitute_bound(
                                program, lookup, bound, parameters, arguments,
                            )
                        })
                }
            }
        }
    }

    /*
        The bound of one receiver-rooted field projection as a stated premise
        operand. Only an immutable receiver's frozen projection identity can
        claim a stated relation; a mutable receiver's projected storage would
        assert a premise about its current contents, which stated evidence has
        no version pin for — the same contract `normalized_bound` already
        applies to whole mutable names.
    */
    fn projected_immutable_bound(
        program: &dyn ::validation::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<NormalizedBound> {
        match super::indexes::projected_bound(program, lookup, expression)? {
            bound @ NormalizedBound::Projected { .. } => Some(bound),
            _ => None,
        }
    }

    /*
        Collect the ordering premises available at one formation scope.

        Entry availability and incoming guards are shared with range checking.
        Required domain predicates additionally bind their own definition scope.
        Facts that do not decompose into builtin integer comparisons over immutable
        bounds contribute no premise; mutable storage needs version evidence first.
    */
    pub fn stated_ordering_premises<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        state: &State,
        incoming_guards: &IncomingGuardIndex,
        bound_lookup: &mut Option<ImmutableBoundLookup<'p>>,
    ) -> Vec<StatedOrderingPremise> {
        let mut premises = Vec::new();
        for (_, row) in facts.proof.contract_facts.iter() {
            if row.kind != ContractProofFactKind::Requires || row.inherited_scope.is_some() {
                continue;
            }
            if !state_requires_facts(program, machine, state).any(|candidate| candidate == row.fact)
            {
                continue;
            }
            let bound_lookup =
                bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program));
            match program.proof_facts.get(row.fact) {
                ProofFact::Expression(expression) => {
                    decompose_premise_expression(
                        program,
                        bound_lookup,
                        PremiseScope::State { machine, state },
                        *expression,
                        false,
                        &None,
                        &mut premises,
                    );
                }
                ProofFact::Membership(membership) => {
                    domains::append_membership_premises(
                        program,
                        bound_lookup,
                        machine,
                        state,
                        membership,
                        &mut premises,
                    );
                }
                ProofFact::Proposition(application) => {
                    propositions::append_proposition_premises(
                        program,
                        bound_lookup,
                        machine,
                        state,
                        application,
                        &mut premises,
                    );
                }
            }
        }
        for guard in incoming_guards
            .for_machine(machine.symbol)
            .iter()
            .filter(|guard| guard.applies_at(state.symbol))
        {
            let Some(evaluation_state) = program
                .machine_states(machine)
                .iter()
                .find(|candidate| candidate.symbol == guard.evaluation_state())
            else {
                continue;
            };
            let bindings = Some(
                program
                    .state_parameters(state)
                    .iter()
                    .filter_map(|parameter| {
                        guard
                            .immutable_argument_symbol_for_parameter(parameter.symbol)
                            .map(|argument| (parameter.symbol, argument))
                    })
                    .collect(),
            );
            decompose_premise_expression(
                program,
                bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
                PremiseScope::State {
                    machine,
                    state: evaluation_state,
                },
                guard.guard(),
                guard.is_negated(),
                &bindings,
                &mut premises,
            );
        }
        premises
    }

    /*
        Extend entry premises with completed immutable values and call guarantees
        at the exact statement entry. Later calls and invalidated captures cannot
        license earlier loan formation or mutation. The state is the one
        `guarantees` was made for, which keeps admissions across its statements.
    */
    pub(in crate::checks::borrows) fn append_statement_premises<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        statement: usize,
        guarantees: &mut StateGuarantees<'_, 'p>,
        premises: &mut Vec<StatedOrderingPremise>,
        bound_lookup: &mut Option<ImmutableBoundLookup<'p>>,
    ) {
        let state_flow = guarantees.caller();
        let Some(state) =
            find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
        else {
            return;
        };
        let Some(statement_flow) = facts
            .flow
            .control
            .statements
            .span_or_empty(state_flow.statements)
            .iter()
            .find(|row| row.statement_index == statement)
        else {
            return;
        };
        let contexts: Vec<_> = facts
            .flow
            .semantic_constraint_contexts(statement_flow.entry_constraints)
            .collect();
        stored_values::append(program, facts, state, statement, &contexts, premises);
        for guarantee in guarantees.available(
            call_guarantees::GuaranteeUse::StatementEntry(statement),
            &contexts,
        ) {
            let (result, result_mutable) = guarantee
                .result_binding(program, &facts.semantic, &contexts, state)
                .unwrap_or_default();
            decompose_premise_expression(
                program,
                bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
                PremiseScope::Call {
                    guarantee: &guarantee,
                    result,
                    result_mutable,
                },
                guarantee.expression,
                false,
                &None,
                premises,
            );
        }
    }

    /*
        Captured paths may use immutable result facts established during operand
        evaluation, never facts about the current contents of mutable storage.
        Bind results to their invocation even when a local originally received
        them: reassignment cannot retarget the captured result occurrence.
    */
    pub(super) fn captured_call_premises(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &FlowStateFact,
        statement: usize,
        contexts: &[FactContextHandle],
        frames: &CallFrameResolver<'_>,
        lookup: &ImmutableBoundLookup<'_>,
    ) -> Vec<StatedOrderingPremise> {
        let mut premises = Vec::new();
        for guarantee in call_guarantees::available(
            program,
            facts,
            state,
            call_guarantees::GuaranteeUse::EvaluatedContext(statement),
            contexts,
            frames,
        ) {
            decompose_premise_expression(
                program,
                lookup,
                PremiseScope::Call {
                    guarantee: &guarantee,
                    result: Default::default(),
                    result_mutable: false,
                },
                guarantee.expression,
                false,
                &None,
                &mut premises,
            );
        }
        let captured = |bound: &NormalizedBound| {
            !matches!(
                bound,
                NormalizedBound::Storage { .. } | NormalizedBound::StorageProjected { .. }
            )
        };
        premises.retain(|premise| captured(&premise.left) && captured(&premise.right));
        premises
    }

    /*
        Decompose one `requires` expression into atomic ordering premises.

        Conjunctions contribute each conjunct independently; `>`/`>=` normalize to
        the flipped `<`/`<=` premise so stored relations stay canonical. Every
        node is gated by its scope's builtin-meaning reader, so an overloaded
        comparison cannot masquerade as integer ordering.
    */
    fn decompose_premise_expression(
        program: &dyn ::validation::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        scope: PremiseScope<'_>,
        expression: ExpressionHandle,
        negated: bool,
        parameter_arguments: &Option<Vec<(SymbolHandle, SymbolHandle)>>,
        premises: &mut Vec<StatedOrderingPremise>,
    ) {
        if !expression.is_valid() {
            return;
        }
        let node = program.expression_table.expression(expression);
        if matches!(node, ExpressionNode::Call(_)) {
            if !negated {
                for consequence in predicate_ordering_consequences(program, expression) {
                    let operand_bound = |operand| match operand {
                        PredicateIntegerOperand::Argument(subject) => scope
                            .has_builtin_meaning(program, subject.expression)
                            .then(|| {
                                scope.projected_bound(
                                    program,
                                    lookup,
                                    subject.expression,
                                    &subject.projection,
                                )
                            })
                            .flatten(),
                        PredicateIntegerOperand::Read(read) => match &scope {
                            PremiseScope::State { machine, state } => {
                                instantiate_immutable_entry_indexed_read_identity(
                                    program,
                                    read.predicate,
                                    read.expression,
                                    &read.arguments,
                                    machine,
                                    state,
                                )
                                .map(NormalizedBound::EntryRead)
                            }
                            _ => None,
                        },
                        PredicateIntegerOperand::Integer(value) => {
                            i64::try_from(value).ok().map(NormalizedBound::Integer)
                        }
                    };
                    let (Some(left), Some(right)) = (
                        operand_bound(consequence.left),
                        operand_bound(consequence.right),
                    ) else {
                        continue;
                    };
                    use BorrowCompatibilityPremiseRelation as Relation;
                    let (relation, left, right) = match consequence.operator {
                        BinaryOperator::Less => (Relation::StrictlyBefore, left, right),
                        BinaryOperator::LessOrEqual => (Relation::LessOrEqual, left, right),
                        BinaryOperator::Greater => (Relation::StrictlyBefore, right, left),
                        BinaryOperator::GreaterOrEqual => (Relation::LessOrEqual, right, left),
                        BinaryOperator::Equal => (Relation::Equal, left, right),
                        BinaryOperator::NotEqual => (Relation::NotEqual, left, right),
                        _ => continue,
                    };
                    premises.push(StatedOrderingPremise {
                        relation,
                        left,
                        right,
                        parameter_arguments: parameter_arguments.clone(),
                    });
                }
            }
            return;
        }
        if !scope.has_builtin_meaning(program, expression) {
            return;
        }
        if let ExpressionNode::Unary(unary) = node
            && unary.operator == UnaryOperator::LogicalNot
        {
            decompose_premise_expression(
                program,
                lookup,
                scope,
                unary.operand,
                !negated,
                parameter_arguments,
                premises,
            );
            return;
        }
        let ExpressionNode::Binary(binary) = node else {
            return;
        };
        use BorrowCompatibilityPremiseRelation as Relation;
        if matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) {
            let boolean_operand = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                _ => None,
            };
            if let Some((operand, value)) = boolean_operand {
                let equality_negated = negated ^ (binary.operator == BinaryOperator::NotEqual);
                decompose_premise_expression(
                    program,
                    lookup,
                    scope,
                    operand,
                    equality_negated == value,
                    parameter_arguments,
                    premises,
                );
                return;
            }
        }
        let (relation, left, right) = match (binary.operator, negated) {
            (BinaryOperator::And, false) | (BinaryOperator::Or, true) => {
                decompose_premise_expression(
                    program,
                    lookup,
                    scope.clone(),
                    binary.left,
                    negated,
                    parameter_arguments,
                    premises,
                );
                decompose_premise_expression(
                    program,
                    lookup,
                    scope,
                    binary.right,
                    negated,
                    parameter_arguments,
                    premises,
                );
                return;
            }
            (BinaryOperator::Less, false) | (BinaryOperator::GreaterOrEqual, true) => {
                (Relation::StrictlyBefore, binary.left, binary.right)
            }
            (BinaryOperator::LessOrEqual, false) | (BinaryOperator::Greater, true) => {
                (Relation::LessOrEqual, binary.left, binary.right)
            }
            (BinaryOperator::Greater, false) | (BinaryOperator::LessOrEqual, true) => {
                (Relation::StrictlyBefore, binary.right, binary.left)
            }
            (BinaryOperator::GreaterOrEqual, false) | (BinaryOperator::Less, true) => {
                (Relation::LessOrEqual, binary.right, binary.left)
            }
            (BinaryOperator::Equal, false) | (BinaryOperator::NotEqual, true) => {
                (Relation::Equal, binary.left, binary.right)
            }
            (BinaryOperator::NotEqual, false) | (BinaryOperator::Equal, true) => {
                (Relation::NotEqual, binary.left, binary.right)
            }
            _ => return,
        };
        let (Some(left), Some(right)) = (
            scope.bound(program, lookup, left),
            scope.bound(program, lookup, right),
        ) else {
            return;
        };
        premises.push(StatedOrderingPremise {
            relation,
            left,
            right,
            parameter_arguments: parameter_arguments.clone(),
        });
    }
}
mod segments {
    use super::indexes::BorrowCompatibilityPlaceSide;
    use super::indexes::{
        EvaluatedIndexExtent, NormalizedBound, SelectorEvaluation, SelectorLocation, bound_equal,
        bound_is_at_or_before, bound_is_strictly_before, fixed_index_extent, fixed_range_extent,
        index_expression_extent, index_expressions_may_overlap, index_extents_may_overlap,
    };
    use super::premises::StatedOrderingPremise;
    use crate::flow::{place_segment_has_unresolved_identity, static_place_segment_pair_equal};
    use checked_trees::CapturedPlaceContainment;

    /*
        One segment containment traversal. Evaluated `Index` extents are cached by
        exact selector location so the equality check and both containment
        directions observe each selector's bound through one recorded value.
    */
    struct SegmentContainmentEvaluation<'program, 'session, 'premises> {
        program: &'program dyn ::validation::CheckingInput,
        selectors: &'session mut SelectorEvaluation<'premises>,
        extents: Vec<(SelectorLocation, EvaluatedIndexExtent)>,
        bound_lookup: &'session mut Option<validation::ImmutableBoundLookup<'program>>,
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
            let extent = index_expression_extent(
                self.program,
                expression,
                location,
                self.selectors,
                self.bound_lookup,
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

        /*
            Whether two segments at the same path position provably denote the same
            extent. Structural identities compare directly; `Index` selectors
            compare their normalized bounds.
        */
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

        /*
            Whether `container` is provably a prefix selector of `contained`:
            every container segment contains the segment at the same position.
        */
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
                    let container_extent =
                        self.index_extent(container_expression, container_location);
                    let contained_extent =
                        self.index_extent(contained_expression, contained_location);
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

    /*
        Whether one evaluated `Index` extent provably contains another: a window
        contains a nested window or a bounded point, while a point only contains
        an equal point. Unknown bounds stay unproven in both directions.
    */
    fn index_extent_contains(
        container: EvaluatedIndexExtent,
        contained: EvaluatedIndexExtent,
        selectors: &mut SelectorEvaluation<'_>,
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
                    (Some(container), Some(contained)) => {
                        bound_equal(container, contained, selectors)
                    }
                    _ => false,
                }
            }
            (EvaluatedIndexExtent::Point(_), EvaluatedIndexExtent::Window { .. }) => false,
        }
    }

    /*
        Evaluated-extent equality for `Index` segments: equal point bounds, or
        pairwise-equal window bounds. Unlike containment, equality does not require
        the window to be provably non-empty -- two equally unknown-empty windows
        still denote the same extent.
    */
    fn index_extents_equal(
        left: EvaluatedIndexExtent,
        right: EvaluatedIndexExtent,
        selectors: &mut SelectorEvaluation<'_>,
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

    /*
        `[container_start, container_end)` contains `point`. All three bounds must
        be known and ordered.
    */
    fn index_window_contains_point(
        container_start: Option<NormalizedBound>,
        container_end: Option<NormalizedBound>,
        point: Option<NormalizedBound>,
        selectors: &mut SelectorEvaluation<'_>,
    ) -> bool {
        match (container_start, container_end, point) {
            (Some(start), Some(end), Some(point)) => {
                bound_is_at_or_before(start, point.clone(), selectors)
                    && bound_is_strictly_before(point, end, selectors)
            }
            _ => false,
        }
    }

    /*
        `[container_start, container_end)` contains `[contained_start,
        contained_end)`. Mirroring the `FixedRange` rule, the contained window must
        be provably non-empty and nest inside the container. The container's own
        non-emptiness is then implied by the ordering chain.
    */
    fn index_window_contains_window(
        container_start: Option<NormalizedBound>,
        container_end: Option<NormalizedBound>,
        contained_start: Option<NormalizedBound>,
        contained_end: Option<NormalizedBound>,
        selectors: &mut SelectorEvaluation<'_>,
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

    fn place_segments_containment_evaluated<'p>(
        program: &'p dyn ::validation::CheckingInput,
        left: &[facts::PlaceSegment],
        right: &[facts::PlaceSegment],
        selectors: &mut SelectorEvaluation<'_>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
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
            bound_lookup,
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
        static_place_segment_pair_equal(left, right).unwrap_or(match (left, right) {
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
            _ => false,
        })
    }

    /*
        Share evaluated bounds between overlap and containment. A disjoint result
        already fixes containment to None and needs no further evaluation.
    */
    pub(super) fn place_segments_compatibility<'p>(
        program: &'p dyn ::validation::CheckingInput,
        left: &[facts::PlaceSegment],
        right: &[facts::PlaceSegment],
        premises: &[StatedOrderingPremise],
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> (bool, CapturedPlaceContainment) {
        let mut selectors = SelectorEvaluation::new(premises);
        let may_overlap = place_segments_may_overlap_evaluated(
            program,
            left,
            right,
            &mut selectors,
            bound_lookup,
        );
        let containment = if may_overlap {
            place_segments_containment_evaluated(program, left, right, &mut selectors, bound_lookup)
        } else {
            CapturedPlaceContainment::None
        };
        (may_overlap, containment)
    }

    fn place_segments_may_overlap_evaluated<'p>(
        program: &'p dyn ::validation::CheckingInput,
        left: &[facts::PlaceSegment],
        right: &[facts::PlaceSegment],
        selectors: &mut SelectorEvaluation<'_>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> bool {
        for (segment_index, (&left_segment, &right_segment)) in left.iter().zip(right).enumerate() {
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
                bound_lookup,
            ) {
                return false;
            }
        }
        true
    }

    fn place_segment_pair_may_overlap<'p>(
        program: &'p dyn ::validation::CheckingInput,
        left: facts::PlaceSegment,
        right: facts::PlaceSegment,
        segment_index: usize,
        selectors: &mut SelectorEvaluation<'_>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> bool {
        let left_location = SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index,
        };
        let right_location = SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Active,
            segment_index,
        };
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
                index_expression_extent(
                    program,
                    expression,
                    right_location,
                    selectors,
                    bound_lookup,
                ),
                selectors,
            ),
            (
                facts::PlaceSegment::Index { expression },
                facts::PlaceSegment::FixedRange { start, end },
            ) => index_extents_may_overlap(
                index_expression_extent(
                    program,
                    expression,
                    left_location,
                    selectors,
                    bound_lookup,
                ),
                fixed_range_extent(start, end),
                selectors,
            ),
            (
                facts::PlaceSegment::FixedIndex { index },
                facts::PlaceSegment::Index { expression },
            ) => index_extents_may_overlap(
                fixed_index_extent(index),
                index_expression_extent(
                    program,
                    expression,
                    right_location,
                    selectors,
                    bound_lookup,
                ),
                selectors,
            ),
            (
                facts::PlaceSegment::Index { expression },
                facts::PlaceSegment::FixedIndex { index },
            ) => index_extents_may_overlap(
                index_expression_extent(
                    program,
                    expression,
                    left_location,
                    selectors,
                    bound_lookup,
                ),
                fixed_index_extent(index),
                selectors,
            ),
            (
                facts::PlaceSegment::Index {
                    expression: left_expression,
                },
                facts::PlaceSegment::Index {
                    expression: right_expression,
                },
            ) => index_expressions_may_overlap(
                program,
                left_expression,
                left_location,
                right_expression,
                right_location,
                selectors,
                bound_lookup,
            ),
            _ => false,
        })
    }
}

pub(super) use self::premises::append_statement_premises;
pub(super) use self::premises::{StatedOrderingPremise, stated_ordering_premises};
use self::segments::place_segments_compatibility;
use crate::flow::expressions;

/*
    Produced evaluation facts shared by source debt and executable planning.
    Borrow only these fields so recording cleanup invocations stays independent.
*/
#[derive(Clone, Copy)]
pub(crate) struct StorageSelectorFacts<'facts> {
    pub flow: checked_trees::FlowFactsRef<'facts>,
    pub semantic: &'facts facts::FactPlan,
}

/*
    Compare captured storage paths without assuming that current mutable
    selector storage still holds an earlier value. Immutable values and exact
    constants retain identity across statements; mutable query coordinates
    cannot establish containment without version-pinned premises. Window debt
    uses this same spatial judgment as loans, but supplies no current premises
    as evidence about the opening occurrence. Detached selector copies obtain
    equality only from their retained reaching values and write-prefix epochs;
    both source debt and executable planning consume that same relation.
*/
pub(crate) fn storage_path_compatibility(
    program: &dyn ::validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> (bool, checked_trees::CapturedPlaceContainment) {
    let lookup = validation::ImmutableBoundLookup::new(program);
    let premises = captured_values::equalities(program, captures, left, right, &lookup);
    place_segments_compatibility(program, left, right, &premises, &mut Some(lookup))
}

/*
    The storage-transfer producer freezes a known selector once, before any
    custody consumer records exclusions. The proof is the same completed-value
    observation used by spatial equality, never a current mutable query.
*/
pub(crate) fn captured_storage_index(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    lookup: &validation::ImmutableBoundLookup<'_>,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    captured_values::captured_storage_index(program, captures, lookup, expression)
}

/*
    Cleanup must separate its selected receiver from moved storage in every
    incoming occurrence. Only immutable captured-value relations supplement the
    context-free spatial judgment; current mutable selector facts cannot do so.
*/
pub(crate) fn captured_storage_paths_are_disjoint(
    program: &dyn ::validation::CheckingInput,
    facts: checked_trees::CheckFactsRef<'_>,
    state: &checked_trees::FlowStateFact,
    contexts: &[(usize, Vec<facts::FactContextHandle>)],
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let mut lookup = Some(validation::ImmutableBoundLookup::new(program));
    let captures = StorageSelectorFacts {
        flow: facts.flow,
        semantic: facts.semantic,
    };
    let Some(frames) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    !contexts.is_empty()
        && contexts.iter().all(|(statement, contexts)| {
            let bound_lookup =
                lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
            let mut premises =
                captured_values::equalities(program, captures, left, right, bound_lookup);
            premises.extend(self::premises::captured_call_premises(
                program,
                facts,
                state,
                *statement,
                contexts,
                &frames,
                bound_lookup,
            ));
            !place_segments_compatibility(program, left, right, &premises, &mut lookup).0
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn captured_place_compatibility<'p>(
    program: &'p dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    left: &checked_trees::CapturedPlace,
    left_access: &checked_trees::BorrowAccessKind,
    right: &checked_trees::CapturedPlace,
    right_access: &checked_trees::BorrowAccessKind,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> checked_trees::CapturedPlaceCompatibility {
    let receiver_namespace = receiver_namespace_places(program, left, right);
    let (left_place, right_place) = receiver_namespace
        .as_ref()
        .map_or((left, right), |(left, right)| (left, right));
    let authenticated = |place: &checked_trees::CapturedPlace| {
        !matches!(place.root, facts::PlaceRoot::InvocationOutput(_))
            || place.incoming_reference.is_valid()
    };
    let roots_valid = left_place.root.is_valid()
        && right_place.root.is_valid()
        && authenticated(left_place)
        && authenticated(right_place);
    let same_family = roots_valid
        && left_place.root == right_place.root
        && left_place.incoming_reference == right_place.incoming_reference;
    let descriptor_load = |place: &checked_trees::CapturedPlace| {
        borrow
            .incoming_references
            .is_valid(place.incoming_reference)
            && matches!(
                &borrow
                    .incoming_references
                    .get(place.incoming_reference)
                    .origin,
                checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
            )
    };
    let has_descriptor_load = descriptor_load(left_place) || descriptor_load(right_place);
    let (owners_overlap, owner_containment) = if same_family {
        place_segments_compatibility(
            program,
            &left_place.incoming_owner_path,
            &right_place.incoming_owner_path,
            premises,
            bound_lookup,
        )
    } else {
        (false, checked_trees::CapturedPlaceContainment::None)
    };
    let same_root =
        same_family && owner_containment == checked_trees::CapturedPlaceContainment::Same;
    let (segments_may_overlap, containment) = if same_root {
        place_segments_compatibility(
            program,
            &left_place.segments,
            &right_place.segments,
            premises,
            bound_lookup,
        )
    } else {
        (
            same_family && owners_overlap,
            checked_trees::CapturedPlaceContainment::None,
        )
    };
    let independent_input_permissions = roots_valid
        && !has_descriptor_load
        && (left_place.incoming_reference.is_valid() || right_place.incoming_reference.is_valid())
        && (!same_family || !owners_overlap);
    let disjoint = roots_valid
        && !independent_input_permissions
        && (!has_descriptor_load || same_family)
        && (!same_family || !segments_may_overlap);
    let shares_dependent_fact = same_root
        && place_segments_share_dependent_fact(
            program,
            &left_place.segments,
            &right_place.segments,
        );
    let both_shared = matches!(left_access, checked_trees::BorrowAccessKind::Read)
        && matches!(right_access, checked_trees::BorrowAccessKind::Read);

    checked_trees::CapturedPlaceCompatibility {
        left: left.clone(),
        right: right.clone(),
        disjoint,
        containment,
        non_interfering: both_shared
            || independent_input_permissions
            || (disjoint && !shares_dependent_fact),
    }
}

/*
    Lifetime retention can forbid an access without identifying the saved
    pointer's address. Return only interference: input windows cannot supply
    containment, a lending parent, or any other spatial authority.
*/
pub(super) fn retained_debt_non_interfering<'p>(
    program: &'p dyn validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
    place: &checked_trees::CapturedPlace,
    access: &checked_trees::BorrowAccessKind,
    premises: &[StatedOrderingPremise],
    lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> Option<bool> {
    let windows = super::resources::supplied_permissions::debt_windows(borrow, loan)?;
    let incoming_place = if !place.incoming_reference.is_valid() {
        place.root.as_symbol().and_then(|symbol| {
            let state_symbol = program.symbols.get(symbol).parent;
            let state = program.state_by_symbol(state_symbol)?;
            let parameter = program.state_parameter_by_symbol(state, symbol)?;
            let shell = validation::reference_shell(program, parameter.type_reference)?;
            if !shell.access.is_exclusive() { return None; }
            let (owner, _) = validation::find_state(program, state_symbol)?;
            let (_, scope) = borrow.first_state_borrow(owner.symbol, state_symbol)?;
            let mut identities = scope.incoming_references.handles().filter(|identity| {
                borrow.state_owns_incoming_reference(scope, *identity)
                    && matches!(borrow.incoming_references.get(*identity).origin,
                        checked_trees::IncomingReferenceOrigin::ParameterReferent { symbol: supplied }
                            if supplied == symbol)
            });
            let identity = identities.next()?;
            if identities.next().is_some() { return None; }
            Some(checked_trees::CapturedPlace { incoming_reference: identity, ..place.clone() })
        })
    } else {
        None
    };
    let place = incoming_place.as_ref().unwrap_or(place);
    Some(windows.is_ok_and(|windows| {
        windows.iter().all(|window| {
            captured_place_compatibility(
                program,
                borrow,
                place,
                access,
                &window.place,
                &window.access,
                premises,
                lookup,
            )
            .non_interfering
        })
    }))
}

pub(super) fn canonical_place_non_interfering<'p>(
    program: &'p dyn validation::CheckingInput,
    place: &validation::CanonicalPlace,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
    borrow: &checked_trees::BorrowFacts,
    premises: &[StatedOrderingPremise],
    lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> bool {
    retained_debt_non_interfering(
        program,
        borrow,
        loan,
        &checked_trees::CapturedPlace {
            root: place.root,
            segments: place.segments.clone(),
            ..Default::default()
        },
        &checked_trees::BorrowAccessKind::Mutable,
        premises,
        lookup,
    )
    .unwrap_or_else(|| {
        canonical_place_loan_compatibility(
            program,
            place,
            borrow.loans.get(loan),
            borrow,
            premises,
            lookup,
        )
        .non_interfering
    })
}

/*
    Borrow access places capture a direct `self.field` at the attached field's
    own root, while a bare `self` operand captures the machine root: the whole
    receiver. When one place is rooted at an attached machine and the other at
    one of that machine's attached fields, re-root the field place under the
    machine so both compare in the receiver's namespace. Any other pair keeps
    its own roots.
*/
fn receiver_namespace_places(
    program: &dyn ::validation::CheckingInput,
    left: &checked_trees::CapturedPlace,
    right: &checked_trees::CapturedPlace,
) -> Option<(checked_trees::CapturedPlace, checked_trees::CapturedPlace)> {
    if !left.root.is_valid()
        || !right.root.is_valid()
        || left.incoming_reference.is_valid()
        || right.incoming_reference.is_valid()
        || left.root == right.root
    {
        return None;
    }
    if let Some(right) = attached_field_place_under_receiver(program, left, right) {
        return Some((left.clone(), right));
    }
    attached_field_place_under_receiver(program, right, left).map(|left| (left, right.clone()))
}

fn attached_field_place_under_receiver(
    program: &dyn ::validation::CheckingInput,
    receiver: &checked_trees::CapturedPlace,
    field_place: &checked_trees::CapturedPlace,
) -> Option<checked_trees::CapturedPlace> {
    let machine = program.machine_by_symbol(receiver.root.as_symbol()?)?;
    let field_symbol = field_place.root.as_symbol()?;
    let field = validation::exact_attached_field(
        program,
        machine,
        field_symbol,
        program.symbols.name(field_symbol),
    )?;
    let mut segments = Vec::with_capacity(field_place.segments.len() + 1);
    segments.push(facts::PlaceSegment::Field {
        symbol: field.symbol,
    });
    segments.extend_from_slice(&field_place.segments);
    Some(checked_trees::CapturedPlace {
        root: receiver.root,
        segments,
        ..Default::default()
    })
}

/*
    Re-roots a canonical write/access place against the loan's root exactly as
    `canonical_place_loan_compatibility` judges the pair. Returns `None` for
    roots that cannot identify storage at all.
*/
pub(super) fn canonical_place_for_loan(
    place: &validation::CanonicalPlace,
    loan: &checked_trees::BorrowLoanFact,
) -> Option<checked_trees::CapturedPlace> {
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if facts::PlaceRoot::Symbol(symbol) == loan.root {
                Some(checked_trees::CapturedPlace {
                    root: place.root,
                    segments: place.segments.clone(),
                    ..Default::default()
                })
            } else {
                match place.segments.split_first() {
                    Some((
                        facts::PlaceSegment::Field {
                            symbol: field_symbol,
                        },
                        remaining,
                    )) if facts::PlaceRoot::Symbol(*field_symbol) == loan.root => {
                        Some(checked_trees::CapturedPlace {
                            root: loan.root,
                            segments: remaining.to_vec(),
                            ..Default::default()
                        })
                    }
                    Some((
                        facts::PlaceSegment::Case { .. },
                        [
                            facts::PlaceSegment::Field {
                                symbol: field_symbol,
                            },
                            remaining @ ..,
                        ],
                    )) if facts::PlaceRoot::Symbol(*field_symbol) == loan.root => {
                        Some(checked_trees::CapturedPlace {
                            root: loan.root,
                            segments: remaining.to_vec(),
                            ..Default::default()
                        })
                    }
                    _ => Some(checked_trees::CapturedPlace {
                        root: place.root,
                        segments: place.segments.clone(),
                        ..Default::default()
                    }),
                }
            }
        }
        facts::PlaceRoot::Expression(_) => Some(checked_trees::CapturedPlace {
            root: place.root,
            segments: place.segments.clone(),
            ..Default::default()
        }),
        facts::PlaceRoot::Unknown
        | facts::PlaceRoot::TypeReference(_)
        | facts::PlaceRoot::InvocationReferent { .. }
        | facts::PlaceRoot::InvocationOutput(_) => None,
    }
}

pub(super) fn canonical_place_loan_compatibility<'p>(
    program: &'p dyn ::validation::CheckingInput,
    place: &validation::CanonicalPlace,
    loan: &checked_trees::BorrowLoanFact,
    borrow: &checked_trees::BorrowFacts,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> checked_trees::CapturedPlaceCompatibility {
    let right = captured_loan_place(borrow, loan);
    let Some(left) = canonical_place_for_loan(place, loan) else {
        return checked_trees::CapturedPlaceCompatibility {
            right,
            ..Default::default()
        };
    };
    captured_place_compatibility(
        program,
        borrow,
        &left,
        &checked_trees::BorrowAccessKind::Mutable,
        &right,
        &loan.kind,
        premises,
        bound_lookup,
    )
}

fn captured_loan_place(
    borrow: &checked_trees::BorrowFacts,
    loan: &checked_trees::BorrowLoanFact,
) -> checked_trees::CapturedPlace {
    checked_trees::CapturedPlace {
        incoming_reference: loan.incoming_reference,
        incoming_owner_path: borrow
            .access_segments
            .span_or_empty(loan.incoming_owner_path)
            .to_vec(),
        root: loan.root,
        segments: borrow.loan_segments(loan).to_vec(),
    }
}

/*
    A borrow of one field participating in a dependent-data fact pins the
    sibling fields named by that SAME fact. A direct structural overlap is not
    required: mutating the sibling would invalidate the relation that makes the
    borrowed projection meaningful while it remains live.
*/
fn place_segments_share_dependent_fact(
    program: &dyn ::validation::CheckingInput,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let divergence = left
        .iter()
        .zip(right)
        .position(|(left, right)| left != right);
    let Some(divergence) = divergence else {
        return false;
    };
    let (
        facts::PlaceSegment::Field { symbol: left_field },
        facts::PlaceSegment::Field {
            symbol: right_field,
        },
    ) = (left[divergence], right[divergence])
    else {
        return false;
    };

    if !left_field.is_valid() || !right_field.is_valid() {
        return false;
    }
    let parent = program.symbols.get(left_field).parent;
    if parent != program.symbols.get(right_field).parent {
        return false;
    }
    let Some(definition) = program.data_definition_by_symbol(parent) else {
        return false;
    };
    let Some(left_name) = data_field_name(program, definition, left_field) else {
        return false;
    };
    let Some(right_name) = data_field_name(program, definition, right_field) else {
        return false;
    };

    program
        .proof_facts
        .span_or_empty(definition.requires_facts)
        .iter()
        .any(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                expression_mentions_field(program, *expression, left_name)
                    && expression_mentions_field(program, *expression, right_name)
            }
            typed_trees::domain::ProofFact::Membership(_) => false,
            typed_trees::domain::ProofFact::Proposition(_) => false,
        })
}

fn data_field_name<'a>(
    program: &'a dyn ::validation::CheckingInput,
    definition: &'a typed_trees::data::DataDefinition,
    symbol: symbols::SymbolHandle,
) -> Option<&'a str> {
    program
        .data_field(definition, symbol)
        .map(|field| field.name.as_str())
}

fn expression_mentions_field(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    field: &str,
) -> bool {
    use typed_trees::expression::ExpressionNode;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => expressions(program)
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == field),
        ExpressionNode::Binary(binary) => {
            expression_mentions_field(program, binary.left, field)
                || expression_mentions_field(program, binary.right, field)
        }
        ExpressionNode::Member(member) => {
            member.member.as_str() == field
                || expression_mentions_field(program, member.receiver, field)
        }
        ExpressionNode::Borrow(inner) => expression_mentions_field(program, inner.target, field),
        _ => false,
    }
}

pub(super) fn borrow_loan_compatibility<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    left: &checked_trees::BorrowLoanFact,
    right: &checked_trees::BorrowLoanFact,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> checked_trees::CapturedPlaceCompatibility {
    captured_place_compatibility(
        program,
        &facts.borrow,
        &captured_loan_place(&facts.borrow, left),
        &left.kind,
        &captured_loan_place(&facts.borrow, right),
        &right.kind,
        premises,
        bound_lookup,
    )
}
