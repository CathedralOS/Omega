//! Declaration-owned collection measurements of a completed call result.
//! The selected contract supplies the relation; the callee body and parameter
//! spelling supply no evidence. Call admission and exit checking remain separate.
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// The shared collection actual measured by this normal result, if the public
/// contract unconditionally equates its reserved result with that formal's
/// compiler-owned length. Consumers capture the descriptor at evaluation and
/// retire the relation after descriptor or selector replacement.
pub fn normal_return_length_collection(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<ExpressionHandle> {
    if !call.selects_only_nominal_route() || call.static_machine_parameter.is_valid() {
        return None;
    }
    let callee = crate::machine_calls::calls::callee_entry_contract(program, call.target_symbol)?;
    if !program
        .primitive_type_reference(callee.return_type)
        .is_some_and(PrimitiveType::is_fixed_integer)
    {
        return None;
    }
    let arguments: Vec<_> = call
        .receiver
        .is_valid()
        .then_some(call.receiver)
        .into_iter()
        .chain(
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        )
        .skip(callee.leading_operands)
        .collect();
    if arguments.len() != callee.parameters.len() {
        return None;
    }
    let mut pending: Vec<_> = program
        .contract_expressions(callee.contracts.iter(), SignatureContractKind::Ensures)
        .collect();
    let mut budget = 128;
    while let Some(expression) = pending.pop() {
        if budget == 0 {
            return None;
        }
        budget -= 1;
        let Some(binary) = program.expression_table.binary_node(expression) else {
            continue;
        };
        if binary.operator == BinaryOperator::And {
            pending.extend([binary.left, binary.right]);
            continue;
        }
        if binary.operator != BinaryOperator::Equal {
            continue;
        }
        let is_result = |operand| {
            crate::reserved_result_owner(program, operand).is_some_and(|(owner, reference)| {
                owner == callee.callable && reference == callee.return_type
            })
        };
        let measure = if is_result(binary.left) {
            binary.right
        } else if is_result(binary.right) {
            binary.left
        } else {
            continue;
        };
        let Some(member) = crate::length_member(program, measure) else {
            continue;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
        else {
            continue;
        };
        let Some(position) = callee.parameters.iter().position(|formal| {
            formal.symbol.is_valid()
                && path.symbol == formal.symbol
                && path.head_symbol == formal.symbol
        }) else {
            continue;
        };
        let formal = &callee.parameters[position];
        if formal.is_mutable || formal.is_const {
            continue;
        }
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = program
            .type_reference_table
            .type_reference(formal.type_reference)
        else {
            continue;
        };
        if *access != language_semantics::ReferenceAccess::Shared
            || !matches!(
                program.type_reference_table.type_reference(*referee),
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
            )
        {
            continue;
        }
        let Some(measure_type) = crate::parameter_expression_result_type_reference(
            program,
            callee.callable,
            callee.parameters,
            measure,
        ) else {
            continue;
        };
        let operand_types = if is_result(binary.left) {
            [Some(callee.return_type), Some(measure_type)]
        } else {
            [Some(measure_type), Some(callee.return_type)]
        };
        if !super::call_result_bounds::builtin(
            program,
            callee.callable,
            expression,
            binary.operator,
            &operand_types,
        ) {
            continue;
        }
        let actual = arguments[position];
        return Some(match program.expression_table.expression(actual) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => actual,
        });
    }
    None
}
