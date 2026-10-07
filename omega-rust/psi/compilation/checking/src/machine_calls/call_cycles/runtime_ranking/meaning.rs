//! The early validator may use intrinsic arithmetic only when the existing
//! operator owner rules out authored and selected-trait interpretations.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn binary_is_builtin(
    program: &TypedTrees,
    machine: SymbolHandle,
    expression: ExpressionHandle,
    binary: &TableBinaryExpression,
) -> bool {
    let spelling = match binary.operator {
        BinaryOperator::CaseMembership => {
            return program
                .machines()
                .iter()
                .find(|value| value.symbol == machine)
                .is_some_and(|machine| {
                    crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                        program, machine, None, expression, binary,
                    )
                });
        }
        operator => {
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(operator)
            else {
                // These have no authored OperatorSpelling. Their operands
                // still traverse the same inert-expression check.
                return true;
            };
            spelling
        }
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine,
        expression,
        spelling,
        &[
            operand_type(program, binary.left),
            operand_type(program, binary.right),
        ],
    )
}

fn operand_type(program: &TypedTrees, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
    match program
        .expression_table
        .expression(super::projection::unwrapped(program, expression))
    {
        ExpressionNode::Name(path) => {
            // Locals and parameters carry their declared type through the same
            // binder identity. Omitting locals turns known integer operands
            // into wildcards and lets unrelated nominal operators hide their
            // builtin meaning. This supplies a carrier, never a saved value:
            // prefix preservation and rank comparison still prove that value.
            crate::value_custody::expression_types::named_value_type_reference(program, path)
        }
        ExpressionNode::Member(member) => {
            let receiver = operand_type(program, member.receiver)?;
            let receiver =
                crate::value_custody::places::unwrapped_type_reference(program, receiver)?;
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(receiver)
            else {
                return None;
            };
            if !symbol.is_valid() {
                return None;
            }
            let data = crate::program_scope::data_definitions::data_definition_by_symbol(
                program, *symbol,
            )?;
            super::super::exact_data_member_field(
                program,
                data,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|variant| variant.as_str()),
            )
            .map(|field| field.type_reference)
        }
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        // Unknown literals/results retain wildcard candidate matching; they
        // cannot erase a possible authored interpretation at this stage.
        _ => None,
    }
}
