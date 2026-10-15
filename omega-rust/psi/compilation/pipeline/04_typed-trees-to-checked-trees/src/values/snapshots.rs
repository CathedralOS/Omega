//! Shared lookup of current assignment values. Storage invalidation owns their
//! lifetime; neither a local initializer nor a nonliteral expression is replayed.

use validation::CanonicalPlace;

use crate::flow::{canonical_place_from_semantic_place, canonical_place_from_symbol};
use facts::{FactContext, FactPayload, FactPlan, ScalarValue};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Read scalar bindings and structural fields from the same live storage facts.
/// Bindings use dense scalar positions; structural paths use authored parameters.
pub(crate) struct PlaceScalarValues<'a, Resolve> {
    pub program: &'a dyn ::validation::CheckingInput,
    pub parameters: &'a [typed_trees::signature::StateParameter],
    pub symbols: &'a [symbols::SymbolHandle],
    pub value_at_place: Resolve,
}

impl<Resolve: FnMut(&CanonicalPlace) -> Option<ScalarValue>> super::ScalarValueSource
    for PlaceScalarValues<'_, Resolve>
{
    fn binding(&mut self, position: usize) -> Option<ScalarValue> {
        self.storage(*self.symbols.get(position)?)
    }

    fn storage(&mut self, symbol: symbols::SymbolHandle) -> Option<ScalarValue> {
        (self.value_at_place)(&canonical_place_from_symbol(symbol)?)
    }

    fn structural_field(
        &mut self,
        parameter_position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Option<ScalarValue> {
        if path.is_empty() {
            return None;
        }
        let (symbol, segments, _, _) = super::resolve_structural_parameter_path(
            self.program,
            self.parameters,
            parameter_position,
            path,
        )?;
        let mut place = canonical_place_from_symbol(symbol)?;
        place.segments = segments;
        (self.value_at_place)(&place)
    }
}

pub(crate) fn scalar_value_at_place<'a>(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Option<ScalarValue> {
    scalar_value_from_payloads(
        program,
        semantic,
        payloads_at_place(program, semantic, contexts, subject),
    )
}

pub(crate) fn scalar_value_from_payloads(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    payloads: impl IntoIterator<Item = FactPayload>,
) -> Option<ScalarValue> {
    let mut retained = None;
    for payload in payloads {
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
            // A singleton interval is the completed observation itself: the
            // captured bounds assert the place held exactly this endpoint at
            // the fact's program point. A wider interval remains evidence
            // against an exact read and still blocks it.
            FactPayload::AssignedIntegerBounds { bounds }
                if semantic.integer_ranges.is_valid(bounds) =>
            {
                let bounds = semantic.integer_ranges.get(bounds);
                (bounds.minimum == bounds.maximum)
                    .then(|| ScalarValue::Integer(bounds.minimum.clone()))
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
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Option<ExpressionHandle> {
    let mut retained = None;
    for payload in payloads_at_place(program, semantic, contexts, subject) {
        let value = match payload {
            FactPayload::AssignedValue { value } if is_assignment_literal(program, value) => value,
            _ => return None,
        };
        if retained.is_some_and(|previous| previous != value) {
            return None;
        }
        retained = Some(value);
    }
    retained
}

pub(crate) fn integer_bounds_at_place<'a>(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Option<facts::IntegerRange> {
    integer_bounds_from_payloads(
        program,
        semantic,
        payloads_at_place(program, semantic, contexts, subject),
    )
}

pub(crate) fn integer_bounds_from_payloads(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    payloads: impl IntoIterator<Item = FactPayload>,
) -> Option<facts::IntegerRange> {
    let mut retained: Option<facts::IntegerRange> = None;
    for payload in payloads {
        let incoming = match payload {
            FactPayload::AssignedIntegerBounds { bounds }
                if semantic.integer_ranges.is_valid(bounds) =>
            {
                semantic.integer_ranges.get(bounds).clone()
            }
            FactPayload::AssignedScalarValue { value } => {
                let ScalarValue::Integer(value) = semantic.scalar_values.get(value) else {
                    return None;
                };
                facts::IntegerRange {
                    minimum: value.clone(),
                    maximum: value.clone(),
                }
            }
            FactPayload::AssignedValue { value }
                if program.expression_table.expression_is_valid(value) =>
            {
                let literal = program.expression_table.integer_node(value)?;
                let value = literal.value_bignum()?;
                facts::IntegerRange {
                    minimum: value.clone(),
                    maximum: value,
                }
            }
            _ => return None,
        };
        if incoming.minimum > incoming.maximum {
            return None;
        }
        retained = Some(match retained {
            Some(previous) => facts::IntegerRange {
                minimum: previous.minimum.min(incoming.minimum),
                maximum: previous.maximum.max(incoming.maximum),
            },
            None => incoming,
        });
    }
    retained
}

fn payloads_at_place<'a>(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Vec<FactPayload> {
    if let Some(payload) = literal_assignment_payload(program, subject) {
        return vec![payload];
    }
    contexts
        .into_iter()
        .flat_map(|context| {
            semantic.context_view(context).facts().filter_map(|fact| {
                if !matches!(
                    fact.payload,
                    FactPayload::AssignedValue { .. }
                        | FactPayload::AssignedScalarValue { .. }
                        | FactPayload::AssignedIntegerBounds { .. }
                ) {
                    return None;
                }
                let place = fact.place.as_place()?;
                let candidate = canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                )?;
                if !crate::flow::canonical_places_agree(program, &candidate, subject) {
                    return None;
                }
                if has_completed_call_snapshot(program, semantic, context, fact) {
                    return None;
                }
                Some(fact.payload)
            })
        })
        .collect()
}

fn has_completed_call_snapshot(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    context: &FactContext,
    fact: &facts::Fact,
) -> bool {
    // Only this exact occurrence can replace call provenance. Unknown calls at
    // another point or origin remain blockers even when the place is the same.
    let FactPayload::AssignedValue { value } = fact.payload else {
        return false;
    };
    matches!(
        program.expression_table.expression(value),
        ExpressionNode::Call(_)
    ) && semantic.context_view(context).facts().any(|snapshot| {
        snapshot.place == fact.place
            && snapshot.point == fact.point
            && snapshot.origin == fact.origin
            && match snapshot.payload {
                FactPayload::AssignedScalarValue { value } => {
                    !matches!(semantic.scalar_values.get(value), ScalarValue::Unknown)
                }
                // Bounds are a completed observation too, without asserting
                // that the call has one exact value.
                FactPayload::AssignedIntegerBounds { bounds } => {
                    semantic.integer_ranges.is_valid(bounds)
                }
                _ => false,
            }
    })
}

/// Literal payloads are immutable authored values, including the exact landed
/// float bits. Keeping their handles does not re-evaluate a source expression.
/// Copies and state-edge joins use this same admission as direct assignments.
pub(crate) fn is_assignment_literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    program.expression_table.expression_is_valid(expression)
        && (typed_trees::byte_predicates::literal_bytes(program, expression).is_some()
            || matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Integer(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::Boolean(_)
                    | ExpressionNode::String(_)
            ))
}

pub(crate) fn literal_assignment_payload(
    program: &dyn ::validation::CheckingInput,
    subject: &CanonicalPlace,
) -> Option<FactPayload> {
    if let Some(expression) = subject.root.as_expression()
        && subject.segments.is_empty()
        && is_assignment_literal(program, expression)
    {
        Some(FactPayload::AssignedValue { value: expression })
    } else {
        None
    }
}

/// Canonical assignment inputs from one immutable published context. The flow
/// builder may retain these only until its semantic sweep is discarded.
#[derive(Debug)]
pub(crate) struct AssignmentObservation {
    pub place: CanonicalPlace,
    pub payload: FactPayload,
}

pub(crate) fn prepare_assignment_observations(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    context: &FactContext,
) -> Vec<AssignmentObservation> {
    semantic
        .context_view(context)
        .facts()
        .filter_map(|fact| {
            if !matches!(
                fact.payload,
                FactPayload::AssignedValue { .. }
                    | FactPayload::AssignedScalarValue { .. }
                    | FactPayload::AssignedIntegerBounds { .. }
            ) {
                return None;
            }
            let place = fact.place.as_place()?;
            let place =
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))?;
            if has_completed_call_snapshot(program, semantic, context, fact) {
                return None;
            }
            Some(AssignmentObservation {
                place,
                payload: fact.payload,
            })
        })
        .collect()
}
