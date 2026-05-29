use super::place_types::expression_type_reference_in_state;
use super::*;

pub(in crate::flow::ownership) fn expression_requires_ownership(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    if !expression_is_place_like(program, expression) {
        return false;
    }

    expression_type_reference_in_state(program, state_symbol, statement_index, expression)
        .map(|type_reference| type_requires_ownership(program, type_reference))
        .unwrap_or(true)
}

pub(in crate::flow::ownership) fn type_requires_ownership(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }

    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => false,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_requires_ownership(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            type_requires_ownership(program, *element_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Named { name, .. } => !matches!(
            omega_typed_trees::types::PrimitiveType::from_name(name.as_str()),
            Some(
                omega_typed_trees::types::PrimitiveType::Bool
                    | omega_typed_trees::types::PrimitiveType::F32
                    | omega_typed_trees::types::PrimitiveType::F64
                    | omega_typed_trees::types::PrimitiveType::I32
                    | omega_typed_trees::types::PrimitiveType::U32
                    | omega_typed_trees::types::PrimitiveType::U64
                    | omega_typed_trees::types::PrimitiveType::Usize
            )
        ),
        omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. } => true,
        omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

fn expression_is_place_like(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_is_place_like(program, *inner),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => true,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => false,
    }
}
