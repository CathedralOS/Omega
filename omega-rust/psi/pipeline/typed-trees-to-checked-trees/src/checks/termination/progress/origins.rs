//! Adapt exact value origins to the field-only progress subject surface.

use super::{FlowCallFact, FlowFacts, FlowStateFact, ProgressSubject};
use crate::flow::{self, CanonicalPlace};
use facts::PlaceRoot;
use typed_trees::{TypedTrees, machine::Machine};

#[cfg(test)]
mod tests;

pub(super) fn at_call(
    program: &TypedTrees,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    subject: ProgressSubject,
) -> Option<ProgressSubject> {
    let mut place = CanonicalPlace {
        root: PlaceRoot::Symbol(subject.root),
        segments: Vec::new(),
    };
    for projection in subject.projections {
        flow::push_field_place_segments(program, &mut place.segments, projection);
    }
    let place = flow::value_origin_at_call(program, flow, machine, state, call, place)?;
    super::subject_from_place(place.root, &place.segments)
}
