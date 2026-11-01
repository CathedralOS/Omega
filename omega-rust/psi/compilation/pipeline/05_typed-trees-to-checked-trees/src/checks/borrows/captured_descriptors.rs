//! Join the invocation's captured operands to their physical backing.
//! The shared completed-storage query owns descriptor/enclosure geometry;
//! this borrow-checking adapter alone supplies checked completion-time loans.
use arena::Handle;
use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};
use facts::PlaceSegment;

mod loan_origins;

pub(super) fn places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
) -> Option<Vec<CapturedPlace>> {
    validation::completed_storage::reference_backing_places(
        program,
        &facts.flow.control,
        operand,
        path,
        &mut |operand, path| loan_origins::places(program, facts, state, operand, path),
    )
}
