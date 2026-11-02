//! Direct abstract-Prop shift-count formation.
//!
//! Executable machine/state expressions use the full arithmetic analyzer in
//! the parent module. Abstract signatures have no machine/state context, so
//! this pass deliberately recognizes only a direct fixed-integer shifted
//! operand and the existing direct interval vocabulary for its count.

use super::visit_binary_operators;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_policy::{IntegerPolicyPrimitive, ShiftCountLaw, integer_policy_bridge};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

use super::ValueEnvironment;
use super::total_specification::{
    AbstractSpecificationBindings, abstract_specification_builtin_arithmetic,
    abstract_specification_interval, direct_abstract_operand,
};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, primitive_name,
};

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    owner: &str,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    visit_binary_operators(
        program,
        expression,
        &[BinaryOperator::ShiftLeft, BinaryOperator::ShiftRight],
        |expression, binary| {
            if !abstract_specification_builtin_arithmetic(program, bindings, expression) {
                return;
            }
            let Some((primitive, domain)) = direct_abstract_operand(program, bindings, expression)
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
        },
    );
}
