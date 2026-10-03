//! Out-of-range comparison literals and mismatched integer carriers.

use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_literal_value, primitive_range,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Reject a comparison (`==`/`!=`/`<`/`<=`/`>`/`>=`) between an integer-typed value
/// and an integer LITERAL outside that type's range: `self.b == 300` for a `u8` `b`
/// silently TRUNCATED the literal to the operand width (`300 & 0xFF == 44`) and
/// compared `b == 44` -- a confirmed miscompile (native took the `== 44` branch).
/// A literal compared against a value must be a representable value of that value's
/// type. Fires only when one operand resolves to an integer primitive and the other
/// is an integer literal outside its range; two-place, float/bool/text, and in-range
/// pairings are skipped. Sibling of the decision-17 narrowing obligation for stores.
pub(crate) fn report_out_of_range_comparison_literal(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        return false;
    }
    for (typed_operand, literal_operand) in [(left, right), (right, left)] {
        let Some(primitive) = crate::value_custody::places::declared_place_type(
            program,
            machine,
            state,
            typed_operand,
        )
        .and_then(|type_reference| program.primitive_type_reference(type_reference)) else {
            continue;
        };
        let Some(range) = primitive_range(primitive) else {
            continue;
        };
        let Some(literal) = integer_literal_value(program, literal_operand) else {
            continue;
        };
        let in_range = range.low().is_none_or(|low| literal >= low)
            && range.high().is_none_or(|high| literal <= high);
        if !in_range {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` compares a `{}` value against `{literal}`, which is out of \
                 range for `{}` -- the comparison would silently truncate the literal to the \
                 operand width; compare an in-range value or widen the value with an `as` cast",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
                primitive.name(),
                primitive.name(),
            )));
            return true;
        }
    }
    false
}

/// Builtin arithmetic and bitwise operands retain one common integer carrier.
/// Resolve each expression's type, not its inferred interval: calls, casts and
/// nested computations remain typed even without a useful range proof. The
/// caller establishes builtin meaning before applying this rule; declared
/// operators keep their own signatures. Comparisons allow mixed integer
/// carriers, shifts take an independent count, and anonymous values land at
/// their typed peer. None of these type checks supplies a range fact.
pub(crate) fn report_mismatched_integer_operands(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
    ) {
        return false;
    }
    let operand_integer = |operand| {
        if crate::value_custody::literals::has_anonymous_numeric_results(program, operand) {
            return None;
        }
        state
            .and_then(|state| {
                crate::value_custody::expression_types::expression_result_type_reference(
                    program, machine, state, operand,
                )
            })
            .or_else(|| {
                crate::value_custody::places::declared_place_type(program, machine, state, operand)
            })
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .filter(|primitive| super::integer_bit_width(*primitive).is_some())
    };
    let (Some(left_primitive), Some(right_primitive)) =
        (operand_integer(left), operand_integer(right))
    else {
        return false;
    };
    if left_primitive == right_primitive {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a `{}` value and a `{}` value -- the \
         operands have different integer types and the operation would silently truncate or \
         reinterpret a value; convert one with an exact `as` cast so both are the same type",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        left_primitive.name(),
        right_primitive.name(),
    )));
    true
}

/// The operators whose result is genuine integer arithmetic and can therefore
/// exceed the `{0, 1}` range even when the operands are bools (bool feeds in as
/// its 0/1 value). Excludes bitwise `& | ^` (which preserve `{0, 1}` for `{0, 1}`
/// operands) and comparison/logical ops (which yield a bool). Used both for the
/// overflow analysis here and, via `expression_types`, to classify an arithmetic
/// result as numeric for the cross-class store check.
/// The source spelling of an arithmetic operator, for diagnostics.
pub(crate) fn arithmetic_operator_spelling(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::ShiftLeft => "<<",
        BinaryOperator::ShiftRight => ">>",
        _ => "?",
    }
}

pub(crate) fn is_arithmetic(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
    )
}
