//! Exit-local congruence for admitted proof integer embeddings.

use super::return_values::{exit_return_expression, is_result_reference};
use psi_checked_trees::{FlowExitFact, FlowStateFact};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::SignatureContractKind;

pub(super) fn proves_exit_equality(
    program: &TypedTrees,
    state_flow: &FlowStateFact,
    exit_flow: &FlowExitFact,
    fact: &psi_facts::Fact,
) -> bool {
    let psi_facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return false;
    };
    if exit_flow.machine_symbol != machine.symbol || exit_flow.state_symbol != state_flow.state_symbol
        || !program.machine_contracts(machine).iter().any(|contract| {
            contract.kind == SignatureContractKind::Ensures && program.proof_facts.span_or_empty(contract.facts).iter()
                .any(|fact| matches!(fact, ProofFact::Expression(source) if *source == expression))
        })
    { return false; }
    let return_value = exit_return_expression(program, exit_flow);
    let source = |argument: ExpressionHandle| {
        if is_result_reference(program, machine, argument) {
            return_value.is_valid().then_some(return_value)
        } else {
            Some(argument)
        }
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let Some((left_carrier, left)) =
        psi_validation::integer_embedding_argument(program, binary.left)
    else {
        return false;
    };
    let Some((right_carrier, right)) =
        psi_validation::integer_embedding_argument(program, binary.right)
    else {
        return false;
    };
    left_carrier == right_carrier
        && source(left)
            .zip(source(right))
            .is_some_and(|(left, right)| {
                psi_validation::integer_embedding_sources_equal(program, left, right)
            })
}
