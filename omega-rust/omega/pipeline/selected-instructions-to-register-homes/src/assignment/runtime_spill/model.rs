use crate::{
    StagedOptimizedAllocationLegality, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
    ValidatedRuntimeSpill,
};
use selected_instructions::VirtualRegisterId;

/// Original source custody and rewrite evidence are retained only for replay.
#[derive(Debug)]
pub(crate) struct RuntimeSpillAllocation {
    pub(crate) source: StagedOptimizedAllocationLegality,
    pub(crate) steps: Vec<RuntimeSpillStep>,
    pub(crate) facts: RuntimeSpillFacts,
    pub(crate) homes: ValidatedRegisterHomes,
    pub(crate) manifest: ValidatedPostAllocationOptimizationManifest,
}

#[derive(Debug)]
pub(crate) struct RuntimeSpillStep {
    pub(crate) function: usize,
    pub(crate) register: VirtualRegisterId,
    pub(crate) rewrite: ValidatedRuntimeSpill,
}

#[derive(Debug)]
pub(crate) struct RuntimeSpillFacts {
    pub(crate) liveness: ValidatedLiveness,
    pub(crate) ranges: ValidatedLiveRanges,
    pub(crate) legality: ValidatedAllocationLegality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSpillAllocationError {
    Upstream(crate::OptimizedAllocationLegalityCustodyError),
    Rewrite(crate::RuntimeSpillError),
    Liveness(crate::LivenessError),
    Ranges(crate::LiveRangeError),
    Legality(crate::AllocationLegalityError),
    Homes(crate::RegisterHomeError),
    Manifest(crate::PostAllocationOptimizationManifestError),
    RecoveryNotRequired,
    CandidateMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for RuntimeSpillAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "runtime spill allocation failed: {self:?}")
    }
}

impl std::error::Error for RuntimeSpillAllocationError {}
