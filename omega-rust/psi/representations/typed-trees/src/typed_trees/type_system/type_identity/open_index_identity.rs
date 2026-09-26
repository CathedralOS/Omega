//! Open index operation selection and algebra identities.

use crate::TypedTrees;
use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::typed_trees::type_system::type_identity::constraint_identity::{atom, compound};
use crate::typed_trees::type_system::type_identity::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_index_expression,
};

pub(crate) fn open_index_operation_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<&crate::typed_trees::OpenIndexOperationSelection> {
    program
        .open_index_normalizations
        .iter()
        .flat_map(|normalization| &normalization.operations)
        .find(|selection| selection.expression == expression)
}

pub(crate) fn open_index_algebra_identity(
    program: &TypedTrees,
    selection: &crate::typed_trees::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> String {
    if context.qualification == TypeIdentityQualification::Ordinary {
        return format!(
            "{}::{} as {}",
            program.symbols.display_path(selection.algebra_trait, "::"),
            selection.algebra_requirement,
            selection.algebra_alias.as_deref().unwrap_or("<default>")
        );
    }
    compound(
        "open-index-algebra",
        [
            atom(
                "provider",
                &context.name(program, selection.provider, "<unresolved-provider>"),
            ),
            atom(
                "trait",
                &context.name(
                    program,
                    selection.algebra_trait,
                    "<unresolved-algebra-trait>",
                ),
            ),
            atom("requirement", &selection.algebra_requirement),
            atom(
                "alias",
                selection.algebra_alias.as_deref().unwrap_or("<default>"),
            ),
        ],
    )
}

pub(crate) fn open_index_operation_identity(
    program: &TypedTrees,
    selection: &crate::typed_trees::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> String {
    if context.qualification == TypeIdentityQualification::Ordinary {
        return selection.operation_contract_identity.clone();
    }
    compound(
        "open-index-operation",
        [
            atom(
                "symbol",
                &context.name(
                    program,
                    selection.operator,
                    selection.operation_contract_identity.as_str(),
                ),
            ),
            atom("contract", &selection.operation_contract_identity),
        ],
    )
}

fn same_open_index_ac_authority(
    left: &crate::typed_trees::OpenIndexOperationSelection,
    right: &crate::typed_trees::OpenIndexOperationSelection,
) -> bool {
    left.operator == right.operator
        && left.operation_contract_identity == right.operation_contract_identity
        && left.algebra_trait == right.algebra_trait
        && left.algebra_requirement == right.algebra_requirement
        && left.algebra_alias == right.algebra_alias
        && left.commutativity_licensed == right.commutativity_licensed
        && left.associativity_licensed == right.associativity_licensed
}

/// Produces the operand list the canonical form is built over, consuming only
/// the laws the selected algebra declares.
///
/// Associativity licenses flattening: a nested chain of the same operation
/// under the same authority collapses into one operand list, so `(a + b) + c`
/// and `a + (b + c)` reach the same form. Without it the two immediate
/// operands stand as written and each nested operation keeps its own
/// structure.
///
/// Commutativity licenses ordering: the operands sort, so `a + b` and `b + a`
/// reach the same form. Without it the authored order is the canonical order.
///
/// A noncommutative, nonassociative operation is still usable — it simply
/// keeps its structural identity, which is what the unlicensed path already
/// produced before any algebra was selected.
pub(crate) fn collect_open_index_operands(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    selection: &crate::typed_trees::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> Vec<String> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return vec![normalize_index_expression(program, expression, context)];
    };
    let mut operands = Vec::new();
    if selection.associativity_licensed {
        collect_licensed_ac_operands(
            program,
            binary.left,
            operator,
            selection,
            context,
            &mut operands,
        );
        collect_licensed_ac_operands(
            program,
            binary.right,
            operator,
            selection,
            context,
            &mut operands,
        );
    } else {
        operands.push(normalize_index_expression(program, binary.left, context));
        operands.push(normalize_index_expression(program, binary.right, context));
    }
    if selection.commutativity_licensed {
        operands.sort();
    }
    operands
}

fn collect_licensed_ac_operands(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    authority: &crate::typed_trees::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
    operands: &mut Vec<String>,
) {
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == operator
        && open_index_operation_selection(program, expression)
            .is_some_and(|selection| same_open_index_ac_authority(authority, selection))
    {
        collect_licensed_ac_operands(program, binary.left, operator, authority, context, operands);
        collect_licensed_ac_operands(
            program,
            binary.right,
            operator,
            authority,
            context,
            operands,
        );
    } else {
        operands.push(normalize_index_expression(program, expression, context));
    }
}

pub(crate) fn index_binary_operator_name(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "add",
        BinaryOperator::And => "and",
        BinaryOperator::BitwiseAnd => "bitwise-and",
        BinaryOperator::BitwiseOr => "bitwise-or",
        BinaryOperator::BitwiseXor => "bitwise-xor",
        BinaryOperator::Divide => "divide",
        BinaryOperator::Equal => "equal",
        BinaryOperator::Greater => "greater",
        BinaryOperator::GreaterOrEqual => "greater-or-equal",
        BinaryOperator::Less => "less",
        BinaryOperator::LessOrEqual => "less-or-equal",
        BinaryOperator::Modulo => "modulo",
        BinaryOperator::Multiply => "multiply",
        BinaryOperator::NotEqual => "not-equal",
        BinaryOperator::Or => "or",
        BinaryOperator::ShiftLeft => "shift-left",
        BinaryOperator::ShiftRight => "shift-right",
        BinaryOperator::Subtract => "subtract",
        BinaryOperator::CaseMembership => "case_membership",
    }
}
