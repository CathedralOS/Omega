//! Out-of-range comparison literals and mismatched operand widths.

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

/// Reject a COMPARISON (`==`/`!=`/`<`/`<=`/`>`/`>=`) or BITWISE (`&`/`|`/`^`)
/// operation between two integer places of DIFFERENT primitive types
/// (`self.i8 == self.i32`, `self.u32 | self.u8`): the backend performs it at the
/// NARROWER operand's width, silently truncating the wider one -- `i8(44) == i32(300)`
/// reads TRUE (`300 & 0xFF == 44`) and `u32(256) | u8(1)` reads `1` not `257` (both
/// confirmed native). Two integer operands must be the SAME type; convert one with an
/// `as` cast. Fires only when BOTH operands resolve to integer primitives
/// (`primitive_range` is `Some`) that differ. NOT arithmetic (`+ - * / %`, whose
/// mismatch is caught by the decision-17 overflow obligation) nor SHIFT (whose right
/// operand is a bit COUNT, not a width-matched value). A literal operand (no declared
/// place type -- handled by the out-of-range check), a float/bool/text operand, and
/// same-type operands are all skipped. Sibling of
/// `report_out_of_range_comparison_literal`.
pub(crate) fn report_mismatched_width_operands(
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
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
    ) {
        return false;
    }
    let operand_integer = |operand| {
        crate::value_custody::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .filter(|primitive| primitive_range(*primitive).is_some())
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
         operands have different integer types and the operation would silently truncate the wider \
         one to the narrower width; convert one with an `as` cast so both are the same type",
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
