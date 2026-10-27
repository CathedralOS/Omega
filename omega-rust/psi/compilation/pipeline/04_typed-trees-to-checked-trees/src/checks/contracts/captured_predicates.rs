//! Preserve predicates of completed copies across writes and selections.
//! Bindings keep each possible value and all of its aliases together. Producer
//! contexts never mix: selection coverage decides which transported conclusions
//! hold for the whole observed value.

use crate::facts::boolean_predicates::{BoundObservation, BoundPredicate};
use checked_trees::{CheckFacts, FlowOperandFact, FlowStateFact};
use validation::CanonicalPlace;

mod bindings;
mod cleanup;
mod coverage;
mod transport;
pub(super) use bindings::{CapturedBinding, retain_bindings};
pub(crate) use cleanup::cleanup_predicates;

#[derive(Default)]
pub(crate) struct PredicateTransport {
    pub predicates: Vec<BoundPredicate>,
    pub proves_required: bool,
}

pub(super) fn transport(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    required: &mut BoundPredicate,
    captures: &[CapturedBinding<'_>],
    premises: &[BoundPredicate],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> PredicateTransport {
    let mut conditions = crate::flow::SelectionConditions::new();
    let Some(selections) = captures
        .iter()
        .map(|capture| conditions.choices(&facts.flow.control, &capture.choices))
        .collect::<Option<Vec<_>>>()
    else {
        return PredicateTransport::default();
    };
    coverage::join_identities(
        program,
        facts,
        required,
        captures,
        &selections,
        &mut conditions,
    );
    let evidence = transport::predicates(
        program,
        facts,
        state,
        required,
        captures,
        &selections,
        &mut conditions,
        frames,
    );
    coverage::qualify(
        program,
        facts,
        required,
        premises,
        evidence,
        &mut conditions,
    )
}
