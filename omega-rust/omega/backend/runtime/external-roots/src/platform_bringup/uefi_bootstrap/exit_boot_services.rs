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
//! Custody rules implement the bounded retry contract:
//!
//! - `EFI_INVALID_PARAMETER` is the stale-key outcome. It returns the complete
//!   provider-and-plan custody so the loop can acquire a fresh map and bind a
//!   new attempt. Nothing is consumed except the spent bound operands.
//! - `EFI_SUCCESS` consumes the entire carrier chain — provider, projection,
//!   physical arrival, and phase lease — because every post-exit Boot Services
//!   call is invalid. Retaining the provider would model a use that can never
//!   be legitimate, so admission drops it and yields only the result.
//! - Any other status is outside the closed table and returns the executed
//!   custody intact for release.
//!
//! The durable target-owned invocation-plan artifact (named identity,
//! commitment digests for report publication) remains a later
//! `program-entry-plan` boundary. This edge instead re-evaluates and replays
//! the exact two-operand call shape at every custody transition, so a drifted
//! or foreign plan still rejects without a second retained authority.
//!
//! This file owns the service constants and the exact call shape.
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

use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl, MachineRegister,
    ValidatedBoundaryEntryPlan, ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};

use crate::ExternalRootDiagnostic;

const EXIT_BOOT_SERVICES_FIELD_ORDINAL: u8 = 31;

const EXIT_BOOT_SERVICES_FIELD_OFFSET: u32 = 232;

const EXIT_BOOT_SERVICES_FIELD_SIZE: u32 = 8;

const EXIT_BOOT_SERVICES_FIELD_ALIGNMENT: u32 = 8;

const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;

const EFI_SUCCESS: u64 = 0;

const EFI_INVALID_PARAMETER: u64 = EFI_STATUS_ERROR_BIT | 2;

const SERVICE_IDENTITY: &str = "EFI_BOOT_SERVICES.ExitBootServices";

fn exit_boot_services_signature() -> CallSignature {
    // EFI_STATUS ExitBootServices(EFI_HANDLE ImageHandle, UINTN MapKey): two
    // pointer-sized operands in, one pointer-sized status out.
    let word = ValueShape::integer(8, 8);
    CallSignature {
        parameters: vec![word, word],
        result: Some(word),
    }
}

pub(super) fn evaluate_exit_boot_services_plan()
-> Result<ValidatedBoundaryEntryPlan, ExternalRootDiagnostic> {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &exit_boot_services_signature(),
    )
    .map_err(|error| {
        ExternalRootDiagnostic(format!(
            "UEFI ExitBootServices calling plan rejected: {error}"
        ))
    })
}

fn matches_exact_uefi_x64_call_plan(plan: &ValidatedBoundaryEntryPlan) -> bool {
    evaluate_exit_boot_services_plan().is_ok_and(|expected| expected.plan() == plan.plan())
}

pub(super) fn validate_exact_call_shape(
    plan: &BoundaryEntryPlan,
) -> Result<(), ExternalRootDiagnostic> {
    let call = &plan.call;
    if !(call.policy == CallingPolicy::MicrosoftX64
        && call.parameters.len() == 2
        && call.shadow_bytes == 32
        && call.stack_alignment == 16
        && call.entry_control == EntryControl::CallReturn)
    {
        return Err(ExternalRootDiagnostic(
            "UEFI ExitBootServices call frame drifted".into(),
        ));
    }
    let expected_registers = [MachineRegister::X86Rcx, MachineRegister::X86Rdx];
    for (placement, expected_register) in call.parameters.iter().zip(expected_registers) {
        if !matches!(placement.locations.as_slice(), [ValueLocation::Register { register, value_byte_offset: 0, byte_size: 8 }] if *register == expected_register)
        {
            return Err(ExternalRootDiagnostic(
                "UEFI ExitBootServices input register placement drifted".into(),
            ));
        }
    }
    if !matches!(
        call.result
            .as_ref()
            .map(|placement| placement.locations.as_slice()),
        Some([ValueLocation::Register {
            register: MachineRegister::X86Rax,
            value_byte_offset: 0,
            byte_size: 8
        }])
    ) {
        return Err(ExternalRootDiagnostic(
            "UEFI ExitBootServices result register placement drifted".into(),
        ));
    }
    Ok(())
}
