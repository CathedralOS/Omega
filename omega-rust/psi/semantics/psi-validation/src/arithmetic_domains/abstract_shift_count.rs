//! Direct abstract-Prop shift-count formation.
//!
//! Executable machine/state expressions use the full arithmetic analyzer in
//! the parent module. Abstract signatures have no machine/state context, so
//! this pass deliberately recognizes only a direct fixed-integer shifted
//! operand and the existing direct interval vocabulary for its count.

use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::integer_policy::{IntegerPolicyPrimitive, ShiftCountLaw, integer_policy_bridge};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::PrimitiveType;

use super::total_specification::{
    AbstractSpecificationBindings, abstract_specification_interval,
    abstract_specification_place_type,
};
use super::{ValueEnv, integer_bit_width, primitive_name};

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
    env: &ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn walk(
        program: &TypedTrees,
        expression: ExpressionHandle,
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        env: &ValueEnv,
        diagnostics: &mut Vec<Diagnostic>,
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);

        let recurse = |child, diagnostics: &mut Vec<Diagnostic>, visited: &mut Vec<_>| {
            walk(program, child, owner, bindings, env, diagnostics, visited);
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => {
                for child in program.expression_table.expression_handles(*values) {
                    recurse(*child, diagnostics, visited);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                recurse(atomic.value, diagnostics, visited);
                recurse(atomic.result, diagnostics, visited);
            }
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
                let count = abstract_specification_interval(program, bindings, env, binary.right);
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
            ExpressionNode::Cast(cast) => recurse(cast.value, diagnostics, visited),
            ExpressionNode::Call(call) => {
                recurse(call.receiver, diagnostics, visited);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    recurse(*argument, diagnostics, visited);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                recurse(indexed.collection, diagnostics, visited);
                recurse(indexed.index, diagnostics, visited);
            }
            ExpressionNode::Member(member) => recurse(member.receiver, diagnostics, visited),
            ExpressionNode::Borrow(value) => recurse(value.target, diagnostics, visited),
            ExpressionNode::Unary(unary) => recurse(unary.operand, diagnostics, visited),
            ExpressionNode::Range(range) => {
                recurse(range.start, diagnostics, visited);
                recurse(range.end, diagnostics, visited);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    recurse(field.value, diagnostics, visited);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    walk(
        program,
        expression,
        owner,
        bindings,
        env,
        diagnostics,
        &mut Vec::new(),
    );
}
