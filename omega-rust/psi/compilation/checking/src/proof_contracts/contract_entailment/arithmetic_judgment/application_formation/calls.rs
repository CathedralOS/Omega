//! Form ordinary integer calls before consuming their public result facts.
//! The outer declaration application owns the transaction and exact inputs;
//! operational purity and totality are independent specification-call debts.

use super::{ApplicationDecisions, Engine, Polynomial, formed_type, formed_value};
use crate::frozen_program::expression_table;
use crate::proof_contracts::contract_entailment::arithmetic_judgment::Judgment;
use crate::proof_contracts::contract_entailment::specification_calls::{
    SelectedCallContract, owner_contracts, owner_parameters, owner_return_type, owner_symbol,
    requirements_are_acyclic, selected_call_contract,
};
use numerics::bignum::BigInt;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;

pub(super) fn form(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
    decisions: &mut ApplicationDecisions,
) -> Option<Polynomial> {
    let call = expression_table(program).call_node(expression)?;
    let owner = selected_call_contract(program, engine.source_owner, call)?;
    let parameters = owner_parameters(program, &owner);
    let arguments = expression_table(program).expression_handles(call.arguments);
    if parameters.len() != arguments.len()
        || !requirements_are_acyclic(program, &owner, &mut Vec::new(), &mut Vec::new())
    {
        return None;
    }
    let actuals = arguments
        .iter()
        .map(|argument| {
            Some((
                *argument,
                formed_value(engine, program, *argument, depth + 1, decisions)?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let formals = parameters
        .iter()
        .zip(&actuals)
        .map(|(formal, (_, value))| (formal.symbol, value.clone()))
        .collect::<Vec<_>>();
    engine.with_formal_arguments(owner_symbol(&owner), &formals, |engine| {
        for (parameter, (_, value)) in parameters.iter().zip(&actuals) {
            if !formed_type(engine, program, parameter.type_reference, value) {
                return None;
            }
        }
        for contract in owner_contracts(program, &owner)
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(requirement) = fact else {
                    return None;
                };
                if contract.binding.is_some()
                    || !condition_forms(
                        engine,
                        program,
                        *requirement,
                        depth + 1,
                        &mut ApplicationDecisions::default(),
                    )
                    || !matches!(engine.judge(*requirement), Judgment::Proven)
                {
                    return None;
                }
            }
        }
        Some(())
    })?;
    let value = engine.selected_call_value(
        engine.source_owner,
        expression,
        owner_symbol(&owner),
        actuals.iter().map(|(_, value)| value.clone()).collect(),
    )?;
    // The result's inherent carrier follows from the checked callee only
    // after its invocation forms. It cannot help prove that invocation.
    let result_type = owner_return_type(&owner);
    let primitive = program.primitive_type_reference(result_type)?;
    let (minimum, maximum) = primitive.integer_bounds()?;
    engine
        .bounds
        .push((value.clone(), BigInt::from_i128(minimum)));
    engine
        .bounds
        .push((value.neg(), BigInt::from_i128(maximum).negate()));
    engine.refresh_difference_bounds();
    if !engine.complete_formed_call(expression, &actuals, &value) {
        return None;
    }
    engine.with_formal_arguments(owner_symbol(&owner), &formals, |engine| {
        formed_type(engine, program, result_type, &value).then_some(())
    })?;
    let SelectedCallContract::Concrete(machine, state) = owner else {
        return None;
    };
    let decision = checked_trees::CheckedDeclarationCall {
        expression,
        authored_target: call.target_symbol,
        arguments: call.arguments,
        target_machine: machine.symbol,
        target_state: state.symbol,
        target_contract: None,
    };
    if !decisions.calls.contains(&decision) {
        decisions.calls.push(decision);
    }
    Some(value)
}

fn condition_forms(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
    decisions: &mut ApplicationDecisions,
) -> bool {
    if depth >= 128 {
        return false;
    }
    match expression_table(program).expression(expression) {
        ExpressionNode::Boolean(_) => true,
        ExpressionNode::Atomic(atomic) => {
            condition_forms(engine, program, atomic.value, depth + 1, decisions)
        }
        ExpressionNode::Binary(binary) if engine.selected_binary_meaning(expression, binary) => {
            if binary.operator == BinaryOperator::And {
                condition_forms(engine, program, binary.left, depth + 1, decisions)
                    && matches!(engine.judge(binary.left), Judgment::Proven)
                    && condition_forms(engine, program, binary.right, depth + 1, decisions)
            } else if binary.operator.complement().is_some() {
                formed_value(engine, program, binary.left, depth + 1, decisions).is_some()
                    && formed_value(engine, program, binary.right, depth + 1, decisions).is_some()
            } else {
                false
            }
        }
        _ => false,
    }
}
