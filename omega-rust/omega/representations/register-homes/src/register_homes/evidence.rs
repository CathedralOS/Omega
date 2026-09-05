//! Typed commitments to allocation prerequisites, not validity certificates.
//!
//! Keeping these byte identities below the analyses lets a home artifact name
//! its exact inputs without depending on the pipeline that computed them.

use crate::RegisterHomeIdentity;
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelectionIdentity, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizationWorkUsage, OptimizedAbstractPlanProjectionIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

mod allocation;
mod analyses;
mod fixed_view;
mod identities;
mod policies;
mod selected_lowering;

pub use allocation::*;
pub use analyses::*;
pub use fixed_view::*;
pub use identities::*;
pub use policies::*;
pub use selected_lowering::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationLegalityIdentity(pub(crate) [u8; 32]);

impl AllocationLegalityIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiveRangeIdentity(pub(crate) [u8; 32]);

impl LiveRangeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocatorAvailabilityIdentity(pub(crate) [u8; 32]);

impl AllocatorAvailabilityIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
