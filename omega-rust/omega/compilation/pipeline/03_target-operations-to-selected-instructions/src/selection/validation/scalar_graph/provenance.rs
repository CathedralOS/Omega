//! Independently preserve zero-payload operations before the next same-block
//! instruction. The source constructor still charges its original fuel once.
use super::{Replay, SelectedInstructionProvenance};

impl Replay<'_> {
    pub(super) fn settle_provenance(
        &mut self,
        provenance: SelectedInstructionProvenance,
    ) -> SelectedInstructionProvenance {
        if self.pending_provenance.operations.is_empty() {
            return provenance;
        }
        let mut pending = std::mem::take(&mut self.pending_provenance);
        pending.operations.extend(provenance.operations);
        pending.values.extend(provenance.values);
        pending.edges.extend(provenance.edges);
        pending.obligations.extend(provenance.obligations);
        pending.fuel.extend(provenance.fuel);
        pending
    }
}
