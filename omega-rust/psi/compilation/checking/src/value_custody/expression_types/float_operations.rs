//! Builtin floating operations retain both operands' actual formats.
//!
//! A Boolean comparison result supplies no permission to convert a landed
//! operand. Shared result queries provide each operand's own type; the existing
//! declaration-selection owner distinguishes builtin meaning from an authored
//! heterogeneous operation. No expression is evaluated or a proof fact admitted.
//! Runtime scanning and specification formation call this same check before
//! accepting a computation or using its proposed proposition as evidence.
//! Canonical contracts: numeric_values.md (literal landing, mixed-integer
//! comparison limits, floating operations) and expressions.md (resolved identity).

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(super) fn validate_binary_float_formats(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    operands: &[Option<TypeReferenceHandle>; 2],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        return;
    }
    let [Some(left), Some(right)] = *operands else {
        return;
    };
    let (Some(left), Some(right)) = (
        program.primitive_type_reference(left),
        program.primitive_type_reference(right),
    ) else {
        return;
    };
    if !matches!(
        (left, right),
        (PrimitiveType::F32, PrimitiveType::F64) | (PrimitiveType::F64, PrimitiveType::F32)
    ) {
        return;
    }
    let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator) else {
        return;
    };
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program, owner, expression, spelling, operands,
    ) {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "builtin `{}` cannot combine `{}` and `{}`; a landed float retains its format, so changing format requires an explicit conversion",
        spelling.symbol(), left.name(), right.name(),
    )).with_source_span(program.expression_table.source_span(expression)));
}

/// Specification owners supply their exact declaration telescope, including
/// reserved results and domain/data subjects. Generic or unrecovered carriers
/// remain unresolved; neither a destination nor a descendant guesses a format.
pub(crate) fn validate_float_expression_formats(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    result_type: &impl Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    crate::expression_traversal::visit_pre_order(program, vec![expression], |handle, node| {
        if let typed_trees::expression::ExpressionNode::Binary(binary) = node {
            let operands = [binary.left, binary.right].map(result_type);
            validate_binary_float_formats(program, owner, handle, &operands, diagnostics);
        }
        false
    });
}
