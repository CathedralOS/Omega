//! Shared lookup of current assignment values. Storage invalidation owns their
//! lifetime; neither a local initializer nor a nonliteral expression is replayed.

use crate::flow::{
    CanonicalPlace, canonical_place_from_semantic_place, normalized_event_place_root,
};
use psi_facts::{FactContext, FactPayload, FactPlace, FactPlan, PlaceRoot, ScalarValue};
use psi_typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};

pub(crate) fn scalar_value_at_place<'a>(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Option<ScalarValue> {
    let mut retained = None;
    for payload in payloads_at_place(program, semantic, contexts, subject) {
        let incoming = match payload {
            FactPayload::AssignedScalarValue { value } => match semantic.scalar_values.get(value) {
                ScalarValue::Unknown => None,
                value => Some(value.clone()),
            },
            FactPayload::AssignedValue { value } => {
                if !program.expression_table.expression_is_valid(value) {
                    return None;
                }
                match program.expression_table.expression(value) {
                    ExpressionNode::Integer(value) => {
                        value.value_bignum().map(ScalarValue::Integer)
                    }
                    ExpressionNode::Boolean(value) => Some(ScalarValue::Boolean(*value)),
                    _ => None,
                }
            }
            _ => None,
        }?;
        if retained
            .as_ref()
            .is_some_and(|retained| retained != &incoming)
        {
            return None;
        }
        retained = Some(incoming);
    }
    retained
}

pub(crate) fn literal_at_place<'a>(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Option<ExpressionHandle> {
    payloads_at_place(program, semantic, contexts, subject)
        .into_iter()
        .find_map(|payload| match payload {
            FactPayload::AssignedValue { value }
                if program.expression_table.expression_is_valid(value) =>
            {
                Some(value)
            }
            _ => None,
        })
}

fn payloads_at_place<'a>(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Vec<FactPayload> {
    if let PlaceRoot::Expression(expression) = subject.root
        && subject.segments.is_empty()
        && program.expression_table.expression_is_valid(expression)
        && matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::String(_)
        )
    {
        return vec![FactPayload::AssignedValue { value: expression }];
    }
    contexts
        .into_iter()
        .flat_map(|context| {
            semantic.context_view(context).facts().filter_map(|fact| {
                if !matches!(
                    fact.payload,
                    FactPayload::AssignedValue { .. } | FactPayload::AssignedScalarValue { .. }
                ) {
                    return None;
                }
                let FactPlace::Place(place) = fact.place else {
                    return None;
                };
                let candidate = canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                )?;
                (normalized_event_place_root(program, candidate.root)
                    == normalized_event_place_root(program, subject.root)
                    && candidate.segments == subject.segments)
                    .then_some(fact.payload)
            })
        })
        .collect()
}
