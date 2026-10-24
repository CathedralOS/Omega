//! Anonymous numeric formation and typed integer-to-float boundaries.
//!
//! Three rules decide whether an anonymous builtin operation has a carrier at
//! all: an integer `%` over wholly anonymous operands, an anonymous tree with
//! no exact value (a division by zero), and a bitwise or shift operation with
//! no typed operand. Each rule was its own expression walk in its own module,
//! and two of them carried their own copy of the anonymity question.
//! `numeric_literal_tree` answers that question once,
//! `anonymous_formation_message` owns the spelling-keyed rule and message, and
//! `report_anonymous_formation` owns the walk. The two entries stay separate
//! because they run at different points of validation.
//!
//! (wiki/spec/language/numeric_values.md): bitwise operations preserve their
//! signed/unsigned carrier and are total at its width, and a shift count is
//! proven against that carrier's `value_width`. An operand with no carrier
//! therefore has no defined result, exactly as an anonymous `%` has none, and
//! a destination annotation does not supply one. Without the bitwise rule such
//! an expression passes checking and then silently omits the machine's unit
//! plan at `structural field store: pure source: scalar expression row` — an
//! author writing `self.mode = 4 | 2;` is told only that the program entry
//! "rejoins 0 Terminal attachment identities". Remainder has no anonymous
//! numeric meaning either, so formation is checked independently of value
//! evaluation, including operands whose anonymous division is fractional.

use diagnostics::Diagnostic;
use language_core::OperatorSpelling;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

pub(in crate::value_custody::literals) fn validate_numeric_tree_destination(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(destination, PrimitiveType::F32 | PrimitiveType::F64) {
        return;
    }
    let Some(has_typed_operand) = numeric_literal_tree(program, expression) else {
        // Calls, places, casts and authored operators retain their ordinary
        // type/selection checks. This check evaluates no user-defined meaning.
        return;
    };
    let message = if has_typed_operand {
        format!(
            "typed integer arithmetic cannot implicitly land in `{}`; use an explicit numeric conversion",
            destination.name()
        )
    } else {
        if super::anonymous_exact_float_tree(program, expression).is_some() {
            return;
        }
        format!(
            "anonymous numeric arithmetic has no exact value for `{}` landing (division by zero is invalid)",
            destination.name()
        )
    };
    diagnostics.push(
        Diagnostic::error(message)
            .with_source_span(program.expression_table.source_span(expression)),
    );
}

/// Classify a closed builtin numeric tree. `Some(true)` contains an already
/// landed integer; typed floats and nonliteral operands decline this check, and
/// every leaf of a `Some(false)` tree is an unlanded numeric spelling joined by
/// operations with builtin meaning, so that tree takes no carrier from its own
/// contents.
pub(super) fn numeric_literal_tree(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<bool> {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    let mut has_typed_operand = false;
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) => has_typed_operand |= literal.landing().is_some(),
            ExpressionNode::Float(literal) if literal.landing().is_none() => {}
            ExpressionNode::Binary(binary)
                if crate::value_custody::literals::has_anonymous_operator_meaning(
                    program, expression,
                ) =>
            {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            _ => return None,
        }
    }
    Some(has_typed_operand)
}

/// The spellings each formation entry owns, in that entry's report order. A
/// bitwise spelling is not overloadable, so a binary node carrying one is the
/// builtin operation; `%` is, so an anonymous `%` needs builtin-operand
/// evidence of its own before its operands are read as untyped spellings.
const REMAINDER_FORMATION_SPELLINGS: &[BinaryOperator] = &[BinaryOperator::Modulo];
const DIVISION_FORMATION_SPELLINGS: &[BinaryOperator] = &[BinaryOperator::Divide];
const BITWISE_FORMATION_SPELLINGS: &[BinaryOperator] = &[
    BinaryOperator::BitwiseAnd,
    BinaryOperator::BitwiseOr,
    BinaryOperator::BitwiseXor,
    BinaryOperator::ShiftLeft,
    BinaryOperator::ShiftRight,
];

/// The symbol and whether it shifts, for a spelling that carries no selection
/// evidence of its own.
const fn bitwise(operator: BinaryOperator) -> Option<(&'static str, bool)> {
    match operator {
        BinaryOperator::BitwiseAnd => Some(("&", false)),
        BinaryOperator::BitwiseOr => Some(("|", false)),
        BinaryOperator::BitwiseXor => Some(("^", false)),
        BinaryOperator::ShiftLeft => Some(("<<", true)),
        BinaryOperator::ShiftRight => Some((">>", true)),
        _ => None,
    }
}

/// The formation message for one anonymous builtin operation, or `None` when
/// the operation has a carrier. A shift takes its width from the VALUE it
/// shifts, so typing only the count leaves it undefined; the symmetric bit
/// operations need either operand typed and land the other.
fn anonymous_formation_message(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<String> {
    let anonymous = |expression: ExpressionHandle| -> bool {
        numeric_literal_tree(program, expression) == Some(false)
    };
    match operator {
        BinaryOperator::Modulo => {
            let builtin =
                crate::value_custody::literals::integer_landing::has_builtin_anonymous_operands(
                    program,
                    expression,
                    OperatorSpelling::Modulo,
                );
            if !builtin || !anonymous(left) || !anonymous(right) {
                return None;
            }
            Some(
                "builtin `%` requires an integer-typed operand; type an operand (for example, \
                 `7i32 % 2`), not just the destination"
                    .to_string(),
            )
        }
        BinaryOperator::Divide => {
            if !anonymous(expression) {
                return None;
            }
            matches!(
                super::anonymous_exact_float_tree(program, right),
                Some(numerics::bignum::ExactFloat::Finite(value)) if value.is_zero()
            )
            .then(|| "anonymous division by zero has no exact numeric value".to_string())
        }
        operator => {
            let (spelling, shifts) = bitwise(operator)?;
            if !anonymous(left) || !(shifts || anonymous(right)) {
                return None;
            }
            Some(if shifts {
                format!(
                    "builtin `{spelling}` takes its width from the value it shifts, so that value \
                     needs an integer type; type it (for example, `4i32 {spelling} 1`), not just \
                     the destination"
                )
            } else {
                format!(
                    "builtin `{spelling}` requires an integer-typed operand; type an operand (for \
                     example, `6i32 {spelling} 4`), not just the destination"
                )
            })
        }
    }
}

fn report_anonymous_formation(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
    spellings: &[BinaryOperator],
) {
    // Formation applies to retained source expressions, even when their value
    // is used only in a comparison or a proof and has no numeric destination.
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Binary(binary) = node else {
            continue;
        };
        if !spellings.contains(&binary.operator) {
            continue;
        }
        let Some(message) = anonymous_formation_message(
            program,
            expression,
            binary.operator,
            binary.left,
            binary.right,
        ) else {
            continue;
        };
        diagnostics.push(
            Diagnostic::error(message)
                .with_source_span(program.expression_table.source_span(expression)),
        );
    }
}

/// The formation checks the pre-machine validation phase owns: an anonymous
/// integer `%` and an anonymous division by zero. Both report before any other
/// validator, and validation stops if either reports.
pub(crate) fn validate_anonymous_numeric_formation(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    report_anonymous_formation(program, diagnostics, REMAINDER_FORMATION_SPELLINGS);
    report_anonymous_formation(program, diagnostics, DIVISION_FORMATION_SPELLINGS);
}

/// The anonymous bitwise and shift formation check. It runs after every other
/// formation and flow check, so a missing anonymous bitwise carrier is
/// reported only when no source error already explains why the program cannot
/// form.
pub(crate) fn validate_anonymous_bitwise(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    report_anonymous_formation(program, diagnostics, BITWISE_FORMATION_SPELLINGS);
}
