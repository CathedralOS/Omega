//! Adapt exact value origins to the field-only progress subject surface.

use validation::CanonicalPlace;

use super::{FlowCallFact, FlowFacts, FlowStateFact, ProgressSubject};
use crate::flow::{self};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::TableCallExpression;
use typed_trees::machine::Machine;

mod call_places;
mod callee_values;
mod leaf_arrivals;
mod reference_bounds;
use call_places::*;
pub(super) use call_places::{call_argument_boundary_place, call_argument_place};
use reference_bounds::*;

pub(super) fn at_call(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    subject: ProgressSubject,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<ProgressSubject>> {
    let mut place = CanonicalPlace {
        root: PlaceRoot::Symbol(subject.root),
        segments: Vec::new(),
    };
    for projection in subject.projections {
        validation::push_field_place_segments(program, &mut place.segments, projection);
    }
    // The shared backward trace keeps value-position call results opaque;
    // progress supplies the two producers it can prove exactly: an owned
    // helper result substitutes the caller argument place the callee's
    // returned expression establishes, and a demanded place that still names
    // storage through a reference leaf inside an owned local rebases to the
    // referent the leaf's latest store supplied at each frontier.
    let mut owned_frames = None;
    let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let resolve = |state: &FlowStateFact,
                   statement_index: usize,
                   call: &TableCallExpression,
                   relative: &[PlaceSegment]| {
        call_result_value_place(program, frames, state, statement_index, call, relative, 16)
    };
    // A leaf demanded directly as the call's operand through an exclusive
    // binding — `r.view.scheduler` where `r: &mut RefBox` — cannot run the
    // operand-prefix check on its literal spelling: the shared prefix walk
    // keeps no origin for an exclusive borrow of a carrier that itself stores
    // shared leaves. Resolve the referent from the binding's own provenance
    // first so the ordinary reference-storage check sees the spelling it can
    // name.
    let place = exclusive_reference_root_place(
        program,
        frames,
        machine,
        state,
        call.statement_index,
        &place,
        &resolve,
    )
    .unwrap_or(place);
    if let Some(subjects) = disagreeing_call_result_subjects(program, frames, state, call, &place) {
        return Some(subjects);
    }
    let place = flow::value_origin_at_call_resolving(
        program,
        flow,
        machine,
        state,
        call,
        place,
        Some(frames),
        flow::ValueObservation::Contents,
        resolve,
        |state, bound, place| {
            reference_boundary_before_statement(
                program, frames, machine, state, bound, place, &resolve,
            )
        },
    )?;
    crate::checks::termination::progress::fact_subjects::subject_from_place(
        place.root,
        &place.segments,
    )
    .map(|subject| vec![subject])
}
