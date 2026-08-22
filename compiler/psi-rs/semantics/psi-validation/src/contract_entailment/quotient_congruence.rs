//! Exact quotient-congruence recognition for the contract entailment engine.
//!
//! This owner is deliberately structural: quotient equality is discharged only
//! by the quotient's exact retained relation premise, never by ambient proof
//! discovery or the generic arithmetic/structural fallback tiers.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;

/// Equality of two quotient mints is exactly the quotient relation over their
/// carrier expressions. `Some(false)` means the goal is a well-formed quotient
/// equality but its relation premise is absent; callers reject it instead of
/// letting the generic structural tier stand down.
pub(super) fn quotient_equality_from_requires(
    program: &TypedTrees,
    machine: &Machine,
    requires: &[ExpressionHandle],
    proposition_requires: &[&psi_typed_trees::proposition::PropositionApplication],
    fact: ExpressionHandle,
) -> Option<bool> {
    let (quotient, left, right) = quotient_equality_goal(program, fact)?;
    let relation = quotient.quotient.as_ref()?;
    let expression_match = requires.iter().any(|required| {
        relation_fact_call(program, *required).is_some_and(|call| {
            relation_call_matches_quotient(program, call, relation.relation_symbol)
                && matches!(
                    program.expression_table.expression_handles(call.arguments),
                    [required_left, required_right]
                        if (program.expression_table.expressions_structurally_equal(*required_left, left)
                            && program.expression_table.expressions_structurally_equal(*required_right, right))
                            || (program.expression_table.expressions_structurally_equal(*required_left, right)
                                && program.expression_table.expressions_structurally_equal(*required_right, left))
                )
        }) || transparent_relation_fact_matches(
            program,
            *required,
            relation.relation_symbol,
            left,
            right,
        )
    });
    let left_identity = named_parameter_identity(program, machine, left);
    let right_identity = named_parameter_identity(program, machine, right);
    let proposition_match = proposition_requires.iter().any(|application| {
        let (Some((left_symbol, left_type)), Some((right_symbol, right_type))) =
            (left_identity, right_identity)
        else {
            return false;
        };
        let forward = crate::quotients::exact_relation_application_matches(
            program,
            application,
            relation.relation_symbol,
            left_symbol,
            right_symbol,
            left_type,
            right_type,
        );
        let reverse = crate::quotients::exact_relation_application_matches(
            program,
            application,
            relation.relation_symbol,
            right_symbol,
            left_symbol,
            right_type,
            left_type,
        );
        forward || reverse
    });
    Some(expression_match || proposition_match)
}

fn named_parameter_identity(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    program
        .machine_states(machine)
        .iter()
        .flat_map(|state| program.state_parameters(state))
        .find(|parameter| parameter.symbol == path.symbol)
        .map(|parameter| (parameter.symbol, parameter.type_reference))
}

fn transparent_relation_fact_matches(
    program: &TypedTrees,
    fact: ExpressionHandle,
    relation_symbol: SymbolHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let Some(relation) = program
        .propositions()
        .iter()
        .find(|relation| relation.symbol == relation_symbol)
    else {
        return false;
    };
    let psi_typed_trees::proposition::PropositionBody::Transparent {
        proposition: psi_typed_trees::proposition::PropositionFormula::BooleanExpression(formula),
    } = relation.body
    else {
        return false;
    };
    let [left_parameter, right_parameter] = program.proposition_parameters(relation) else {
        return false;
    };
    let actual = program.render_proof_expression_with_symbols(fact, &[]);
    [(left, right), (right, left)]
        .into_iter()
        .any(|(left, right)| {
            let expected = program.render_proof_expression_with_parameters(
                formula,
                &[
                    (
                        left_parameter.symbol,
                        left_parameter.name.as_str().to_owned(),
                        program.render_proof_expression_with_symbols(left, &[]),
                    ),
                    (
                        right_parameter.symbol,
                        right_parameter.name.as_str().to_owned(),
                        program.render_proof_expression_with_symbols(right, &[]),
                    ),
                ],
            );
            actual == expected
        })
}

pub(super) fn quotient_equality_names(
    program: &TypedTrees,
    fact: ExpressionHandle,
) -> Option<(String, String)> {
    let (definition, _, _) = quotient_equality_goal(program, fact)?;
    let quotient = definition.quotient.as_ref()?;
    Some((
        definition.name.as_str().to_owned(),
        quotient
            .relation
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::"),
    ))
}

fn quotient_equality_goal(
    program: &TypedTrees,
    fact: ExpressionHandle,
) -> Option<(
    &psi_typed_trees::data::DataDefinition,
    ExpressionHandle,
    ExpressionHandle,
)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    let (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) = (
        program.expression_table.expression(binary.left),
        program.expression_table.expression(binary.right),
    ) else {
        return None;
    };
    if left.form.is_recast() || right.form.is_recast() {
        return None;
    }
    let left_name = program.named_type_reference(left.target_type)?;
    let right_name = program.named_type_reference(right.target_type)?;
    if left_name.as_str() != right_name.as_str() {
        return None;
    }
    let quotient = program.data_definitions().iter().find(|definition| {
        definition.name.as_str() == left_name.as_str() && definition.quotient.is_some()
    })?;
    Some((quotient, left.value, right.value))
}

fn relation_fact_call(
    program: &TypedTrees,
    fact: ExpressionHandle,
) -> Option<&psi_typed_trees::expression::TableCallExpression> {
    match program.expression_table.expression(fact) {
        ExpressionNode::Call(call) => Some(call),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                relation_fact_call(program, binary.left)
            } else if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Boolean(true)
            ) {
                relation_fact_call(program, binary.right)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn relation_call_matches_quotient(
    program: &TypedTrees,
    call: &psi_typed_trees::expression::TableCallExpression,
    relation_symbol: SymbolHandle,
) -> bool {
    if call.target_symbol == relation_symbol {
        return true;
    }
    let relation_name = program
        .data_definitions()
        .iter()
        .filter_map(|definition| definition.quotient.as_ref())
        .find(|quotient| quotient.relation_symbol == relation_symbol)
        .and_then(|quotient| quotient.relation.last())
        .map(|name| name.as_str());
    if relation_name.is_some_and(|name| call.target.as_str() == name) {
        return true;
    }
    program.machines().iter().any(|machine| {
        (machine.symbol == relation_symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == relation_symbol))
            && program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
    })
}
