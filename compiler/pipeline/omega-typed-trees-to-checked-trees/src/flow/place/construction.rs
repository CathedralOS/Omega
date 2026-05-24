use super::*;
use crate::lookup::first_valid_name_path_symbol;
use resolution::effective_member_symbol;

pub(crate) fn canonical_place_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => canonical_place_from_expression(program, *inner),
        ExpressionNode::Name(path) => {
            let root_symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            let segments = program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .skip(1)
                .copied()
                .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                .collect();
            Some(CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(root_symbol),
                segments,
            })
        }
        ExpressionNode::Member(member) => {
            let mut place = canonical_place_from_expression(program, member.receiver)?;
            place.segments.push(omega_facts::PlaceSegment::Field {
                symbol: effective_member_symbol(program, member.receiver, member),
            });
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place = canonical_place_from_expression(program, indexed.collection)?;
            place.segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(place)
        }
        _ => Some(CanonicalPlace {
            root: omega_facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        }),
    }
}

pub(crate) fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
    symbol.is_valid().then_some(CanonicalPlace {
        root: omega_facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    })
}

pub(crate) fn canonical_place_from_semantic_place(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> Option<CanonicalPlace> {
    let mut canonical = match place.root {
        omega_facts::PlaceRoot::Unknown => return None,
        omega_facts::PlaceRoot::Symbol(symbol) => canonical_place_from_symbol(symbol)?,
        omega_facts::PlaceRoot::Expression(expression) => {
            canonical_place_from_expression(program, expression)?
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => CanonicalPlace {
            root: omega_facts::PlaceRoot::TypeReference(type_reference),
            segments: Vec::new(),
        },
    };
    canonical.extend_segments(semantic.place_segments.span_or_empty(place.segments));
    Some(canonical)
}
