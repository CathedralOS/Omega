//! Read-only target recipes for native logical-fuel instrumentation.
//!
//! These are plan and report projections, not admission authority. Omega
//! orchestration seals accepted values; target lowering and image replay use
//! the structural records here to agree on one exact physical realization.

use omega_calling_conventions::MachineRegister;
use omega_target::{NativeTarget, TargetProfile};

mod evidence;
mod fingerprint;
mod plan;

pub use evidence::{
    NativeFuelRuntimeTextEvidence, NativeFuelRuntimeTextSpan, NativeFuelTransferEvidenceError,
    NativeFuelTransferRuntimeEvidence,
};
pub use plan::{
    NativeFuelActivationStateSlot, NativeFuelRuntimeEntryIdentity, NativeFuelSavedValue,
    NativeFuelSponsorStackPlan, NativeFuelTransferPlanError,
    NativeFuelTransferRuntimePlanProjection,
};

/// Target-selected route to the sponsor-owned per-activation fuel context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SponsorContextTransport {
    ReservedNonvolatileRegister { register: MachineRegister },
}

/// Exact byte layout of the private sponsor context consumed by charge and
/// cold-transfer stubs. Scalar offsets name one aligned native `u64`; the
/// activation-state interval is subdivided by the transfer-runtime plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFuelContextLayout {
    pub byte_size: u32,
    pub alignment: u32,
    pub remaining_units_offset: u32,
    pub unpaid_site_kind_offset: u32,
    pub unpaid_site_identity_offset: u32,
    pub required_units_offset: u32,
    pub transfer_entry_offset: u32,
    pub retry_code_offset_offset: u32,
    pub sponsor_stack_top_offset: u32,
    pub activation_state_offset: u32,
    pub activation_state_byte_count: u32,
}

/// Dependency-light projection retained below orchestration. Profile identity
/// is deliberately separate from `NativeTarget`: Windows and UEFI x86-64 have
/// the same architecture/object tuple but different deployment policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFuelTargetPlanProjection {
    pub profile: TargetProfile,
    pub target: NativeTarget,
    pub transport: SponsorContextTransport,
    pub context: NativeFuelContextLayout,
    /// Historical compact report coordinate for diagnostics and wire reports.
    pub transfer_plan_report_identity: u64,
    /// Strong commitment to the complete canonical transfer-runtime plan.
    pub transfer_plan_commitment: NativeFuelTransferPlanCommitment,
}

/// Domain-separated SHA-256 commitment to one complete canonical native-fuel
/// transfer-runtime plan. This is the target-policy authority; the adjacent
/// compact report identity is never sufficient for admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeFuelTransferPlanCommitment([u8; 32]);

impl NativeFuelTransferPlanCommitment {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == [0; 32]
    }
}

#[cfg(test)]
mod tests;
