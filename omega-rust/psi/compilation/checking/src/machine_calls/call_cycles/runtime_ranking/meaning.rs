//! The early validator may use intrinsic arithmetic only when the existing
//! operator owner rules out authored and selected-trait interpretations.
//! Sealed IEEE comparisons are also inert, without becoming integer arithmetic.

use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableBinaryExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(super) fn binary_is_inert(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    binary: &TableBinaryExpression,
) -> bool {
    let spelling = match binary.operator {
        BinaryOperator::CaseMembership => {
            return crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                program, machine, Some(state), expression, binary,
            );
        }
        operator => {
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(operator) else {
                // These have no authored OperatorSpelling. Their operands
                // still traverse the same inert-expression check.
                return true;
            };
            spelling
        }
    };
    let operand_types = [binary.left, binary.right].map(|operand| {
        crate::proof_contracts::bound_expression_meaning::operand_type(
            program,
            machine,
            Some(state),
            super::projection::unwrapped(program, operand),
        )
    });
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &operand_types,
    ) || crate::proof_contracts::bound_expression_meaning::sealed_float_comparison(
        program,
        machine,
        Some(state),
        expression,
        binary,
    )
}
