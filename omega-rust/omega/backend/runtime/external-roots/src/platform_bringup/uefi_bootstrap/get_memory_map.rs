//! Lifecycle-scoped UEFI `GetMemoryMap` acquisition edge.
//!
//! This module is the sole issuance boundary for `UefiMemoryMapAcquisition`:
//! the handoff ledger in `os_handoff.rs` requires that executed evidence
//! before it forms a `UefiOsHandoffMapAcquired`, so no caller-chosen
//! snapshot/key identity can reach `bind_uefi_exit_boot_services_invocation`.
//! The acquired key is the exact `UINTN` the firmware wrote, so the key and
//! descriptor-version identity bound into an exit attempt always name the
//! most recent map this edge physically acquired.
//!
//! Acquisition custody is deliberately scoped beneath a pending exact
//! `ExitBootServices` invocation. `ExitBootServices` accepts only the newest
//! map key, so this edge borrows the live planned invocation rather than
//! competing for the single Boot Services projection that provider retains.
//! Every custody level ends by returning that pending exit custody
//! (`into_pending_exit_invocation`); the projection, physical arrival, and
//! phase lease stay owned by the landed exit provider chain, and a completed
//! exit still consumes it entire.
//!
//! Custody rules implement the UEFI grow-and-retry idiom, keyed by the
//! custody role the planned leg assigns each firmware status:
//!
//! - `MapAcquired` seals the occupied map extent, descriptor geometry, and
//!   physical key into `UefiMemoryMapAcquisition`, then returns the buffer
//!   holding its live map together with the provider custody for a later
//!   attempt.
//! - `GrowMapBuffer` returns provider and buffer custody with the
//!   firmware-reported required size; the caller grows the buffer and
//!   re-binds without spending a handoff attempt.
//! - `Reject` rows and statuses outside the closed target table reject with
//!   executed custody retained for release.
//!
//! The target facts this edge drives — the `GetMemoryMap` service-table row,
//! the five-operand Microsoft-x64 call shape (RCX/RDX/R8/R9 plus the
//! stack-resident descriptor-version slot, RAX status, 32-byte shadow), and
//! the closed status table with its custody roles — come from the
//! `program-entry-plan` `GetMemoryMap` leg of
//! `plan_uefi_os_handoff_invocation`, mirroring the exit edge. The join
//! checks the sealed row against that leg, preparation retains the leg on
//! the planned invocation, and every later custody transition replays both
//! the retained leg and the borrowed pending exit's leg against a freshly
//! derived plan.
//!
//! This file owns the descriptor floor the acquisition edge admits.
//! `memory_map_buffer.rs` carries the memory map buffer,
//! `provider_lifecycle.rs` the lifecycle-scoped provider and its join,
//! `invocation_planning.rs` planned and bound invocations and
//! `execution.rs` executed invocations, acquisitions and admission;
//! `tests.rs` holds the service tests.

mod execution;
mod invocation_planning;
mod memory_map_buffer;
mod provider_lifecycle;
#[cfg(test)]
mod tests;

pub use execution::{
    ExecutedUefiGetMemoryMapInvocation, UefiGetMemoryMapAttemptError,
    UefiGetMemoryMapAttemptOutcome, UefiGetMemoryMapAttemptStatus, UefiGetMemoryMapExecutionError,
    UefiMemoryMapAcquisition, admit_uefi_get_memory_map_execution,
};
pub use invocation_planning::{
    BoundUefiGetMemoryMapInvocation, PlannedUefiGetMemoryMapInvocation,
    UefiGetMemoryMapInvocationBindingError, UefiGetMemoryMapInvocationPlanningError,
    bind_uefi_get_memory_map_invocation, prepare_uefi_get_memory_map_invocation,
};
pub use memory_map_buffer::UefiMemoryMapBuffer;
pub use provider_lifecycle::{
    LifecycleScopedUefiGetMemoryMapProvider, join_lifecycle_scoped_uefi_get_memory_map_provider,
};

pub use execution::execute_uefi_get_memory_map;

/// Smallest legal `DescriptorSize`: the fixed `EFI_MEMORY_DESCRIPTOR` prefix
/// (Type, Pad, PhysicalStart, VirtualStart, NumberOfPages, Attribute). Newer
/// descriptor versions may extend it; they may not shrink it.
const MIN_MEMORY_DESCRIPTOR_BYTES: usize = 40;
