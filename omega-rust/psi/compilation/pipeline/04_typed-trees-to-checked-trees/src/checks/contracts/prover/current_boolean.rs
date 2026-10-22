//! Boolean observations of one exact live storage place.
//!
//! Call requirements and crash-route substitution consume the same fact join.
//! The caller supplies current contexts and owns operand freshness; this query
//! neither recovers an old initializer nor grants immutable entry identity.

use validation::CanonicalPlace;

use facts::{FactOrigin, FactPayload, FactPlan, ProgramPoint, ScalarValue};

pub(crate) fn boolean_value_at_place(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    point: ProgramPoint,
    actual: &CanonicalPlace,
) -> Option<bool> {
    // This query observes a Boolean value, not the truth of an arbitrary
    // predicate about a place. Whole aggregate transfers visit many numeric
    // and authority fields; rebuilding every retained Boolean predicate for
    // each non-Boolean field makes that transport quadratic in its fact set.
    // Unknown types retain the full judgment; a known different carrier cannot
    // have a Boolean value, regardless of which facts mention its storage.
    let scope = match point {
        ProgramPoint::State { state_symbol, .. } => Some((state_symbol, 0)),
        ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::Call {
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::CallRequires {
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::CallEnsures {
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::TransitionArm {
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::Exit {
            state_symbol,
            statement_index,
            ..
        } => Some((state_symbol, statement_index)),
        _ => None,
    };
    if let Some(reference) = scope
        .and_then(|(state, statement)| {
            validation::canonical_place_type_reference(program, state, statement, actual)
        })
        .and_then(|reference| {
            validation::field_domains::readable_type_reference(program, reference)
        })
        && program
            .primitive_type_reference(reference)
            .is_some_and(|primitive| primitive != typed_trees::types::PrimitiveType::Bool)
    {
        return None;
    }
    if let Some(value) = ordinary_boolean_value_at_place(program, semantic, contexts, actual) {
        return Some(value);
    }
    let predicates =
        crate::facts::boolean_predicates::retained(program, operators, semantic, contexts, point);
    crate::facts::boolean_predicates::inferred_boolean_at_place(&predicates, actual, |place| {
        ordinary_boolean_value_at_place(program, semantic, contexts, place)
            .map(ScalarValue::Boolean)
            .or_else(|| {
                crate::values::scalar_value_at_place(
                    program,
                    semantic,
                    contexts
                        .iter()
                        .map(|context| semantic.contexts.get(*context)),
                    place,
                )
            })
    })
}

fn ordinary_boolean_value_at_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    actual: &CanonicalPlace,
) -> Option<bool> {
    let mut retained = match super::scalar_value_at_place(
        program,
        semantic,
        contexts
            .iter()
            .map(|context| semantic.contexts.get(*context)),
        actual,
    ) {
        Some(ScalarValue::Boolean(value)) => Some(value),
        Some(_) => return None,
        None => None,
    };
    for fact in contexts.iter().flat_map(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
    }) {
        let (expression, value) = match fact.payload {
            FactPayload::BooleanExpression(expression)
            | FactPayload::ContractBooleanExpression { expression, .. } => (expression, true),
            FactPayload::BooleanValue { expression, value } => (expression, value),
            _ => continue,
        };
        let Some(place) = fact.place.as_place() else {
            continue;
        };
        if crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(place),
        )
        .as_ref()
            != Some(actual)
        {
            continue;
        }
        let matches_subject = |candidate| {
            let Some(place) = super::field_actuals::checked_place(program, candidate) else {
                return false;
            };
            if fact.origin == FactOrigin::CallEnsures
                && matches!(fact.point, ProgramPoint::CallEnsures { .. })
            {
                // The fact's storage dependency is not the identity of every
                // leaf in its payload. Substitute this exact formal through
                // the originating invocation before comparing the full place.
                postcondition_place(program, fact.point, candidate).as_ref() == Some(actual)
            } else {
                place == *actual
            }
        };
        let mut occurrences = Vec::new();
        crate::facts::contract_occurrences::append_expression_occurrences(
            program,
            expression,
            &mut occurrences,
        );
        for leaf in occurrences
            .into_iter()
            .filter(|leaf| matches_subject(*leaf))
        {
            for required_value in [false, true] {
                if !super::super::direct::guard_value_proves_with(
                    program,
                    expression,
                    value,
                    leaf,
                    required_value,
                    &|candidate, required| required == leaf && matches_subject(candidate),
                ) {
                    continue;
                }
                if retained.is_some_and(|previous| previous != required_value) {
                    return None;
                }
                retained = Some(required_value);
            }
        }
    }
    retained
}

fn postcondition_place(
    program: &dyn ::validation::CheckingInput,
    point: ProgramPoint,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CanonicalPlace> {
    let ProgramPoint::CallEnsures {
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    } = point
    else {
        return None;
    };
    let site = validation::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    )?;
    let target = match &site {
        validation::CallSite::Statement(call) => call.target_symbol,
        validation::CallSite::Expression { call, .. } => call.target_symbol,
        validation::CallSite::TransitionNamed { path, .. } => path.symbol,
    };
    let parameters = validation::call_target_parameters(program, target)?;
    let (value, remaining) = crate::semantic::places::call_contract_argument_projection(
        program,
        parameters,
        validation::call_site_argument_expressions(program, &site),
        expression,
    )?;
    super::field_actuals::checked_place(program, value)?;
    let mut actual = validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        value,
    )?;
    actual.segments.extend(remaining);
    Some(actual)
}
