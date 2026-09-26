//! Save one completed jump argument while its operand facts are still live.
use super::{ScalarValue, TransitionTargetNode};
use crate::flow::CanonicalPlace;
use crate::flow::FlowBuildContext;
use crate::flow::state_values::literal;
use arena::HandleSpan;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::ExpressionHandle;
use checked_trees::statement::StatementNode;
use facts::FactPlan;

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn capture_argument(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    ordinal: usize,
    argument: ExpressionHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> ScalarValue {
    if !program.expression_table.expression_is_valid(argument) {
        return ScalarValue::Unknown;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(target)
    else {
        return ScalarValue::Unknown;
    };
    if program
        .statement_table
        .expression_handles(*arguments)
        .get(ordinal)
        != Some(&argument)
    {
        return ScalarValue::Unknown;
    }
    let Some(destination) = context
        .state_index_in_machine(program, machine.symbol, path.symbol)
        .and_then(|index| program.machine_states(machine).get(index))
    else {
        return ScalarValue::Unknown;
    };
    let Some((position, parameter)) = program
        .state_parameters(destination)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| !parameter.is_self)
        .nth(ordinal)
    else {
        return ScalarValue::Unknown;
    };
    let (Ok(statement_ordinal), Ok(argument_ordinal)) =
        (u32::try_from(statement_index), u32::try_from(position))
    else {
        return ScalarValue::Unknown;
    };
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return ScalarValue::Unknown;
    };
    let role = if target == transition.target {
        CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
    } else if target == transition.continuation {
        CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
    } else {
        return ScalarValue::Unknown;
    };
    let plans = context.scalar_expressions;
    let mut bindings = context
        .scalar_binding_handles_at(state.symbol, statement_ordinal)
        .iter()
        .map(|handle| plans.source_bindings.get(*handle))
        .filter(|binding| binding.role == role);
    if let Some(binding) = bindings.next() {
        if bindings.next().is_some()
            || binding.expression != argument
            || binding.destination != parameter.symbol
        {
            return ScalarValue::Unknown;
        }
        let mut expressions = context
            .scalar_expression_rows_at(state.symbol, statement_ordinal)
            .iter()
            .map(|row| &plans.expressions[*row])
            .filter(|expression| expression.role == role);
        let Some(expression) = expressions.next() else {
            return ScalarValue::Unknown;
        };
        if expressions.next().is_some() {
            return ScalarValue::Unknown;
        }
        return crate::values::evaluate_checked_scalar(
            &expression.expression,
            &mut crate::values::PlaceScalarValues {
                program,
                parameters: program.state_parameters(state),
                symbols: plans.binding_symbols.span_or_empty(binding.symbols),
                value_at_place: |place: &CanonicalPlace| {
                    crate::values::scalar_value_at_place(
                        program,
                        semantic,
                        context
                            .contexts
                            .semantic_context_refs
                            .span_or_empty(contexts)
                            .iter()
                            .map(|reference| semantic.contexts.get(reference.context)),
                        place,
                    )
                },
            },
        )
        .unwrap_or_default();
    }
    if let Some(value) = literal(program, argument) {
        return value;
    }
    // Projected reads can use existing exact place facts. Dynamic selectors
    // still need their own captured index identity before they carry values.
    let Some(place) = context.canonical_place_at(program, state.symbol, statement_index, argument)
    else {
        return ScalarValue::Unknown;
    };
    if !place.segments.iter().all(|segment| {
        matches!(
            segment,
            facts::PlaceSegment::Field { .. }
                | facts::PlaceSegment::Case { .. }
                | facts::PlaceSegment::FixedIndex { .. }
        )
    }) {
        return ScalarValue::Unknown;
    }
    crate::values::scalar_value_at_place(
        program,
        semantic,
        context
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts)
            .iter()
            .map(|reference| semantic.contexts.get(reference.context)),
        &place,
    )
    .unwrap_or_default()
}
