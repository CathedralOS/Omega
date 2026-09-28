//! Lifecycle-scoped UEFI `ExitBootServices` provider edge.
//!
//! This module is the sole issuance boundary for
//! `UefiExitBootServicesProviderResult`: external code cannot mint a provider
//! outcome, so the handoff ledger in `os_handoff.rs` can be driven only through
//! an executed attempt that sealed real custody. The join retains the live
//! Boot Services projection, replays the exact target-owned service-table row,
//! and seals the private `ExitBootServices` pointer. Binding joins the
//! retained physical image handle to the exact acquired map under the
//! Microsoft-x64 calling plan. One unsafe edge invokes that pointer once and
//! seals the status; admission classifies it.
//!
//! Custody rules implement the bounded retry contract, keyed by the custody
//! role the planned leg assigns each firmware status:
//!
//! - `RetryStaleMapKey` is the stale-key outcome. It returns the complete
//!   provider-and-plan custody so the loop can acquire a fresh map and bind a
//!   new attempt. Nothing is consumed except the spent bound operands.
//! - `TransferNonReturning` consumes the entire carrier chain — provider,
//!   projection, physical arrival, and phase lease — because every post-exit
//!   Boot Services call is invalid. Retaining the provider would model a use
//!   that can never be legitimate, so admission drops it and yields only the
//!   result.
//! - Any other status is outside the closed table and returns the executed
//!   custody intact for release.
//!
//! The target facts this edge drives — the `ExitBootServices` service-table
//! row, the two-operand Microsoft-x64 call shape, and the closed status table
//! with its custody roles — come from the `program-entry-plan`
//! `ExitBootServices` leg of `plan_uefi_os_handoff_invocation`. The join
//! checks the sealed row against that leg, preparation retains the leg on the
//! planned invocation, and every later custody transition replays the
//! retained leg against a freshly derived plan and the provider's row, so a
//! drifted or foreign plan rejects without a second authority.
//!
//! `provider_lifecycle.rs` carries the lifecycle-scoped provider, its join
//! and its release error, `invocation_planning.rs` planned and bound
//! invocations, `execution.rs` executed invocations, outcomes and
//! admission and `tests.rs` the service tests.

mod execution;
mod invocation_planning;
mod provider_lifecycle;
#[cfg(test)]
mod tests;

pub use execution::{
    ExecutedUefiExitBootServicesInvocation, UefiExitBootServicesAttemptError,
    UefiExitBootServicesAttemptOutcome, UefiExitBootServicesAttemptStatus,
    UefiExitBootServicesExecutionError, admit_uefi_exit_boot_services_execution,
};
pub use invocation_planning::{
    BoundUefiExitBootServicesInvocation, PlannedUefiExitBootServicesInvocation,
    UefiExitBootServicesInvocationBindingError, UefiExitBootServicesInvocationPlanningError,
    bind_uefi_exit_boot_services_invocation, prepare_uefi_exit_boot_services_invocation,
};
pub use provider_lifecycle::{
    LifecycleScopedUefiExitBootServicesProvider, UefiExitBootServicesProviderJoinError,
    UefiExitBootServicesProviderReleaseError,
    join_lifecycle_scoped_uefi_exit_boot_services_provider,
};

pub use execution::execute_uefi_exit_boot_services;
