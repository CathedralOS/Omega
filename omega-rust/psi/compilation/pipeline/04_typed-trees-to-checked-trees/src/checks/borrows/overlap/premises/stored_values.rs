//! Completed assignment values supplement relational premises at their live use.
//!
//! Field constructions, scalar calls and value joins already publish scalar
//! observations through flow's shared value owner. Borrow compatibility must
//! consume those observations too; replaying their initializers would lose the
//! captured value and could reread mutated storage. Only immutable owned paths
//! have a timeless normalized bound here. Mutable locals and paths through any
//! reference retain occurrence-dependent storage, so current observations must
//! not become equations about a previously captured loan selector.

use facts::{FactPayload, ScalarValue};
use typed_trees::state::State;

use super::super::indexes::NormalizedBound;
use super::StatedOrderingPremise;

pub(super) fn append(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    premises: &mut Vec<StatedOrderingPremise>,
) {
    let mut visited = Vec::new();
    for fact in contexts.iter().flat_map(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
    }) {
        if !matches!(
            fact.payload,
            FactPayload::AssignedValue { .. }
                | FactPayload::AssignedScalarValue { .. }
                | FactPayload::AssignedIntegerBounds { .. }
        ) {
            continue;
        }
        let Some(place) = fact.place.as_place().and_then(|place| {
            crate::flow::canonical_place_from_semantic_place(
                program,
                &facts.semantic,
                facts.semantic.places.get(place),
            )
        }) else {
            continue;
        };
        if visited.contains(&place) {
            continue;
        }
        visited.push(place.clone());
        let Some(symbol) =
            crate::values::immutable_owned_place_root(program, state, statement, &place)
        else {
            continue;
        };
        let bound = if place.segments.is_empty() {
            NormalizedBound::Symbol { symbol, offset: 0 }
        } else {
            NormalizedBound::Projected {
                symbol,
                segments: place.segments.clone(),
            }
        };
        let Some(ScalarValue::Integer(value)) = crate::values::scalar_value_at_place(
            program,
            &facts.semantic,
            contexts
                .iter()
                .map(|context| facts.semantic.contexts.get(*context)),
            &place,
        ) else {
            continue;
        };
        let Some(value) = value.to_i64() else {
            continue;
        };
        premises.push(StatedOrderingPremise::captured_equality(
            bound,
            NormalizedBound::Integer(value),
        ));
    }
}
