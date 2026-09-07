//! Live attached-field facts at ordinary state edges. No declaration seeds them.

use super::*;
use crate::field_domain::ByteSequencePredicate;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FieldValue {
    segments: Vec<facts::PlaceSegment>,
    literal: ExpressionHandle,
    predicates: Vec<ByteSequencePredicate>,
}

pub(super) fn height(fields: &[FieldValue]) -> usize {
    fields.iter().map(|field| 1 + field.predicates.len()).sum()
}

/// Missing evidence is absorbing after a state has one reachable predecessor.
pub(super) fn meet(previous: &mut Vec<FieldValue>, incoming: &[FieldValue]) -> bool {
    let mut changed = false;
    previous.retain_mut(|field| {
        let Some(next) = incoming.iter().find(|next| next.segments == field.segments) else {
            changed = true;
            return false;
        };
        if field.literal != next.literal && field.literal.is_valid() {
            field.literal = ExpressionHandle::invalid();
            changed = true;
        }
        field.predicates.retain(|predicate| {
            let retained = next.predicates.contains(predicate);
            changed |= !retained;
            retained
        });
        field.literal.is_valid() || !field.predicates.is_empty()
    });
    changed
}

fn has_self(program: &typed_trees::TypedTrees, state: &typed_trees::state::State) -> bool {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self)
        .count()
        == 1
}

pub(super) fn capture(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    ctx: &FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &typed_trees::state::State,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<FieldValue> {
    if !has_self(program, state) || !has_self(program, destination) {
        return Vec::new();
    }
    let mut fields: Vec<FieldValue> = Vec::new();
    for reference in ctx.contexts.semantic_context_refs.span_or_empty(contexts) {
        for fact in semantic
            .context_view(semantic.contexts.get(reference.context))
            .facts()
        {
            let (literal, predicates) = match fact.payload {
                FactPayload::AssignedValue { value }
                    if program.expression_table.expression_is_valid(value) =>
                {
                    let ExpressionNode::String(bytes) = program.expression_table.expression(value)
                    else {
                        continue;
                    };
                    (
                        value,
                        ByteSequencePredicate::ALL
                            .into_iter()
                            .filter(|predicate| predicate.holds_for(bytes))
                            .collect::<Vec<_>>(),
                    )
                }
                FactPayload::BytePredicate { predicate: proved } => (
                    ExpressionHandle::invalid(),
                    ByteSequencePredicate::ALL
                        .into_iter()
                        .filter(|predicate| proved.implies(*predicate))
                        .collect(),
                ),
                _ => continue,
            };
            let FactPlace::Place(place) = fact.place else {
                continue;
            };
            let Some(mut place) =
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
            else {
                continue;
            };
            normalize_attached_place_root(program, machine.symbol, state.symbol, &mut place);
            place.root = normalized_event_place_root(program, place.root);
            if place.root != facts::PlaceRoot::Symbol(machine.symbol)
                || !matches!(place.segments.first(), Some(facts::PlaceSegment::Field { symbol }) if symbol.is_valid())
                || !place.segments.iter().all(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                    facts::PlaceSegment::Case { variant } => variant.is_valid(),
                    facts::PlaceSegment::FixedIndex { .. } => true,
                    _ => false,
                })
            {
                continue;
            }
            if let Some(field) = fields
                .iter_mut()
                .find(|field| field.segments == place.segments)
            {
                if literal.is_valid() {
                    field.literal = literal;
                }
                for predicate in predicates {
                    if !field.predicates.contains(&predicate) {
                        field.predicates.push(predicate);
                    }
                }
            } else {
                fields.push(FieldValue {
                    segments: place.segments,
                    literal,
                    predicates,
                });
            }
        }
    }
    fields
}

pub(super) fn append(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    fields: &[FieldValue],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    point: ProgramPoint,
) {
    if !has_self(program, state) {
        return;
    }
    for field in fields {
        let mut segments = HandleSpan::empty();
        for segment in &field.segments {
            semantic
                .place_segments
                .append_to_span(&mut segments, *segment);
        }
        let place = semantic.append_place(facts::Place {
            root: facts::PlaceRoot::Symbol(machine.symbol),
            segments,
        });
        let mut references = HandleSpan::empty();
        let payloads = field
            .literal
            .is_valid()
            .then_some(FactPayload::AssignedValue {
                value: field.literal,
            })
            .into_iter()
            .chain(
                field
                    .predicates
                    .iter()
                    .map(|predicate| FactPayload::BytePredicate {
                        predicate: *predicate,
                    }),
            );
        for payload in payloads {
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::default(),
                payload,
            });
            semantic.append_ref(&mut references, fact);
        }
        semantic.append_context(point, references);
    }
}
