use crate::{
    OptimizedActiveResidentRematerializationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedSelectedFormEncodingError, SelectedFormEncodingIdentity,
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding,
};

/// Owning pre-layout custody for one pressure-rematerialized selected form, its
/// source-specific post-allocation machine plan, and canonical scalar bytes.
/// Deferred control rows remain unresolved and this grants no layout, frame,
/// emission, section, object, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerializationSelectedFormEncoding {
    pub(super) source: StagedOptimizedActiveResidentRematerialization,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) custody:
        StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerializationSelectedFormEncoding {
    pub const fn source(&self) -> &StagedOptimizedActiveResidentRematerialization {
        &self.source
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
    pub(super) rematerialization: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) transformed_selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) encoding: SelectedFormEncodingIdentity,
    pub(super) row_count: usize,
    pub(super) encoded_count: usize,
    pub(super) deferred_count: usize,
}

impl StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
    pub const fn rematerialization(
        &self,
    ) -> StagedOptimizedActiveResidentRematerializationCustodyReceipt {
        self.rematerialization
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }

    pub const fn transformed_selected(
        &self,
    ) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.transformed_selected
    }

    pub const fn encoding(&self) -> SelectedFormEncodingIdentity {
        self.encoding
    }

    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    pub const fn encoded_count(&self) -> usize {
        self.encoded_count
    }

    pub const fn deferred_count(&self) -> usize {
        self.deferred_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationSelectedFormEncodingError {
    Rematerialization(OptimizedActiveResidentRematerializationError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(OptimizedSelectedFormEncodingError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedActiveResidentRematerializationSelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized active-resident rematerialization selected-form encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedActiveResidentRematerializationSelectedFormEncodingError {}
