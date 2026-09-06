//! Shared lookup of current assignment values. Storage invalidation owns their
//! lifetime; neither a local initializer nor a nonliteral expression is replayed.

use crate::flow::{
    CanonicalPlace, canonical_place_from_semantic_place, normalized_event_place_root,
};
use facts::{FactContext, FactPayload, FactPlace, FactPlan, PlaceRoot, ScalarValue};
use typed_trees::{
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
                if program.expression_table.expression_is_valid(value)
                    && matches!(
                        program.expression_table.expression(value),
                        ExpressionNode::Integer(_)
                            | ExpressionNode::Boolean(_)
                            | ExpressionNode::String(_)
                    ) =>
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
                // Call provenance and its completed scalar snapshot share one
                // transfer context and point. Keep unknown calls at other
                // points as blockers; never let one path's snapshot mask them.
                if let FactPayload::AssignedValue { value } = fact.payload
                    && matches!(program.expression_table.expression(value), ExpressionNode::Call(_))
                    && semantic.context_view(context).facts().any(|snapshot| {
                        snapshot.place == fact.place
                            && snapshot.point == fact.point
                            && snapshot.origin == fact.origin
                            && matches!(snapshot.payload, FactPayload::AssignedScalarValue { value }
                                if !matches!(semantic.scalar_values.get(value), ScalarValue::Unknown))
                    })
                {
                    return None;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use facts::{Fact, FactOrigin, ProgramPoint};

    #[test]
    fn call_provenance_requires_its_own_snapshot_and_conflicts_still_reject() {
        let mut program = TypedTrees::default();
        let call = program.expression_table.insert(ExpressionNode::Call(
            typed_trees::expression::TableCallExpression {
                receiver: Default::default(),
                target_symbol: Default::default(),
                target: Default::default(),
                static_requirement_dispatch: None,
                machine_arguments: Default::default(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments: Default::default(),
                evidence_arguments: Default::default(),
                operational_acknowledgement: Default::default(),
            },
        ));
        let symbol = symbols::SymbolHandle::from_arena_index(1);
        let subject = CanonicalPlace {
            root: PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        };
        for (different_point, separate_context, conflicting_value, accepted) in [
            (false, false, false, true),
            (true, false, false, false),
            (false, true, false, false),
            (false, false, true, false),
        ] {
            let mut semantic = FactPlan::default();
            let place = semantic.append_symbol_place(symbol);
            let point = ProgramPoint::Statement {
                machine_symbol: symbol,
                state_symbol: symbol,
                statement_index: 0,
            };
            let snapshot_point = ProgramPoint::Statement {
                machine_symbol: symbol,
                state_symbol: symbol,
                statement_index: usize::from(different_point),
            };
            let call_fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: Default::default(),
                payload: FactPayload::AssignedValue { value: call },
            });
            let mut call_references = Default::default();
            semantic.append_ref(&mut call_references, call_fact);
            let mut scalar_references = Default::default();
            for value in [65, if conflicting_value { 66 } else { 65 }] {
                let value = semantic.scalar_values.append(ScalarValue::Integer(
                    numerics::bignum::BigInt::from_u64(value),
                ));
                let fact = semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point: snapshot_point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: Default::default(),
                    payload: FactPayload::AssignedScalarValue { value },
                });
                if separate_context {
                    semantic.append_ref(&mut scalar_references, fact);
                } else {
                    semantic.append_ref(&mut call_references, fact);
                }
            }
            let call_context = semantic.append_context(point, call_references);
            let scalar_context = semantic.append_context(snapshot_point, scalar_references);
            let value = scalar_value_at_place(
                &program,
                &semantic,
                [
                    semantic.contexts.get(call_context),
                    semantic.contexts.get(scalar_context),
                ],
                &subject,
            );
            assert_eq!(
                value.is_some(),
                accepted,
                "point={different_point} context={separate_context} conflict={conflicting_value}"
            );
        }
    }
}
