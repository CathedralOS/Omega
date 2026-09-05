//! Exit-local congruence for admitted proof integer embeddings.

use psi_checked_trees::{FlowExitFact, FlowStateFact};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};

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
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == exit_flow.state_symbol)
    else {
        return false;
    };
    let return_value = program
        .statement_table
        .statements(state.statement_nodes)
        .get(exit_flow.statement_index)
        .and_then(|statement| {
            if let StatementNode::Expression(value) = statement {
                return (!exit_flow.transition_target.is_valid()
                    && exit_flow.statement_index.checked_add(1)
                        == Some(state.statement_nodes.len()))
                .then_some(*value);
            }
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            if !exit_flow.transition_target.is_valid()
                || ![transition.target, transition.continuation]
                    .contains(&exit_flow.transition_target)
                || transition.exit != psi_typed_trees::statement::TransitionExit::Ordinary
            {
                return None;
            }
            match program
                .statement_table
                .transition_target(exit_flow.transition_target)
            {
                TransitionTargetNode::Value(value) => Some(*value),
                _ => None,
            }
        });
    let result_shadowed = program.machine_states(machine).first().is_none_or(|entry| {
        program
            .state_parameters(entry)
            .iter()
            .any(|parameter| parameter.name.as_str() == "result")
    });
    let source = |argument: ExpressionHandle| {
        if !result_shadowed
            && matches!(program.expression_table.expression(argument),
            ExpressionNode::Name(path) if matches!(program.expression_table.name_path_members(path.members), [name] if name.as_str() == "result"))
        {
            return_value
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
