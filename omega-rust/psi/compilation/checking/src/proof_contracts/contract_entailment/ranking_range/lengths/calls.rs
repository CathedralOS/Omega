//! A selected call's published normal-return length equation.
//! Argument values come from the caller's captured arithmetic roster. The
//! selected declaration supplies formal identity and its independent ensures;
//! neither a callee name nor a range-shaped argument supplies result geometry.

use super::{BigInt, Engine, ExpressionNode};
use crate::proof_contracts::contract_entailment::{
    StrictArithmeticBindingValue, StrictArithmeticSymbolBinding,
};
use typed_trees::expression::{BinaryOperator, TableCallExpression};
use typed_trees::signature::SignatureContractKind;

pub(super) fn normal_return_length(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
    caller: &mut Engine<'_>,
) -> Option<u64> {
    if !call.selects_only_nominal_route() || call.receiver.is_valid() {
        return None;
    }
    let contract = crate::machine_calls::calls::callee_entry_contract(program, call.target_symbol)?;
    if !crate::value_custody::type_shapes::has_slice_base(program, contract.return_type) {
        return None;
    }
    let (machine, state) =
        crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)?;
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .get(contract.leading_operands..)?;
    if arguments.len() != contract.parameters.len() {
        return None;
    }
    let bindings = contract
        .parameters
        .iter()
        .zip(arguments)
        .filter_map(|(parameter, argument)| {
            if parameter.is_mutable
                || parameter.is_const
                || !program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
            {
                return None;
            }
            caller
                .normalize(*argument)
                .and_then(|value| value.constant_value())
                .map(|value| StrictArithmeticSymbolBinding {
                    symbol: parameter.symbol,
                    value: StrictArithmeticBindingValue::Integer(value),
                })
        })
        .collect::<Vec<_>>();
    let mut callee = Engine::strict(program, machine, &bindings);
    let result_length = |expression| {
        let Some(receiver) =
            crate::collection_length_receiver(program, machine, Some(state), expression)
        else {
            return false;
        };
        crate::reserved_result_owner(program, receiver)
            == Some((contract.callable, contract.return_type))
    };
    for expression in program.contract_expressions(
        contract
            .contracts
            .iter()
            .filter(|contract| contract.binding.is_none()),
        SignatureContractKind::Ensures,
    ) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            continue;
        };
        if binary.operator != BinaryOperator::Equal
            || !crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            continue;
        }
        let value = if result_length(binary.left) {
            binary.right
        } else if result_length(binary.right) {
            binary.left
        } else {
            continue;
        };
        if let Some(length) = callee
            .normalize(value)
            .and_then(|value| value.constant_value())
            && length >= BigInt::zero()
        {
            return length.to_u64();
        }
    }
    None
}
