//! Context selects data identity; spelling and an executable Boolean result do
//! not. Formation and crash-key production share this classification so a
//! logical child retains Prop meaning while an ordinary call's Bool argument,
//! member receiver or array index remains a value expression.

use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::type_identity::NormalizedTypeIdentity;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Complete data identity preserves qualifications and array extent. Only a
/// root loan observes its referent; a stored reference is not implicitly read.
pub fn contextual_identity_subject_type(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    root_borrow: bool,
) -> Option<NormalizedTypeIdentity> {
    if let Some(shell) = crate::reference_shell(program, reference) {
        if !root_borrow || shell.access == language_core::ReferenceAccess::WriteOnly {
            return None;
        }
        reference = shell.referee;
    }
    if program.primitive_type_reference(reference).is_some() {
        return None;
    }
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Unit => {}
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            program.closed_data_definition(reference)?;
        }
        _ => return None,
    }
    Some(program.normalized_type_identity(reference))
}

/// Logical composition supplies proposition context to its children. Ordinary
/// expression-call arguments are value terms; nominal proposition applications
/// have their own declared proof telescope, outside this expression grammar.
pub fn contextual_proposition_children(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> [Option<ExpressionHandle>; 2] {
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            [Some(unary.operand), None]
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                || (matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && [binary.left, binary.right].into_iter().all(|operand| {
                    super::mathematical_applications::operand_type_reference(program, operand)
                        .and_then(|reference| program.primitive_type_reference(reference))
                        == Some(PrimitiveType::Bool)
                })) =>
        {
            [Some(binary.left), Some(binary.right)]
        }
        _ => [None, None],
    }
}

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    owner: &impl std::fmt::Display,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            continue;
        }
        visited.push(expression);
        pending.extend(
            contextual_proposition_children(program, expression)
                .into_iter()
                .flatten(),
        );
        let Some(binary) = program.expression_table.binary_node(expression) else {
            continue;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) {
            continue;
        }
        let references = [binary.left, binary.right].map(|operand| {
            super::mathematical_applications::operand_type_reference(program, operand)
        });
        let [Some(left), Some(right)] = references else {
            continue;
        };
        let identities =
            [(binary.left, left), (binary.right, right)].map(|(operand, reference)| {
                contextual_identity_subject_type(
                    program,
                    reference,
                    matches!(
                        program.expression_table.expression(operand),
                        ExpressionNode::Name(_) | ExpressionNode::Borrow(_)
                    ),
                )
            });
        if identities.iter().any(Option::is_some)
            && (identities[0].is_none() || identities[0] != identities[1])
        {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "{owner} data identity requires the same complete operand type, found `{}` and `{}`",
                program.display_type_reference(left), program.display_type_reference(right),
            )).with_source_span(program.expression_table.source_span(expression)));
        }
    }
}
