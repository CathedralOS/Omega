//! Assignment binding invalidation consumes the address captured before the RHS.
//! Exact scalar coordinates may be unavailable through a representation view
//! even though its physical enclosure is known. That enclosure bounds possible
//! writes; it never establishes a stored value, equality or untouched contents.
//! Descriptor-slot replacement keeps the slot's own address, not its old pointer.
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandFact};
use validation::CanonicalPlace;

pub(super) fn assignment(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    statement: &checked_trees::statement::StatementNode,
    control: &FlowControlFacts,
    target: Handle<FlowOperandFact>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    if !control.operands.is_valid(target) {
        return None;
    }
    let capture = control.operands.get(target);
    if !capture.referents.is_empty() {
        return control
            .operand_referents
            .span(capture.referents)?
            .iter()
            .map(|place| {
                Some(CanonicalPlace {
                    root: place.root,
                    segments: control
                        .operand_referent_segments
                        .span(place.segments)?
                        .to_vec(),
                })
            })
            .collect();
    }
    let mut owned_frames = None;
    let frames = super::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    if frames.assignment_replaces_reference_binding(machine, statement)? {
        // Unknown slot geometry cannot recover its old pointee as a target.
        return None;
    }
    validation::completed_storage::reference_backing_places(
        program,
        control,
        target,
        &[],
        &mut |_, _| None,
    )
    .map(|places| {
        places
            .into_iter()
            .map(|place| CanonicalPlace {
                root: place.root,
                segments: place.segments,
            })
            .collect()
    })
}
