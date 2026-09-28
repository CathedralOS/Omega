//! Executed exit-boot-services invocations, outcomes and admission.

use crate::identities::Fnv1a;
use crate::platform_bringup::uefi_bootstrap::UefiExitBootServicesProviderResult;
use crate::platform_bringup::uefi_bootstrap::exit_boot_services::invocation_planning::UefiExitBootServicesFunction;
use crate::platform_bringup::uefi_bootstrap::exit_boot_services::{
    BoundUefiExitBootServicesInvocation, PlannedUefiExitBootServicesInvocation,
};
use crate::{
    ExternalRootDiagnostic, UefiExitBootServicesReceiptId, UefiImageHandleOccurrenceId,
    UefiMemoryMapKeyId, UefiPhysicalInvocationId,
};
use program_entry_plan::UefiOsHandoffStatusRole;
use std::ffi::c_void;

/// Closed interpretation of the exact returned `EFI_STATUS`, derived from the
/// custody role the retained `ExitBootServices` leg assigns the code. Codes
/// outside the leg's table remain observable as unsupported execution
/// results and retain their custody for release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UefiExitBootServicesAttemptStatus {
    /// The leg's `TransferNonReturning` row: Boot Services are consumed.
    Success,
    /// The leg's `RetryStaleMapKey` row: the bound key went stale.
    StaleMapKey,
    Unknown,
}

/// Sealed evidence that the exact retained service was invoked once with its
/// retained operands. The raw status is private evidence behind the closed
/// classification.
#[must_use = "executed UEFI ExitBootServices custody must be admitted or released"]
pub struct ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    pub(crate) invocation: BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    status_code: u64,
}

impl std::fmt::Debug for ExecutedUefiExitBootServicesInvocation<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutedUefiExitBootServicesInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("map_key", &self.map_key())
            .field("status_code", &self.status_code())
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl ExecutedUefiExitBootServicesInvocation<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }
    pub const fn map_key(&self) -> UefiMemoryMapKeyId {
        self.invocation.map_key()
    }
    pub const fn status_code(&self) -> u64 {
        self.status_code
    }
    pub fn status(&self) -> UefiExitBootServicesAttemptStatus {
        match self
            .invocation
            .invocation
            .plan
            .status_role(self.status_code)
        {
            Some(UefiOsHandoffStatusRole::TransferNonReturning) => {
                UefiExitBootServicesAttemptStatus::Success
            }
            Some(UefiOsHandoffStatusRole::RetryStaleMapKey) => {
                UefiExitBootServicesAttemptStatus::StaleMapKey
            }
            // The exit leg's closed table carries only the two rows above; an
            // acquisition-side or rejecting role here is outside its contract.
            Some(_) | None => UefiExitBootServicesAttemptStatus::Unknown,
        }
    }
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices execution rejection retains bound provider custody"]
pub struct UefiExitBootServicesExecutionError<'system_table, 'boot_services> {
    invocation: BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services>
    UefiExitBootServicesExecutionError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.diagnostic)
    }
}

impl std::fmt::Display for UefiExitBootServicesExecutionError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiExitBootServicesExecutionError<'_, '_> {}

/// Invoke the exact retained UEFI `ExitBootServices` service once.
///
/// # Safety
///
/// The target-runtime caller must establish that the retained numeric service
/// and image-handle values came from the live UEFI physical invocation and
/// remain callable for this operation, that the map key is the exact key the
/// bound acquired carrier names, and that a successful return permanently
/// ends Boot Services custody for this invocation. This is the sole
/// host-language unsafe premise; the resulting receipt cannot be constructed
/// directly.
pub unsafe fn execute_uefi_exit_boot_services<'system_table, 'boot_services>(
    invocation: BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
) -> Result<
    ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    Box<UefiExitBootServicesExecutionError<'system_table, 'boot_services>>,
> {
    if !invocation.invocation.retains_exact_plan() {
        return Err(Box::new(UefiExitBootServicesExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI ExitBootServices execution operands drifted after binding".into(),
            ),
        }));
    }
    let Ok(service_address) = usize::try_from(invocation._service.get()) else {
        return Err(Box::new(UefiExitBootServicesExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI ExitBootServices service address does not fit the runtime pointer carrier"
                    .into(),
            ),
        }));
    };
    let Ok(handle_address) = usize::try_from(invocation._handle.get()) else {
        return Err(Box::new(UefiExitBootServicesExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI image handle does not fit the runtime pointer carrier".into(),
            ),
        }));
    };
    let map_key = invocation.map_key.normalized_identity() as usize;
    // SAFETY: the function-pointer validity and exact EFI ABI are the explicit
    // caller obligations above. The bound carrier owns the only route to these
    // private numeric operands.
    let service: UefiExitBootServicesFunction = unsafe { std::mem::transmute(service_address) };
    // SAFETY: upheld by the same target-runtime contract. The key operand is
    // the exact key retained inside the bound acquired carrier.
    let status_code = unsafe { service(handle_address as *mut c_void, map_key) };
    Ok(ExecutedUefiExitBootServicesInvocation {
        invocation,
        status_code,
    })
}

/// Exact outcome of one admitted `ExitBootServices` attempt. On the stale-key
/// path the complete provider-and-plan custody returns for the bounded loop's
/// next acquisition; on success the entire carrier chain was consumed inside
/// admission and only the provider result remains.
#[must_use = "UEFI ExitBootServices attempt outcome must feed the handoff ledger or release custody"]
pub enum UefiExitBootServicesAttemptOutcome<'system_table, 'boot_services> {
    /// The leg's `RetryStaleMapKey` row: the map/key pair is stale. Provider
    /// custody returns for a fresh acquire and bind; the result feeds the
    /// ledger's stale-key transition.
    Retry {
        invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        status_code: u64,
        result: UefiExitBootServicesProviderResult,
    },
    /// The leg's `TransferNonReturning` row: boot-scoped services were
    /// consumed. The result feeds the ledger's completing transition; no
    /// provider custody survives.
    Exited {
        status_code: u64,
        result: UefiExitBootServicesProviderResult,
    },
}

impl std::fmt::Debug for UefiExitBootServicesAttemptOutcome<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retry { status_code, .. } => formatter
                .debug_struct("UefiExitBootServicesAttemptOutcome::Retry")
                .field("status_code", status_code)
                .finish_non_exhaustive(),
            Self::Exited { status_code, .. } => formatter
                .debug_struct("UefiExitBootServicesAttemptOutcome::Exited")
                .field("status_code", status_code)
                .finish_non_exhaustive(),
        }
    }
}

impl UefiExitBootServicesAttemptOutcome<'_, '_> {
    pub const fn status_code(&self) -> u64 {
        match self {
            Self::Retry { status_code, .. } | Self::Exited { status_code, .. } => *status_code,
        }
    }
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices attempt rejection retains executed custody"]
pub struct UefiExitBootServicesAttemptError<'system_table, 'boot_services> {
    execution: ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services>
    UefiExitBootServicesAttemptError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub const fn status_code(&self) -> u64 {
        self.execution.status_code()
    }
    pub fn into_parts(
        self,
    ) -> (
        ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.execution, self.diagnostic)
    }
}

impl std::fmt::Display for UefiExitBootServicesAttemptError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiExitBootServicesAttemptError<'_, '_> {}

/// Consume one exact execution receipt and classify its sealed status. This
/// is the sole route minting `UefiExitBootServicesProviderResult`: stale keys
/// return the complete provider custody, success consumes it, and statuses
/// outside the closed target table reject with custody intact.
pub fn admit_uefi_exit_boot_services_execution<'system_table, 'boot_services>(
    execution: ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
) -> Result<
    UefiExitBootServicesAttemptOutcome<'system_table, 'boot_services>,
    Box<UefiExitBootServicesAttemptError<'system_table, 'boot_services>>,
> {
    let status_code = execution.status_code;
    match execution.status() {
        UefiExitBootServicesAttemptStatus::StaleMapKey => {
            Ok(UefiExitBootServicesAttemptOutcome::Retry {
                invocation: execution.invocation.invocation,
                status_code,
                result: UefiExitBootServicesProviderResult::stale_map_key(),
            })
        }
        UefiExitBootServicesAttemptStatus::Success => {
            let receipt = exit_receipt(&execution);
            // Boot-scoped services end here: the executed carrier — provider,
            // projection, physical arrival, and phase lease inside it — is
            // consumed so no post-exit provider use can be expressed.
            let ExecutedUefiExitBootServicesInvocation { .. } = execution;
            Ok(UefiExitBootServicesAttemptOutcome::Exited {
                status_code,
                result: UefiExitBootServicesProviderResult::succeeded(receipt),
            })
        }
        UefiExitBootServicesAttemptStatus::Unknown => {
            Err(Box::new(UefiExitBootServicesAttemptError {
                execution,
                diagnostic: ExternalRootDiagnostic(
                    "UEFI ExitBootServices returned a status outside its closed target table"
                        .into(),
                ),
            }))
        }
    }
}

/// The success receipt names the exact executed occurrence: its identity is
/// derived from the private operands and sealed status, never chosen by the
/// caller. `| 1` keeps the normalized identity nonzero.
fn exit_receipt(
    execution: &ExecutedUefiExitBootServicesInvocation<'_, '_>,
) -> UefiExitBootServicesReceiptId {
    let mut hash = Fnv1a::new();
    hash.string("omega.uefi-exit-boot-services-receipt.v1");
    hash.u64(execution.physical_invocation().normalized_identity());
    hash.u64(execution.image_handle_occurrence().normalized_identity());
    hash.u64(
        execution
            .invocation
            .invocation
            .provider
            .occurrence
            .normalized_identity(),
    );
    hash.u64(execution.invocation.map_key.normalized_identity());
    hash.u64(execution.invocation._service.get());
    hash.u64(execution.status_code);
    UefiExitBootServicesReceiptId::from_normalized_identity(hash.finish() | 1)
        .expect("receipt identity forced nonzero")
}
