use facts::FactPlan;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use validation::{
    CanonicalPlace, canonical_place_from_expression, canonical_place_from_expression_in_state,
    effective_member_symbol,
};

/// Narrow each runtime `Index` segment whose selector value is pinned in the
/// live contexts to the `FixedIndex` it names at this program point.
///
/// A selector the scalar facts cannot pin stays an `Index` segment, so the
/// overlap predicates keep their conservative may-alias treatment. This is
/// the same narrowing `transfers::write_target` applies to a write target:
/// sharing it keeps the invalidation view of a runtime index identical to the
/// establishment view, instead of the write landing on a fixed element while
/// retiring facts at every element.
pub(crate) fn narrow_pinned_index_segments(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    state_symbol: SymbolHandle,
    statement_index: usize,
    context_handles: &[facts::FactContextHandle],
    segments: &mut [facts::PlaceSegment],
) {
    for segment in segments.iter_mut() {
        let facts::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        let Some(selector) = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        ) else {
            continue;
        };
        let Some(facts::ScalarValue::Integer(index)) = crate::values::scalar_value_at_place(
            program,
            semantic,
            context_handles
                .iter()
                .map(|handle| semantic.contexts.get(*handle)),
            &selector,
        ) else {
            continue;
        };
        if let Some(index) = index.to_u64().and_then(|index| usize::try_from(index).ok()) {
            *segment = facts::PlaceSegment::FixedIndex { index };
        }
    }
}

/// Compose the collection coordinates of one captured address. A view keeps
/// its collection dimension, so indexing a fixed window adds its base rather
/// than descending into another array. These are address projections, not the
/// pointwise range sets used to summarize effects or residual cleanup.
pub(crate) fn compose_captured_view_segments(
    segments: &mut Vec<facts::PlaceSegment>,
) -> Option<()> {
    use facts::PlaceSegment;
    let mut composed = Vec::with_capacity(segments.len());
    for &segment in segments.iter() {
        let combined = match (composed.last().copied(), segment) {
            (Some(PlaceSegment::FixedRange { start, end }), PlaceSegment::FixedIndex { index }) => {
                if index >= end.checked_sub(start)? {
                    return None;
                }
                Some(PlaceSegment::FixedIndex {
                    index: start.checked_add(index)?,
                })
            }
            (
                Some(PlaceSegment::FixedRange { start, end }),
                PlaceSegment::FixedRange {
                    start: next_start,
                    end: next_end,
                },
            ) => {
                if next_start > next_end || next_end > end.checked_sub(start)? {
                    return None;
                }
                Some(PlaceSegment::FixedRange {
                    start: start.checked_add(next_start)?,
                    end: start.checked_add(next_end)?,
                })
            }
            _ => None,
        };
        if let Some(segment) = combined {
            *composed.last_mut()? = segment;
        } else {
            composed.push(segment);
        }
    }
    *segments = composed;
    Some(())
}

pub(crate) fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
    symbol.is_valid().then_some(CanonicalPlace {
        root: facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    })
}

pub(crate) fn canonical_place_from_semantic_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    place: &facts::Place,
) -> Option<CanonicalPlace> {
    let mut canonical = match place.root {
        facts::PlaceRoot::Unknown => return None,
        facts::PlaceRoot::Symbol(symbol) => canonical_place_from_symbol(symbol)?,
        facts::PlaceRoot::Expression(expression) => {
            canonical_place_from_expression(program, expression)?
        }
        facts::PlaceRoot::TypeReference(type_reference) => CanonicalPlace {
            root: facts::PlaceRoot::TypeReference(type_reference),
            segments: Vec::new(),
        },
    };
    canonical.extend_segments(semantic.place_segments.span_or_empty(place.segments));
    Some(canonical)
}

/// The contract `field_actuals::checked_place` states for a requires leaf:
/// the receiver chain must stay acyclic and every retained member selection
/// must still be the member its expression resolves to, or the place it
/// yields would read evidence the source did not name.
pub(crate) fn projected_leaf_place(
    program: &dyn ::validation::CheckingInput,
    leaf: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let mut current = leaf;
    let mut visited = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(current) || visited.contains(&current) {
            return None;
        }
        visited.push(current);
        current = match program.expression_table.expression(current) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Name(name) => {
                if !name.symbol.is_valid()
                    || !name.head_symbol.is_valid()
                    || program
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        .first()
                        .is_some_and(|symbol| *symbol != name.head_symbol)
                    || matches!(
                        program.expression_table.name_path_members(name.members),
                        [_]
                    ) && name.symbol != name.head_symbol
                {
                    return None;
                }
                break;
            }
            _ => break,
        };
    }
    for expression in visited.into_iter().rev() {
        if let Some(member) = program.expression_table.member_node(expression)
            && (!member.member_symbol.is_valid()
                || effective_member_symbol(program, member.receiver, member)
                    != member.member_symbol)
        {
            return None;
        }
    }
    let place = canonical_place_from_expression(program, leaf)?;
    (!place
        .segments
        .iter()
        .any(|segment| super::comparison::place_segment_has_unresolved_identity(*segment)))
    .then_some(place)
}

/// The canonical place a source-independent expression names: a valid symbol
/// root, and every segment a statically known field, case, or literal index —
/// computed indices keep the place source-dependent.
pub(crate) fn source_independent_place(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let place = canonical_place_from_expression(program, expression)?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || !place.segments.iter().all(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            facts::PlaceSegment::Case { variant } => variant.is_valid(),
            facts::PlaceSegment::FixedIndex { .. } => true,
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::Index { .. } => false,
        })
    {
        return None;
    }
    Some(place)
}

/// The referent place a reference initializer captures: explicit borrows and
/// whole-place recasts unwrap to their target, and a plain place expression
/// names the place the new local aliases.
pub(crate) fn reference_source_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    initializer: ExpressionHandle,
) -> Option<CanonicalPlace> {
    reference_source_place_shaped(program, state_symbol, statement_index, initializer, false)
}

/// The same referent place through collection-view locals: a
/// `let v = xs.as_mut_slice()` alias refers to the receiver collection's
/// storage — writes through `v` name the same place as the receiver's own.
pub(crate) fn reference_source_place_through_collection_views(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    initializer: ExpressionHandle,
) -> Option<CanonicalPlace> {
    reference_source_place_shaped(program, state_symbol, statement_index, initializer, true)
}

fn reference_source_place_shaped(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    initializer: ExpressionHandle,
    through_collection_views: bool,
) -> Option<CanonicalPlace> {
    let mut expression = initializer;
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => expression = cast.value,
            ExpressionNode::Call(call)
                if through_collection_views
                    && matches!(
                        validation::collection_operations::collection_view_call(program, call),
                        Some(
                            language_semantics::declaration_selection::CollectionViewOperation::SharedSlice
                                | language_semantics::declaration_selection::CollectionViewOperation::MutableSlice
                        )
                    ) =>
            {
                expression = call.receiver;
            }
            _ => break,
        }
    }
    canonical_place_from_expression_in_state(program, state_symbol, statement_index, expression)
}
