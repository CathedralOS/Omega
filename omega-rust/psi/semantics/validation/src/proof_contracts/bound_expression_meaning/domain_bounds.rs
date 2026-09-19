//! Domain predicates bind their own reserved `self`, never a caller's receiver.
//! These readers supply selected meaning and carrier identity to relational
//! consumers; they do not establish membership. Arithmetic needs the domain's
//! overflow/interpretation evidence before it can become a mathematical bound.

use super::builtin_type_reference;
use super::definition_membership::{
    exact_domain_self_type, facts_contain_expression, has_exact_domain_definition,
};
use language_core::OperatorSpelling;
use typed_trees::TypedTrees;
use typed_trees::domain::DomainDefinition;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Rejoin a scalar membership subject by its retained binding, and compare
/// exact compiler builtin carriers. Display spelling cannot bind a value or
/// turn a foreign nominal type into an integer. Mutability and capture identity
/// are separate obligations of the bound normalizer.
pub fn has_exact_integer_domain_subject(
    program: &TypedTrees,
    domain: &DomainDefinition,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    if !has_exact_domain_definition(program, domain)
        || !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || program
            .expression_table
            .name_path_member_symbols(path.member_symbols)
            != [path.symbol]
    {
        return false;
    }
    let Some(subject_type) =
        crate::value_custody::places::bound_symbol_declared_type(program, path.symbol)
    else {
        return false;
    };
    matches!(integer_carrier(program, domain.target_type), Some(carrier)
        if integer_carrier(program, subject_type) == Some(carrier))
}

fn integer_carrier(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    // Do not unwrap references or atomic shells: those do not name an owned
    // captured integer. Bound malformed constraint cycles without recursion.
    for _ in 0..128 {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => {
                let primitive =
                    crate::value_custody::recasts::exact_primitive_type(program, reference)?;
                return (primitive.accepts_integer_literal() && primitive != PrimitiveType::Addr)
                    .then_some(primitive);
            }
        }
    }
    None
}

/// Check one definition-owned Boolean node before a consumer decomposes it.
/// Each extracted child must be checked separately, starting at the declared
/// predicate root. Subtree ownership alone does not imply the subtree is true.
pub fn has_builtin_domain_decomposed_guard_meaning(
    program: &TypedTrees,
    domain: &DomainDefinition,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression)
        || !has_exact_domain_definition(program, domain)
        || !facts_contain_expression(program, domain.facts, expression)
    {
        return false;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return matches!(program.expression_table.expression(expression),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot);
    };
    let spelling = match binary.operator {
        BinaryOperator::And | BinaryOperator::Or => return true,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        _ => return false,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        domain.symbol,
        expression,
        spelling,
        &[
            operand_type(program, domain, binary.left),
            operand_type(program, domain, binary.right),
        ],
    )
}

fn operand_type(
    program: &TypedTrees,
    domain: &DomainDefinition,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => exact_domain_self_type(program, domain, expression),
        ExpressionNode::Integer(_) => {
            crate::landed_integer_literal_type_reference(program, expression)
        }
        ExpressionNode::Boolean(_) => {
            builtin_type_reference(program, symbols::BuiltinTypeAtom::Bool)
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            builtin_type_reference(program, symbols::BuiltinTypeAtom::Bool)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or
            ) =>
        {
            builtin_type_reference(program, symbols::BuiltinTypeAtom::Bool)
        }
        // Unknown is a wildcard for overload lookup, never an assumed copy
        // of the other operand's type. Consumers separately admit bound leaves.
        _ => None,
    }
}
