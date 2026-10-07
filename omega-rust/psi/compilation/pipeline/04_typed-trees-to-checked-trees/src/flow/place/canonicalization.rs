use super::contextual::contextual_canonical_place_from_expression;
use super::resolution::effective_member_symbol;
use crate::flow::CanonicalPlace;
use crate::lookup::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use facts::FactPlan;
use symbols::SymbolHandle;

pub(crate) fn index_place_segment(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> facts::PlaceSegment {
    if let ExpressionNode::Range(range) = program.expression_table.expression(expression)
        && let Some((start, end)) = fixed_half_open_range(program, range)
    {
        return facts::PlaceSegment::FixedRange { start, end };
    }
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
        .map(|index| facts::PlaceSegment::FixedIndex { index })
        .unwrap_or(facts::PlaceSegment::Index { expression })
}

/// Normalize an expression-only range when its half-open bounds do not need
/// collection metadata. An omitted start is zero; an omitted end remains
/// dynamic here because the collection length is resolved by the range checker,
/// not by the place algebra. Inclusive syntax is normalized once by advancing
/// its last included ordinal.
fn fixed_half_open_range(
    program: &typed_trees::TypedTrees,
    range: &typed_trees::expression::TableRangeExpression,
) -> Option<(usize, usize)> {
    let start = if range.start.is_valid() {
        usize::try_from(
            program
                .expression_table
                .constant_integer_value(range.start)?,
        )
        .ok()?
    } else {
        0
    };
    if !range.end.is_valid() {
        return None;
    }
    let end = usize::try_from(program.expression_table.constant_integer_value(range.end)?).ok()?;
    let end = if range.end_inclusive {
        end.checked_add(1)?
    } else {
        end
    };
    Some((start, end))
}

pub(crate) fn push_field_place_segments(
    program: &typed_trees::TypedTrees,
    segments: &mut Vec<facts::PlaceSegment>,
    symbol: SymbolHandle,
) {
    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
        segments.push(facts::PlaceSegment::Case { variant });
    }
    segments.push(facts::PlaceSegment::Field { symbol });
}

pub(crate) fn canonical_place_from_expression(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => canonical_place_from_expression(program, inner.target),
        // A borrow recast re-views the operand's bytes at the same address:
        // the recast place is the operand's place. Value casts compute a new
        // rvalue and keep the expression root.
        ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            canonical_place_from_expression(program, cast.value)
        }
        ExpressionNode::Name(path) => {
            let root_symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            let mut place = CanonicalPlace {
                root: facts::PlaceRoot::Symbol(root_symbol),
                segments: Vec::new(),
            };
            let members = program.expression_table.name_path_members(path.members);
            for (offset, retained) in program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .skip(1)
                .copied()
                .enumerate()
            {
                let symbol = members
                    .get(offset + 1)
                    .and_then(|member| {
                        super::contextual::resolve_member_symbol_from_place(
                            program,
                            &place,
                            member.as_str(),
                            None,
                            retained,
                        )
                    })
                    .unwrap_or(retained);
                push_field_place_segments(program, &mut place.segments, symbol);
            }
            Some(place)
        }
        ExpressionNode::Member(member) => {
            let mut place = canonical_place_from_expression(program, member.receiver)?;
            let symbol = effective_member_symbol(program, member.receiver, member);
            push_field_place_segments(program, &mut place.segments, symbol);
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place = canonical_place_from_expression(program, indexed.collection)?;
            place
                .segments
                .push(index_place_segment(program, indexed.index));
            Some(place)
        }
        _ => Some(CanonicalPlace {
            root: facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        }),
    }
}

pub(crate) fn canonical_place_from_expression_in_state(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    contextual_canonical_place_from_expression(program, state_symbol, statement_index, expression)
        .or_else(|| canonical_place_from_expression(program, expression))
}

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
    program: &typed_trees::TypedTrees,
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

pub(crate) fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
    symbol.is_valid().then_some(CanonicalPlace {
        root: facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    })
}

pub(crate) fn canonical_place_from_semantic_place(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
        if let ExpressionNode::Member(member) = program.expression_table.expression(expression)
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
