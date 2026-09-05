use psi_facts::FactPlace;
use psi_symbols::SymbolHandle;

use super::labels::{ContractTargetParameters, instantiate_call_contract_expression_label};
use super::places::{expression_is_boolean_place_like, expression_place_matches};
use crate::labels::{canonical_place_label, semantic_boolean_fact_label};

mod guard_values;

pub(super) fn direct_context_proves_boolean_expression(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    context: &psi_facts::FactContext,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let required_label = program.expression_table.display_name(expression);

    semantic.context_view(context).facts().any(|fact| {
        if let psi_facts::FactPayload::BooleanValue {
            expression: guard,
            value,
        } = fact.payload
        {
            return guard_values::proves(program, guard, value, expression, true);
        }
        let candidate_label = semantic_boolean_fact_label(program, semantic, fact).or_else(|| {
            semantic
                .proposition_fact_label(program, fact)
                .and_then(|label| label.strip_prefix("boolean:").map(str::to_owned))
        });
        let Some(candidate_label) = candidate_label else {
            return false;
        };

        candidate_label == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if expression_place_matches(program, semantic, expression, candidate_place)))
    })
}

pub(super) fn direct_context_proves_instantiated_boolean_expression(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    context: &psi_facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    if instantiated_live_value_proves(
        program,
        semantic,
        context,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    ) {
        return true;
    }
    let required_label = instantiate_call_contract_expression_label(
        program,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    );

    semantic.context_view(context).facts().any(|fact| {
        let candidate_label = semantic_boolean_fact_label(program, semantic, fact).or_else(|| {
            semantic
                .proposition_fact_label(program, fact)
                .and_then(|label| label.strip_prefix("boolean:").map(str::to_owned))
        });
        let Some(candidate_label) = candidate_label else {
            return false;
        };

        candidate_label == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        expression,
                ) == canonical_place_label(
                    program,
                    semantic,
                    semantic.places.get(candidate_place),
                )))
    })
}

#[allow(clippy::too_many_arguments)]
fn instantiated_live_value_proves(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    context: &psi_facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target: &(impl ContractTargetParameters + ?Sized),
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    use super::prover::{ScalarValue, evaluate_scalar};
    use psi_typed_trees::expression::ExpressionNode;

    let parameters = target.contract_parameters(program);
    let arguments = crate::call_site_argument_expressions(program, call_site);
    evaluate_scalar(program, expression, &mut |formal| {
        let ExpressionNode::Name(path) = program.expression_table.expression(formal) else {
            return None;
        };
        if !path.symbol.is_valid()
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        let argument = if parameters
            .iter()
            .any(|parameter| parameter.is_self && parameter.symbol == path.symbol)
        {
            match call_site {
                crate::CallSite::Expression { call, .. } => call.receiver,
                _ => return None,
            }
        } else {
            let position = parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|parameter| parameter.symbol == path.symbol)?;
            *arguments.get(position)?
        };
        evaluate_scalar(program, argument, &mut |leaf| {
            // Local declarations are not current values. Read only assignment
            // snapshots which survived the scheduled argument effects.
            if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                caller_state_symbol,
                statement_index,
                leaf,
            ) && let Some(value) =
                super::prover::scalar_value_at_place(program, semantic, [context], &place)
            {
                return Some(value);
            }
            semantic.context_view(context).facts().find_map(|fact| {
                let psi_facts::FactPayload::BooleanValue {
                    expression: guard,
                    value,
                } = fact.payload
                else {
                    return None;
                };
                [true, false]
                    .into_iter()
                    .find(|required| guard_values::proves(program, guard, value, leaf, *required))
                    .map(ScalarValue::Boolean)
            })
        })
    }) == Some(ScalarValue::Boolean(true))
}
