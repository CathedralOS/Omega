//! Semantic predicate transport through selected float result equations.
//!
//! Public total wellness queries can state a sealed predicate over a private
//! projection in their guarantees. An observed Boolean fact therefore proves
//! that predicate. Quiet float-producing expressions supply their selected
//! raw equation compositionally; an adapted result supplies no unconditional
//! raw equation. Names, carriers, format constants and selected declarations
//! remain attached to the exact source expressions throughout substitution.

use checked_trees::{CheckFacts, CheckedArithmeticPolicyAdapter};
use facts::{FactContextHandle, FactPayload};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::OperatorDefinition;
use typed_trees::signature::StateParameter;

/// Extend the ordinary exact Boolean fact query with the selected callable's
/// sealed semantic equation. Instantiated facts without retained caller
/// expression coordinates remain opaque rather than reusing formal storage.
pub(crate) fn context_proves_float_semantic_predicate(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    context: FactContextHandle,
    required: &str,
) -> bool {
    facts
        .semantic
        .context_view(facts.semantic.contexts.get(context))
        .facts()
        .any(|fact| {
            let expression = match fact.payload {
                FactPayload::BooleanExpression(expression)
                | FactPayload::BooleanValue {
                    expression,
                    value: true,
                } => expression,
                FactPayload::ContractBooleanExpression {
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => expression,
                _ => return false,
            };
            positive_expression_proves(program, facts, expression, required)
        })
}

fn positive_expression_proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    required: &str,
) -> bool {
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == BinaryOperator::And
    {
        return positive_expression_proves(program, facts, binary.left, required)
            || positive_expression_proves(program, facts, binary.right, required);
    }
    let Some((operator, operands, adapter)) = selected_expression(program, facts, expression)
    else {
        return false;
    };
    if adapter != CheckedArithmeticPolicyAdapter::None {
        return false;
    }
    let Some(predicate) = validation::float_boolean_semantic_contract(program, operator) else {
        return false;
    };
    let inputs = program
        .operator_parameters(operator)
        .iter()
        .zip(&operands)
        .map(|(parameter, operand)| (parameter.symbol, *operand))
        .collect::<Vec<_>>();
    let mut remaining_work = 1024;
    semantic_label(program, facts, predicate, &inputs, &mut remaining_work).as_deref()
        == Some(required)
}

/// Instantiate the exact raw result term with the same normalization as a
/// public predicate's guarantees. Value-preserving qualification casts do not
/// change a proof denotation; result adapters do.
pub(crate) fn instantiated_float_semantic_label(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
) -> Option<String> {
    if parameters.len() != operands.len() {
        return None;
    }
    let inputs = parameters
        .iter()
        .zip(operands)
        .map(|(parameter, operand)| (parameter.symbol, *operand))
        .collect::<Vec<_>>();
    let mut remaining_work = 1024;
    semantic_label(program, facts, expression, &inputs, &mut remaining_work)
}

fn semantic_label(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    inputs: &[(symbols::SymbolHandle, ExpressionHandle)],
    remaining_work: &mut usize,
) -> Option<String> {
    *remaining_work = remaining_work.checked_sub(1)?;
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some((_, operand)) = inputs
            .iter()
            .find(|(parameter, _)| *parameter == path.symbol)
    {
        return semantic_label(program, facts, *operand, &[], remaining_work);
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return Some(program.render_proof_expression(
            expression,
            typed_trees::proposition::ProofSubstitutions::None,
        ));
    };
    let arguments = program.expression_table.expression_handles(call.arguments);
    if let Some((_, _, projection, _)) =
        validation::exact_toolchain_float_projection_operation(program, call)
    {
        let [source] = arguments else {
            return None;
        };
        let source = substituted_source(program, *source, inputs);
        if let Some((selected, operands, CheckedArithmeticPolicyAdapter::None)) =
            selected_expression(program, facts, source)
            && let Some(raw) = validation::float_result_semantic_contract(program, selected)
        {
            let inputs = program
                .operator_parameters(selected)
                .iter()
                .zip(&operands)
                .map(|(parameter, operand)| (parameter.symbol, *operand))
                .collect::<Vec<_>>();
            return semantic_label(program, facts, raw, &inputs, remaining_work);
        }
        let source_label = program
            .render_proof_expression(source, typed_trees::proposition::ProofSubstitutions::None);
        return Some(format!(
            "{}::{}({source_label})",
            projection.source_namespace(),
            projection.source_name()
        ));
    }
    let selected = typed_trees::operator::resolve_named_expression_call(program, call)?;
    validation::exact_toolchain_float_semantic_contract(program, selected)?;
    let argument_labels = arguments
        .iter()
        .map(|argument| semantic_label(program, facts, *argument, inputs, remaining_work))
        .collect::<Option<Vec<_>>>()?;
    let name = program
        .operator_path_members(selected.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    Some(format!("{name}({})", argument_labels.join(", ")))
}

fn substituted_source(
    program: &dyn ::validation::CheckingInput,
    mut expression: ExpressionHandle,
    inputs: &[(symbols::SymbolHandle, ExpressionHandle)],
) -> ExpressionHandle {
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some((_, operand)) = inputs
            .iter()
            .find(|(parameter, _)| *parameter == path.symbol)
    {
        expression = *operand;
    }
    while let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) {
        if cast.form.is_recast() {
            break;
        }
        expression = cast.value;
    }
    expression
}

fn selected_expression<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
) -> Option<(
    &'program OperatorDefinition,
    Vec<ExpressionHandle>,
    CheckedArithmeticPolicyAdapter,
)> {
    let mut selections = facts
        .operators
        .resolved_expression_selections()
        .filter(|(selected, _, _, _)| *selected == expression);
    let (_, _, selected, adapter) = selections.next()?;
    if selections.next().is_some() {
        return None;
    }
    let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
    let operands = if let Some((_, selected_use)) =
        facts.operators.uses.iter().find(|(_, selected_use)| {
            selected_use.expression == expression
                && selected_use.selected_operator_symbol == selected
        }) {
        selected_use.operands(program)?
    } else {
        let call = program.expression_table.call_node(expression)?;
        crate::facts::operator_crashes::named_call_operands(
            program,
            call,
            program.operator_parameters(operator),
        )?
    };
    (operands.len() == program.operator_parameters(operator).len())
        .then_some((operator, operands, adapter))
}
