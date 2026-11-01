//! Scalar observations preserve their result while ending a fresh receiver.
//! The checked action roster supplies authority for descriptor retirement;
//! this normal continuation does not infer disposal from source syntax or own
//! the referenced storage. The observing owner separately authenticates whether
//! the receiver is fresh: an established outer operand keeps its consumer's
//! lifetime and never enters this completion.

use super::{Expansion, QualifiedScalarType};

impl Expansion<'_> {
    pub(super) fn complete_observation_receiver(
        &mut self,
        actions: arena::HandleSpan<checked_trees::CheckedNormalCleanupAction>,
        input_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> usize {
        if actions.is_empty() {
            return target;
        }
        let mut completed_types = input_types.to_vec();
        completed_types.push(result_type);
        self.push(super::LoweredScalarBranchState::jumping(
            target,
            Vec::new(),
            completed_types.clone(),
            &completed_types,
            vec![super::LoweredScalarEffect::CompleteObservationReceiver { actions }],
        ))
    }
}
