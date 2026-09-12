//! Declared range bounds share exact anonymous arithmetic with static inference.
//!
//! Keep the authored expression and its operator selections intact. Range
//! validation, proof construction and representation readers must agree on the
//! completed integer value; the syntax-only i64 folder truncates fractional
//! intermediates and cannot supply that semantic fact. Consumers with bounded
//! storage convert only the final value, without changing its denotation.

use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// The exact integer carrier and single authored range, without intersecting
/// multiple declarations or converting symbolic endpoints to guessed values.
pub fn declared_integer_range(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<(symbols::BuiltinTypeAtom, [ExpressionHandle; 2])> {
    if program.arithmetic_domain_for_type_reference(type_reference)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    let mut endpoints = None;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            if let TypeConstraintNode::Range { minimum, maximum } = constraint
                && endpoints.replace([*minimum, *maximum]).is_some()
            {
                return None;
            }
        }
        type_reference = *base_type;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let carrier = program.symbols.builtin_type_atom(*symbol)?;
    use symbols::BuiltinTypeAtom;
    matches!(
        carrier,
        BuiltinTypeAtom::U8
            | BuiltinTypeAtom::U16
            | BuiltinTypeAtom::U32
            | BuiltinTypeAtom::U64
            | BuiltinTypeAtom::I8
            | BuiltinTypeAtom::I16
            | BuiltinTypeAtom::I32
            | BuiltinTypeAtom::I64
    )
    .then_some((carrier, endpoints?))
}

/// Read a closed integer range endpoint without interpreting typed
/// computations as anonymous arithmetic. Unknown, fractional and invalid
/// expressions supply no bound, including when an authored operator is selected.
pub fn closed_integer_range_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    if let ExpressionNode::Integer(value) = program.expression_table.expression(expression) {
        return value.value_bignum();
    }
    if let Some(value) = crate::evaluate_anonymous_numeric_expression_with_selected_match_arms(
        program,
        expression,
        &[],
        |expression| crate::has_anonymous_operator_meaning(program, expression),
    ) {
        return value.to_integer_exact();
    }
    crate::arithmetic_domains::closed_integer_expression_value(program, expression)
}
