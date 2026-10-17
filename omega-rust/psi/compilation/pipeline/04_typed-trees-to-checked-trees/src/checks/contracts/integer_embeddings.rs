//! Exit-local congruence for admitted proof integer embeddings.

use super::return_values::{exit_return_expression, is_result_reference};
use checked_trees::{FlowExitFact, FlowStateFact};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::SignatureContractKind;

pub(super) fn proves_exit_equality(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    exit_flow: &FlowExitFact,
    fact: &facts::Fact,
) -> bool {
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
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
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let Some((left_carrier, left)) = validation::integer_embedding_argument(program, binary.left)
    else {
        return false;
    };
    let Some((right_carrier, right)) =
        validation::integer_embedding_argument(program, binary.right)
    else {
        return false;
    };
    left_carrier == right_carrier
        && source(left)
            .zip(source(right))
            .is_some_and(|(left, right)| {
                validation::integer_embedding_sources_equal(program, left, right)
            })
}

/// `embed(result) == 3` and `embed(result.field) == 3` when the returned
/// integer is a closed length or literal. Plain equality already knows that
/// value; embedding is the same mathematical integer, not a second unknown.
pub(super) fn proves_embedded_literal_exit(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    exit_flow: &FlowExitFact,
    fact: &facts::Fact,
) -> bool {
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == exit_flow.state_symbol)
    else {
        return false;
    };
    if exit_flow.machine_symbol != machine.symbol || exit_flow.state_symbol != state.symbol {
        return false;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let Some((embedded, literal)) = embedded_literal(program, binary.left, binary.right)
        .or_else(|| embedded_literal(program, binary.right, binary.left))
    else {
        return false;
    };
    let returned = exit_return_expression(program, exit_flow);
    let subject = returned_subject(program, machine, embedded, returned);
    let Some(value) = known_exit_integer(program, machine, state, subject) else {
        return false;
    };
    value == literal
}

fn embedded_literal(
    program: &dyn ::validation::CheckingInput,
    embedded: ExpressionHandle,
    literal: ExpressionHandle,
) -> Option<(ExpressionHandle, numerics::bignum::BigInt)> {
    let (_, argument) = validation::integer_embedding_argument(program, embedded)?;
    let literal = program
        .expression_table
        .integer_node(literal)?
        .value_bignum()?;
    Some((argument, literal))
}

fn returned_subject(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    returned: ExpressionHandle,
) -> ExpressionHandle {
    if is_result_reference(program, machine, expression) {
        return returned;
    }
    let Some(member) = program.expression_table.member_node(expression) else {
        return expression;
    };
    if !is_result_reference(program, machine, member.receiver) {
        return expression;
    }
    let Some(literal) = program.expression_table.struct_literal_node(returned) else {
        return expression;
    };
    program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .find(|field| field.name.as_str() == member.member.as_str())
        .map(|field| field.value)
        .unwrap_or(expression)
}

fn known_exit_integer(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<numerics::bignum::BigInt> {
    if let Some(literal) = program.expression_table.integer_node(expression) {
        return literal.value_bignum();
    }
    let member = program.expression_table.member_node(expression)?;
    if member.case_variant.is_some()
        || language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
            member.member.as_str(),
        ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return None;
    }
    let extent =
        validation::declared_collection_extent(program, machine, Some(state), member.receiver)?;
    Some(numerics::bignum::BigInt::from_u64(
        u64::try_from(extent).ok()?,
    ))
}
