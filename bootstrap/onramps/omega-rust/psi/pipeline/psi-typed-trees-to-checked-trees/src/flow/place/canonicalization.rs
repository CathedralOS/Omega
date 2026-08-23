use super::contextual::contextual_canonical_place_from_expression;
use super::resolution::effective_member_symbol;
use super::*;
use crate::lookup::first_valid_name_path_symbol;

pub(crate) fn index_place_segment(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> psi_facts::PlaceSegment {
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
        .map(|index| psi_facts::PlaceSegment::FixedIndex { index })
        .unwrap_or(psi_facts::PlaceSegment::Index { expression })
}

pub(crate) fn push_field_place_segments(
    program: &psi_typed_trees::TypedTrees,
    segments: &mut Vec<psi_facts::PlaceSegment>,
    symbol: SymbolHandle,
) {
    if let Some(variant) = psi_facts::payload_variant_for_field(program, symbol) {
        segments.push(psi_facts::PlaceSegment::Case { variant });
    }
    segments.push(psi_facts::PlaceSegment::Field { symbol });
}

pub(crate) fn canonical_place_from_expression(
    program: &psi_typed_trees::TypedTrees,
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
                root: psi_facts::PlaceRoot::Symbol(root_symbol),
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
            root: psi_facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        }),
    }
}

pub(crate) fn canonical_place_from_expression_in_state(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    contextual_canonical_place_from_expression(program, state_symbol, statement_index, expression)
        .or_else(|| canonical_place_from_expression(program, expression))
}

pub(crate) fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
    symbol.is_valid().then_some(CanonicalPlace {
        root: psi_facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    })
}

pub(crate) fn canonical_place_from_semantic_place(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &psi_facts::Place,
) -> Option<CanonicalPlace> {
    let mut canonical = match place.root {
        psi_facts::PlaceRoot::Unknown => return None,
        psi_facts::PlaceRoot::Symbol(symbol) => canonical_place_from_symbol(symbol)?,
        psi_facts::PlaceRoot::Expression(expression) => {
            canonical_place_from_expression(program, expression)?
        }
        psi_facts::PlaceRoot::TypeReference(type_reference) => CanonicalPlace {
            root: psi_facts::PlaceRoot::TypeReference(type_reference),
            segments: Vec::new(),
        },
    };
    canonical.extend_segments(semantic.place_segments.span_or_empty(place.segments));
    Some(canonical)
}
