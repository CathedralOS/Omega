//! Save one completed jump argument while its operand facts are still live.
use validation::CanonicalPlace;

use super::{ScalarValue, TransitionTargetNode};

use crate::flow::FlowBuildContext;
use crate::flow::literal;
use arena::HandleSpan;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::ExpressionHandle;
use checked_trees::statement::StatementNode;
use facts::FactPlan;

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn capture_argument(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    ordinal: usize,
    argument: ExpressionHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
    copied_values: HandleSpan<checked_trees::FlowOperandFact>,
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
        .nth(ordinal)
    else {
        return ScalarValue::Unknown;
    };
    let (Ok(statement_ordinal), Ok(argument_ordinal)) =
        (u32::try_from(statement_index), u32::try_from(position))
    else {
        return ScalarValue::Unknown;
    };
    let Some(StatementNode::Transition(transition)) =
        program.statement_table.statement_at(state, statement_index)
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
    match context.selected_scalar_at(state.symbol, statement_ordinal, |binding| {
        binding.role == role
    }) {
        // No matching custody row: the literal or projected read below still
        // bounds the delivered value.
        None => {}
        // A conflict is a capture failure, never a reason to re-derive the
        // argument from its declared formal.
        Some(None) => return ScalarValue::Unknown,
        Some(Some((binding, expression))) => {
            if binding.expression != argument || binding.destination != parameter.symbol {
                return ScalarValue::Unknown;
            }
            return crate::values::evaluate_checked_scalar(
                expression,
                &mut crate::values::PlaceScalarValues {
                    program,
                    parameters: program.state_parameters(state),
                    symbols: context
                        .scalar_expressions
                        .binding_symbols
                        .span_or_empty(binding.symbols),
                    value_at_place: |place: &CanonicalPlace, observation| {
                        let place = crate::flow::observations::completed_copy_read_place(
                            program,
                            &context.control,
                            copied_values,
                            state.symbol,
                            statement_index,
                            observation,
                            place,
                        )?;
                        context.scalar_value_at_place(program, semantic, contexts, &place)
                    },
                },
            )
            .unwrap_or_default();
        }
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
    let Some(place) = crate::flow::observations::completed_copy_read_place(
        program,
        &context.control,
        copied_values,
        state.symbol,
        statement_index,
        argument,
        &place,
    ) else {
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
    context
        .scalar_value_at_place(program, semantic, contexts, &place)
        .unwrap_or_default()
}
