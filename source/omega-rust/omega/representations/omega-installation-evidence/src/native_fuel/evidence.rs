use omega_calling_conventions::StateFootprintEvidence;

use super::fingerprint::non_authoritative_transfer_evidence_report_fingerprint;
use super::plan::{NativeFuelRuntimeEntryIdentity, NativeFuelTransferRuntimePlanProjection};

/// Exact interval in the final image's compiler-owned text coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFuelRuntimeTextSpan {
    pub text_offset: usize,
    pub byte_count: usize,
}

/// Both sides of relocation for one transfer-runtime entry. Keeping the
/// unrelocated and final bytes distinct prevents a materialized image from
/// masquerading as independently replayed object evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFuelRuntimeTextEvidence {
    pub(super) entry: NativeFuelRuntimeEntryIdentity,
    pub(super) span: NativeFuelRuntimeTextSpan,
    pub(super) unrelocated_bytes: Vec<u8>,
    pub(super) final_bytes: Vec<u8>,
}

impl NativeFuelRuntimeTextEvidence {
    /// Construct read-only byte evidence. Success validates carrier shape; it
    /// does not prove that an encoder produced the bytes or grant authority to
    /// install or execute them.
    pub fn new(
        entry: NativeFuelRuntimeEntryIdentity,
        span: NativeFuelRuntimeTextSpan,
        unrelocated_bytes: Vec<u8>,
        final_bytes: Vec<u8>,
    ) -> Result<Self, NativeFuelTransferEvidenceError> {
        if span.byte_count == 0
            || unrelocated_bytes.len() != span.byte_count
            || final_bytes.len() != span.byte_count
            || span.text_offset.checked_add(span.byte_count).is_none()
        {
            return Err(NativeFuelTransferEvidenceError::InvalidTextSpan);
        }
        Ok(Self {
            entry,
            span,
            unrelocated_bytes,
            final_bytes,
        })
    }

    pub const fn entry(&self) -> NativeFuelRuntimeEntryIdentity {
        self.entry
    }

    pub const fn span(&self) -> NativeFuelRuntimeTextSpan {
        self.span
    }

    pub fn unrelocated_bytes(&self) -> &[u8] {
        &self.unrelocated_bytes
    }

    pub fn final_bytes(&self) -> &[u8] {
        &self.final_bytes
    }
}

/// Final dependency-light report for the two compiler-owned runtime entries
/// and their separately bounded physical resources. Construction checks
/// internal consistency only. Installation must independently replay these
/// facts and bind them to exact installed code and sponsor authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFuelTransferRuntimeEvidence {
    pub(super) plan: NativeFuelTransferRuntimePlanProjection,
    pub(super) transfer_text: NativeFuelRuntimeTextEvidence,
    pub(super) resume_text: NativeFuelRuntimeTextEvidence,
    pub(super) physical_state_footprint: StateFootprintEvidence,
    pub(super) sponsor_stack_peak_bytes: u64,
    pub(super) non_authoritative_report_fingerprint: u64,
}

impl NativeFuelTransferRuntimeEvidence {
    pub fn new(
        plan: NativeFuelTransferRuntimePlanProjection,
        transfer_text: NativeFuelRuntimeTextEvidence,
        resume_text: NativeFuelRuntimeTextEvidence,
        physical_state_footprint: StateFootprintEvidence,
        sponsor_stack_peak_bytes: u64,
    ) -> Result<Self, NativeFuelTransferEvidenceError> {
        if transfer_text.entry != plan.transfer_entry() {
            return Err(NativeFuelTransferEvidenceError::TransferEntryMismatch);
        }
        if resume_text.entry != plan.resume_entry() {
            return Err(NativeFuelTransferEvidenceError::ResumeEntryMismatch);
        }
        if spans_overlap(transfer_text.span, resume_text.span) {
            return Err(NativeFuelTransferEvidenceError::OverlappingTextSpans);
        }
        if physical_state_footprint
            .registers()
            .as_slice()
            .iter()
            .any(|register| register.architecture() != plan.target().architecture)
        {
            return Err(NativeFuelTransferEvidenceError::FootprintTargetMismatch);
        }
        if !physical_state_footprint
            .machine_state()
            .contains_all(plan.saved_state())
        {
            return Err(NativeFuelTransferEvidenceError::IncompleteStateFootprint);
        }
        if sponsor_stack_peak_bytes == 0
            || sponsor_stack_peak_bytes > plan.sponsor_stack().byte_ceiling
        {
            return Err(NativeFuelTransferEvidenceError::StackPeakExceedsPlan);
        }

        let mut evidence = Self {
            plan,
            transfer_text,
            resume_text,
            physical_state_footprint,
            sponsor_stack_peak_bytes,
            non_authoritative_report_fingerprint: 0,
        };
        evidence.non_authoritative_report_fingerprint =
            non_authoritative_transfer_evidence_report_fingerprint(&evidence);
        Ok(evidence)
    }

    pub const fn plan(&self) -> &NativeFuelTransferRuntimePlanProjection {
        &self.plan
    }

    pub const fn transfer_text(&self) -> &NativeFuelRuntimeTextEvidence {
        &self.transfer_text
    }

    pub const fn resume_text(&self) -> &NativeFuelRuntimeTextEvidence {
        &self.resume_text
    }

    pub const fn physical_state_footprint(&self) -> &StateFootprintEvidence {
        &self.physical_state_footprint
    }

    pub const fn sponsor_stack_peak_bytes(&self) -> u64 {
        self.sponsor_stack_peak_bytes
    }

    /// Explicitly non-authoritative compact report/cache coordinate. The exact
    /// plan, byte rows, footprint, and stack peak above remain the evidence.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    /// Compatibility accessor for [`Self::report_fingerprint`].
    pub const fn fingerprint(&self) -> u64 {
        self.report_fingerprint()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFuelTransferEvidenceError {
    InvalidTextSpan,
    TransferEntryMismatch,
    ResumeEntryMismatch,
    OverlappingTextSpans,
    FootprintTargetMismatch,
    IncompleteStateFootprint,
    StackPeakExceedsPlan,
}

impl std::fmt::Display for NativeFuelTransferEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeFuelTransferEvidenceError {}

fn spans_overlap(left: NativeFuelRuntimeTextSpan, right: NativeFuelRuntimeTextSpan) -> bool {
    let left_end = left.text_offset + left.byte_count;
    let right_end = right.text_offset + right.byte_count;
    left.text_offset < right_end && right.text_offset < left_end
}
