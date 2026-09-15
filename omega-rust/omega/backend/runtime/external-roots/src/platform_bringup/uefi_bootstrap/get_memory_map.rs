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
//! Custody rules implement the UEFI grow-and-retry idiom:
//!
//! - `EFI_SUCCESS` seals the occupied map extent, descriptor geometry, and
//!   physical key into `UefiMemoryMapAcquisition`, then returns the buffer
//!   holding its live map together with the provider custody for a later
//!   attempt.
//! - `EFI_BUFFER_TOO_SMALL` returns provider and buffer custody with the
//!   firmware-reported required size; the caller grows the buffer and
//!   re-binds without spending a handoff attempt.
//! - `EFI_INVALID_PARAMETER` and statuses outside the closed target table
//!   reject with executed custody retained for release.
//!
//! The exact five-operand call shape (RCX/RDX/R8/R9 plus the stack-resident
//! descriptor-version slot, RAX status, 32-byte shadow) is re-evaluated and
//! replayed at every custody transition rather than retained in a durable
//! `program-entry-plan` artifact, mirroring the exit edge.
//!
//! This file owns the service constants and the exact call shape.
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

use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl, MachineRegister,
    ValidatedBoundaryEntryPlan, ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};

use super::PlannedUefiExitBootServicesInvocation;
use super::exit_boot_services::{evaluate_exit_boot_services_plan, validate_exact_call_shape};
use crate::ExternalRootDiagnostic;

const GET_MEMORY_MAP_FIELD_ORDINAL: u8 = 9;

const GET_MEMORY_MAP_FIELD_OFFSET: u32 = 56;

const GET_MEMORY_MAP_FIELD_SIZE: u32 = 8;

const GET_MEMORY_MAP_FIELD_ALIGNMENT: u32 = 8;

const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;

const EFI_SUCCESS: u64 = 0;

const EFI_INVALID_PARAMETER: u64 = EFI_STATUS_ERROR_BIT | 2;

const EFI_BUFFER_TOO_SMALL: u64 = EFI_STATUS_ERROR_BIT | 5;

/// Smallest legal `DescriptorSize`: the fixed `EFI_MEMORY_DESCRIPTOR` prefix
/// (Type, Pad, PhysicalStart, VirtualStart, NumberOfPages, Attribute). Newer
/// descriptor versions may extend it; they may not shrink it.
const MIN_MEMORY_DESCRIPTOR_BYTES: usize = 40;

const SERVICE_IDENTITY: &str = "EFI_BOOT_SERVICES.GetMemoryMap";

fn get_memory_map_signature() -> CallSignature {
    // EFI_STATUS GetMemoryMap(UINTN *MemoryMapSize, EFI_MEMORY_DESCRIPTOR
    // *MemoryMap, UINTN *MapKey, UINTN *DescriptorSize, UINT32
    // *DescriptorVersion): five pointer-sized operand addresses in, one
    // pointer-sized status out. Under Microsoft x64 the fifth operand lands
    // in the first stack slot above the 32-byte shadow area.
    let word = ValueShape::integer(8, 8);
    CallSignature {
        parameters: vec![word; 5],
        result: Some(word),
    }
}

fn evaluate_get_memory_map_plan() -> Result<ValidatedBoundaryEntryPlan, ExternalRootDiagnostic> {
    evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &get_memory_map_signature())
        .map_err(|error| {
            ExternalRootDiagnostic(format!("UEFI GetMemoryMap calling plan rejected: {error}"))
        })
}

fn matches_exact_uefi_x64_call_plan(plan: &ValidatedBoundaryEntryPlan) -> bool {
    evaluate_get_memory_map_plan().is_ok_and(|expected| expected.plan() == plan.plan())
}

fn validate_exact_get_memory_map_call_shape(
    plan: &BoundaryEntryPlan,
) -> Result<(), ExternalRootDiagnostic> {
    let call = &plan.call;
    if !(call.policy == CallingPolicy::MicrosoftX64
        && call.parameters.len() == 5
        && call.shadow_bytes == 32
        && call.stack_alignment == 16
        && call.entry_control == EntryControl::CallReturn)
    {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap call frame drifted".into(),
        ));
    }
    let expected_registers = [
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ];
    for (placement, expected_register) in call.parameters.iter().zip(expected_registers) {
        if !matches!(placement.locations.as_slice(), [ValueLocation::Register { register, value_byte_offset: 0, byte_size: 8 }] if *register == expected_register)
        {
            return Err(ExternalRootDiagnostic(
                "UEFI GetMemoryMap input register placement drifted".into(),
            ));
        }
    }
    if !matches!(
        call.parameters[4].locations.as_slice(),
        [ValueLocation::Stack {
            stack_byte_offset: 32,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }]
    ) {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap descriptor-version operand missed its stack slot".into(),
        ));
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
            "UEFI GetMemoryMap result register placement drifted".into(),
        ));
    }
    Ok(())
}

/// Replay that the borrowed exit invocation still retains the exact
/// target-authored two-operand plan; acquisition only exists to feed it.
fn pending_exit_plan_is_exact(invocation: &PlannedUefiExitBootServicesInvocation<'_, '_>) -> bool {
    evaluate_exit_boot_services_plan().is_ok_and(|expected| expected.plan() == invocation.plan())
        && validate_exact_call_shape(invocation.plan()).is_ok()
}
