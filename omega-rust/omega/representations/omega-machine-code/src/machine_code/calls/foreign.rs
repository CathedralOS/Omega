//! Normalized foreign calls and their target floating-control preservation.

use crate::{
    CallbackAddressMaterialization, ForeignCallScalarArgumentRecord, ForeignCallScalarResultRecord,
    ProviderExecutionRecord, UnitCallStackEvidence, X86ForeignCallFloatingControlRecord,
};
use omega_target_operations::CallSiteOwner;

/// Exact source-free custody for one call to a normalized foreign locator.
///
/// `offset` names the mutable relocation field: the four-byte displacement
/// following x86-64 `CALL rel32`, or the complete AArch64 `BL` instruction.
/// Raw object/symbol/version bytes remain private inside the normalized locator
/// and are never reconstructed from an Omega or object-local symbol name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallRelocation {
    pub owner: CallSiteOwner,
    /// Ordinal of the exact attached-Unit operation that owns this call.
    pub operation_ordinal: usize,
    pub offset: usize,
    pub locator: omega_target::NormalizedForeignLocator,
    pub provider_execution: ProviderExecutionRecord,
    /// Exact admitted boundary contract consumed before assignment.
    pub boundary_entry_plan: omega_calling_conventions::BoundaryEntryPlan,
    /// Exact source-selected ABI plan consumed to emit this call.
    pub call_plan: omega_calling_conventions::CallPlan,
    /// Canonically ordered evaluated scalar arguments materialized before the
    /// unresolved native procedure-call placeholder. Each source is either an
    /// exact authored constant or an exact preceding scalar-result home. The
    /// bounded native carrier admits the target's complete register-resident
    /// fixed-integer argument bank.
    pub scalar_arguments: Vec<ForeignCallScalarArgumentRecord>,
    /// Exact compiler-private function-address materialization occupying one
    /// native-only callback parameter of this registrar call.
    pub callback_address: Option<CallbackAddressMaterialization>,
    /// Optional fixed-integer result normalized from its evaluated ABI
    /// placement and stored in a durable attached-Unit scalar home.
    pub scalar_result: Option<ForeignCallScalarResultRecord>,
    /// Complete MXCSR preservation around this returning x86 foreign call.
    pub x86_floating_control: Option<X86ForeignCallFloatingControlRecord>,
    /// Complete FPCR preservation around this returning AArch64 foreign call.
    pub aarch64_floating_control: Option<Aarch64ForeignCallFloatingControlRecord>,
    /// Byte-addressed outbound stack custody plus the independently admitted
    /// opaque same-stack contribution for the foreign leaf.
    pub unit_stack: UnitCallStackEvidence,
    pub same_stack_contribution: omega_task_plans::AdmittedSameStackContribution,
}

/// Per-call proof that one returning AArch64 foreign boundary preserved the
/// caller's complete FPCR. The slot may be reused by sequential calls, while
/// each call retains distinct save/restore instruction intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64ForeignCallFloatingControlRecord {
    pub target: omega_target::NativeTarget,
    pub saved_slot_byte_offset: u32,
    pub save_offset: usize,
    pub save_byte_count: usize,
    pub restore_offset: usize,
    pub restore_byte_count: usize,
}
