//! Read-only target recipe for native logical-fuel instrumentation.
//!
//! This is plan data, not admission authority. Orchestration validates and
//! seals a recipe before a backend may consume this projection.

use omega_calling_conventions::MachineRegister;
use omega_target::{NativeTarget, TargetProfile};

/// Target-selected route to the sponsor-owned per-activation fuel context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SponsorContextTransport {
    ReservedNonvolatileRegister { register: MachineRegister },
}

/// Exact byte layout of the private sponsor context consumed by charge and
/// cold-transfer stubs. Scalar offsets name one aligned native `u64`; the
/// activation-state interval is an opaque target-owned save area.
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
    pub transfer_plan_identity: u64,
}
