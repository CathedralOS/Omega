//! Direct abstract-Prop shift-count formation.
//!
//! Executable machine/state expressions use the full arithmetic analyzer in
//! the parent module. Abstract signatures have no machine/state context, so
//! this pass deliberately recognizes only a direct fixed-integer shifted
//! operand and the existing direct interval vocabulary for its count.

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_policy::{IntegerPolicyPrimitive, ShiftCountLaw, integer_policy_bridge};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

use super::ValueEnvironment;
use super::total_specification::{
    AbstractSpecificationBindings, abstract_specification_interval,
    abstract_specification_place_type,
};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, primitive_name,
};

fn direct_shifted_operand(
    program: &TypedTrees,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => Some((
            program.primitive_type_reference(cast.target_type)?,
            cast.domain,
        )),
        ExpressionNode::Borrow(value) => direct_shifted_operand(program, bindings, value.target),
        _ => {
            let type_reference = abstract_specification_place_type(program, bindings, expression)?;
            Some((
                program.primitive_type_reference(type_reference)?,
                program.arithmetic_domain_for_type_reference(type_reference),
            ))
        }
    }
}

pub(super) fn validate(
    program: &TypedTrees,
    expression: ExpressionHandle,
    owner: &str,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn walk(
        program: &TypedTrees,
        expression: ExpressionHandle,
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        environment: &ValueEnvironment,
        diagnostics: &mut Vec<Diagnostic>,
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);

        let recurse = |child, diagnostics: &mut Vec<Diagnostic>, visited: &mut Vec<_>| {
            walk(
                program,
                child,
                owner,
                bindings,
                environment,
                diagnostics,
                visited,
            );
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                recurse(binary.left, diagnostics, visited);
                recurse(binary.right, diagnostics, visited);
                if !matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                ) {
                    return;
                }
                let Some((primitive, domain)) =
                    direct_shifted_operand(program, bindings, binary.left)
                else {
                    return;
                };
                let shift_primitive_kind = match binary.operator {
                    BinaryOperator::ShiftLeft => IntegerPolicyPrimitive::ShiftLeft,
                    BinaryOperator::ShiftRight => IntegerPolicyPrimitive::ShiftRight,
                    _ => unreachable!("non-shift operator returned above"),
                };
                if integer_policy_bridge(shift_primitive_kind, domain).shift_count_law
                    != ShiftCountLaw::MustBeWithinWidth
                {
                    return;
                }
                let Some(width) = integer_bit_width(primitive) else {
                    return;
                };
                let count =
                    abstract_specification_interval(program, bindings, environment, binary.right);
                let provably_in_range = count.is_some_and(|count| {
                    matches!(count.low(), Some(low) if low >= 0)
                        && matches!(count.high(), Some(high) if high < width)
                });
                if provably_in_range {
                    return;
                }
                let always_out = count.is_some_and(|count| {
                    matches!(count.low(), Some(low) if low >= width)
                        || matches!(count.high(), Some(high) if high < 0)
                });
                let verdict = if always_out {
                    "is provably out of range and can never execute"
                } else {
                    "is not provably below the operand width"
                };
                let saturating_hint = if domain == ArithmeticDomain::Saturating {
                    " (`Saturating` governs value overflow, not count validity -- its count obligation is Exact's)"
                } else {
                    ""
                };
                diagnostics.push(Diagnostic::error(format!(
                    "shift count in {owner} {verdict} for `{prim}`{saturating_hint}: exact shifts prove `count < {width}` before the term forms; discharge the count with an independently accepted prior fact, or use `{prim} in Wrapping` for a modulo-{width} count",
                    prim = primitive_name(primitive),
                )));
            }
            other => crate::expression_traversal::children(program, other, |child| {
                recurse(child, diagnostics, visited);
            }),
        }
    }

    walk(
        program,
        expression,
        owner,
        bindings,
        environment,
        diagnostics,
        &mut Vec::new(),
    );
}
