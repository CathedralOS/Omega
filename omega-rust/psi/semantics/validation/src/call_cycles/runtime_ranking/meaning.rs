//! The early validator may use intrinsic arithmetic only when the existing
//! operator owner rules out authored and selected-trait interpretations.

use language_core::operator_spelling::OperatorSpelling;
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
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        // These have no authored OperatorSpelling. Their operands still
        // traverse the same inert-expression check.
        BinaryOperator::And
        | BinaryOperator::Or
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return true,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine,
        expression,
        spelling,
        &[
            operand_type(program, machine, binary.left),
            operand_type(program, machine, binary.right),
        ],
    )
}

fn operand_type(
    program: &TypedTrees,
    machine: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program
        .expression_table
        .expression(super::projection::unwrapped(program, expression))
    {
        ExpressionNode::Name(path) => program
            .machines()
            .iter()
            .find(|value| value.symbol == machine)
            .into_iter()
            .flat_map(|value| program.machine_states(value))
            .flat_map(|state| program.state_parameters(state))
            .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == path.symbol)
            .map(|parameter| parameter.type_reference),
        ExpressionNode::Member(member) => {
            let receiver = operand_type(program, machine, member.receiver)?;
            let receiver = crate::places::unwrapped_type_reference(program, receiver)?;
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(receiver)
            else {
                return None;
            };
            if !symbol.is_valid() {
                return None;
            }
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *symbol)?;
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
