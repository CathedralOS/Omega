//! Shared lookup of current assignment values. Storage invalidation owns their
//! lifetime; neither a local initializer nor a nonliteral expression is replayed.

use crate::flow::{
    CanonicalPlace, canonical_place_from_semantic_place, canonical_place_from_symbol,
    normalized_event_place_root,
};
use facts::{FactContext, FactPayload, FactPlace, FactPlan, PlaceRoot, ScalarValue};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};

/// Read scalar bindings and structural fields from the same live storage facts.
/// Bindings use dense scalar positions; structural paths use authored parameters.
pub(crate) struct PlaceScalarValues<'a, Resolve> {
    pub program: &'a TypedTrees,
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
        let (symbol, segments, _) = super::resolve_structural_parameter_path(
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
    let mut retained = None;
    for payload in payloads_at_place(program, semantic, contexts, subject) {
        let value = match payload {
            FactPayload::AssignedValue { value }
                if program.expression_table.expression_is_valid(value)
                    && matches!(
                        program.expression_table.expression(value),
                        ExpressionNode::Integer(_)
                            | ExpressionNode::Boolean(_)
                            | ExpressionNode::String(_)
                    ) =>
            {
                value
            }
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
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: impl IntoIterator<Item = &'a FactContext>,
    subject: &CanonicalPlace,
) -> Option<facts::IntegerRange> {
    let mut retained: Option<facts::IntegerRange> = None;
    for payload in payloads_at_place(program, semantic, contexts, subject) {
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
                let ExpressionNode::Integer(literal) = program.expression_table.expression(value)
                else {
                    return None;
                };
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
                        | FactPayload::AssignedIntegerBounds { .. }
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
    fn byte_literals_reject_unknown_and_conflicting_live_snapshots() {
        let mut program = TypedTrees::default();
        let first = program
            .expression_table
            .insert(ExpressionNode::String(std::sync::Arc::from(&b"ABC"[..])));
        let second = program
            .expression_table
            .insert(ExpressionNode::String(std::sync::Arc::from(&b"XYZ"[..])));
        let symbol = symbols::SymbolHandle::from_arena_index(1);
        let subject = canonical_place_from_symbol(symbol).expect("symbol place");
        for (other, accepted) in [
            (first, true),
            (second, false),
            (ExpressionHandle::invalid(), false),
        ] {
            for reverse in [false, true] {
                let mut semantic = FactPlan::default();
                let place = semantic.append_symbol_place(symbol);
                let mut references = Default::default();
                let values = if reverse {
                    [other, first]
                } else {
                    [first, other]
                };
                for value in values {
                    let fact = semantic.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point: ProgramPoint::default(),
                        origin: FactOrigin::StatementTransfer,
                        evidence: Default::default(),
                        payload: FactPayload::AssignedValue { value },
                    });
                    semantic.append_ref(&mut references, fact);
                }
                let context = semantic.append_context(ProgramPoint::default(), references);
                assert_eq!(
                    literal_at_place(
                        &program,
                        &semantic,
                        [semantic.contexts.get(context)],
                        &subject
                    )
                    .is_some(),
                    accepted
                );
            }
        }
    }

    #[test]
    fn integer_bounds_hull_live_values_and_reject_invalid_range_handles() {
        let program = TypedTrees::default();
        let symbol = symbols::SymbolHandle::from_arena_index(1);
        let subject = canonical_place_from_symbol(symbol).expect("symbol place");
        for corrupt in [false, true] {
            let mut semantic = FactPlan::default();
            let place = semantic.append_symbol_place(symbol);
            let mut references = Default::default();
            for (minimum, maximum) in [(65, 70), (75, 80)] {
                let bounds = semantic.integer_ranges.append(facts::IntegerRange {
                    minimum: numerics::bignum::BigInt::from_u64(minimum),
                    maximum: numerics::bignum::BigInt::from_u64(maximum),
                });
                let fact = semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point: ProgramPoint::default(),
                    origin: FactOrigin::StatementTransfer,
                    evidence: Default::default(),
                    payload: FactPayload::AssignedIntegerBounds {
                        bounds: if corrupt { Default::default() } else { bounds },
                    },
                });
                semantic.append_ref(&mut references, fact);
            }
            let context = semantic.append_context(ProgramPoint::default(), references);
            let range = integer_bounds_at_place(
                &program,
                &semantic,
                [semantic.contexts.get(context)],
                &subject,
            );
            assert_eq!(
                range.map(|range| (range.minimum.to_u64(), range.maximum.to_u64())),
                (!corrupt).then_some((Some(65), Some(80)))
            );
        }
    }

    #[test]
    fn call_provenance_requires_its_own_snapshot_and_conflicts_still_reject() {
        let mut program = TypedTrees::default();
        let call = program.expression_table.insert(ExpressionNode::Call(
            typed_trees::expression::TableCallExpression {
                receiver: Default::default(),
                target_symbol: Default::default(),
                target: Default::default(),
                static_machine_parameter: symbols::SymbolHandle::invalid(),
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
