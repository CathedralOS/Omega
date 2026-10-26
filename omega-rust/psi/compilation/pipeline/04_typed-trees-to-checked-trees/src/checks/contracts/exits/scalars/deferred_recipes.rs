//! Exact return substitution transports a predicate over closed recipe terms.
//!
//! This is conditional congruence, never a guessed endpoint value. Immutable
//! invocation origins establish the returned scalar; identical closed source
//! applications supply paired recipe demands. Every pair independently forms,
//! evaluates, and agrees under the owning target before publication. A failed
//! conjunction/candidate publishes no partial debt or proof.

use super::{ExitScalars, is_result_reference};
use checked_trees::{
    CheckFacts, ConstantRecipeObligation, ConstantRecipeRequirement, FlowExitFact,
};
use facts::{FactContextHandle, FactPayload};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(in super::super) fn proves_with_recipe_conditions<'program>(
    program: &'program dyn validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    contexts: &[FactContextHandle],
    requirement: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
) -> Option<Vec<ConstantRecipeObligation>> {
    let FactPayload::ContractBooleanExpression {
        fact: contract,
        expression,
        ..
    } = requirement.payload
    else {
        return None;
    };
    let machine = program.machine(exit.machine_symbol)?;
    let entry = program.machine_states(machine).first()?;
    let evaluator = ExitScalars {
        program,
        facts,
        machine,
        exit,
        contexts,
        contract,
        call_frames,
    };
    let mut hypotheses = Vec::new();
    for expression in program.contract_expressions(
        program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(entry).iter()),
        typed_trees::signature::SignatureContractKind::Requires,
    ) {
        conjuncts(&evaluator, expression, &mut hypotheses)?;
    }
    let mut conditions = Vec::new();
    let mut used_result = false;
    prove(
        &evaluator,
        expression,
        &hypotheses,
        &mut conditions,
        &mut used_result,
    )?;
    (used_result && !conditions.is_empty()).then_some(conditions)
}

fn builtin(evaluator: &ExitScalars<'_, '_>, expression: ExpressionHandle) -> bool {
    super::has_builtin_operators(evaluator.program, &evaluator.facts.operators, expression)
        && validation::has_builtin_bound_expression_meaning(
            evaluator.program,
            evaluator.machine,
            evaluator.program.machine_states(evaluator.machine).first(),
            expression,
        )
}

fn conjuncts(
    evaluator: &ExitScalars<'_, '_>,
    expression: ExpressionHandle,
    output: &mut Vec<ExpressionHandle>,
) -> Option<()> {
    if let Some(binary) = evaluator.program.expression_table.binary_node(expression)
        && binary.operator == BinaryOperator::And
        && builtin(evaluator, expression)
    {
        conjuncts(evaluator, binary.left, output)?;
        conjuncts(evaluator, binary.right, output)?;
    } else {
        output.push(expression);
    }
    Some(())
}

fn prove(
    evaluator: &ExitScalars<'_, '_>,
    goal: ExpressionHandle,
    hypotheses: &[ExpressionHandle],
    conditions: &mut Vec<ConstantRecipeObligation>,
    used_result: &mut bool,
) -> Option<()> {
    if let Some(binary) = evaluator.program.expression_table.binary_node(goal)
        && binary.operator == BinaryOperator::And
        && builtin(evaluator, goal)
    {
        prove(evaluator, binary.left, hypotheses, conditions, used_result)?;
        return prove(evaluator, binary.right, hypotheses, conditions, used_result);
    }
    for hypothesis in hypotheses {
        let mut candidate = Vec::new();
        let mut candidate_result = false;
        if congruent(
            evaluator,
            *hypothesis,
            goal,
            &mut candidate,
            &mut candidate_result,
            0,
        ) {
            conditions.extend(candidate);
            *used_result |= candidate_result;
            return Some(());
        }
    }
    None
}

fn congruent(
    evaluator: &ExitScalars<'_, '_>,
    hypothesis: ExpressionHandle,
    goal: ExpressionHandle,
    conditions: &mut Vec<ConstantRecipeObligation>,
    used_result: &mut bool,
    depth: usize,
) -> bool {
    if depth >= 64 {
        return false;
    }
    let program = evaluator.program;
    let expressions = &program.expression_table;
    if is_result_reference(program, evaluator.machine, goal) {
        let Some(name) = expressions.name_path_node(hypothesis) else {
            return false;
        };
        if name.symbol.is_valid()
            && name.symbol == name.head_symbol
            && expressions.name_path_members(name.members).len() == 1
            && evaluator.returned_immutable_parameter(name.symbol)
        {
            *used_result = true;
            return true;
        }
        return false;
    }
    let left = program.constant_recipe_for_contract_expression(hypothesis);
    let right = program.constant_recipe_for_contract_expression(goal);
    if left.is_valid() && right.is_valid() {
        if !closed_terms_identical(program, hypothesis, goal, depth + 1) {
            return false;
        }
        if left != right {
            conditions.push(ConstantRecipeObligation::retain(
                evaluator.machine.symbol,
                left,
                ConstantRecipeRequirement::EqualRecipe(right),
                expressions.source_span(goal),
                "normal return preserves its requires predicate under exact scalar identity".into(),
            ));
        }
        return true;
    }
    match (
        expressions.expression(hypothesis),
        expressions.expression(goal),
    ) {
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.symbol.is_valid()
                && left.symbol == left.head_symbol
                && right.symbol == right.head_symbol
                && left.symbol == right.symbol
                && expressions.name_path_members(left.members).len() == 1
                && expressions.name_path_members(right.members).len() == 1
                && program
                    .machine_states(evaluator.machine)
                    .first()
                    .and_then(|entry| program.state_parameter_by_symbol(entry, left.symbol))
                    .is_some_and(|parameter| {
                        !parameter.is_mutable
                            && !parameter.is_const
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some_and(|primitive| {
                                    primitive.is_integer()
                                        || primitive == typed_trees::types::PrimitiveType::Bool
                                })
                    })
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && builtin(evaluator, hypothesis)
                && builtin(evaluator, goal)
                && congruent(
                    evaluator,
                    left.left,
                    right.left,
                    conditions,
                    used_result,
                    depth + 1,
                )
                && congruent(
                    evaluator,
                    left.right,
                    right.right,
                    conditions,
                    used_result,
                    depth + 1,
                )
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && builtin(evaluator, hypothesis)
                && builtin(evaluator, goal)
                && congruent(
                    evaluator,
                    left.operand,
                    right.operand,
                    conditions,
                    used_result,
                    depth + 1,
                )
        }
        _ => false,
    }
}

/// Ordinary expression equality deliberately ignores generic call metadata in
/// some source-reuse queries. Contract denotation identity cannot: retain the
/// whole applied selector and every nested actual in the same operand order.
fn closed_terms_identical(
    program: &dyn validation::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 64 {
        return false;
    }
    let expressions = &program.expression_table;
    if !expressions.expressions_structurally_equal(left, right) {
        return false;
    }
    match (expressions.expression(left), expressions.expression(right)) {
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            left.target_symbol.is_valid()
                && left.target_symbol == right.target_symbol
                && left.machine_arguments == right.machine_arguments
                && left.authored_machine_application == right.authored_machine_application
                && left.requirement_application == right.requirement_application
                && left.static_machine_parameter == right.static_machine_parameter
                && left.static_requirement_dispatch == right.static_requirement_dispatch
                && left.evidence_arguments == right.evidence_arguments
                && left.quotient_operation == right.quotient_operation
                && left.private_layout_operation == right.private_layout_operation
                && left.layout_query == right.layout_query
                && left.native_code_layout == right.native_code_layout
                && left.build_selection == right.build_selection
                && expressions
                    .expression_handles(left.arguments)
                    .iter()
                    .zip(expressions.expression_handles(right.arguments))
                    .all(|(left, right)| closed_terms_identical(program, *left, *right, depth + 1))
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            closed_terms_identical(program, left.left, right.left, depth + 1)
                && closed_terms_identical(program, left.right, right.right, depth + 1)
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            closed_terms_identical(program, left.operand, right.operand, depth + 1)
        }
        (ExpressionNode::Integer(_), ExpressionNode::Integer(_))
        | (ExpressionNode::Boolean(_), ExpressionNode::Boolean(_)) => true,
        _ => left == right,
    }
}
