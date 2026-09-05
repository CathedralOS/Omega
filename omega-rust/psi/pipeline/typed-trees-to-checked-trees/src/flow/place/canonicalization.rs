use super::contextual::contextual_canonical_place_from_expression;
use super::resolution::effective_member_symbol;
use super::*;
use crate::lookup::first_valid_name_path_symbol;

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
        ExpressionNode::Name(path) => {
            let root_symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            let mut segments = Vec::new();
            for symbol in program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .skip(1)
                .copied()
            {
                push_field_place_segments(program, &mut segments, symbol);
            }
            Some(CanonicalPlace {
                root: facts::PlaceRoot::Symbol(root_symbol),
                segments,
            })
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
