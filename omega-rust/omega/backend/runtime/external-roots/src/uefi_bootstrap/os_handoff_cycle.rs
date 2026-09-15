//! Caller-side composition of the bounded UEFI OS-handoff cycle.
//!
//! `os_handoff.rs` owns the linear handoff ledger; `get_memory_map.rs` and
//! `exit_boot_services.rs` own the two provider edges. This module is the
//! single ordered composition the target runtime drives. For each handoff
//! arrival it runs the `GetMemoryMap` join/prepare/bind/execute/admit edge —
//! growing the map buffer through `EFI_BUFFER_TOO_SMALL` without spending a
//! handoff attempt — then registers the sealed acquisition with the ledger,
//! binds the pending `ExitBootServices` invocation to that exact key, executes
//! and admits the attempt, and applies the provider result. The map key
//! reaching the exit binding therefore always names the most recent firmware
//! map this physical invocation acquired.
//!
//! No other order is expressible through the composition: the caller supplies
//! only the loop-invariant custody — the live firmware ledger borrow, the
//! handoff ledger, its current arrival, the pending exit invocation, and the
//! map buffer — and receives either a terminal resolution or the complete
//! custody live at the rejecting stage. Grow-and-retry inside acquisition and
//! stale-key retry across attempts both preserve that custody end to end.

use crate::ExternalRootDiagnostic;

use super::{
    BoundUefiExitBootServicesInvocation, ExecutedUefiExitBootServicesInvocation,
    PlannedUefiExitBootServicesInvocation, UefiApplicationFirmwareLedger,
    UefiExitBootServicesAttemptOutcome, UefiExitBootServicesProviderResult,
    UefiGetMemoryMapAttemptOutcome, UefiMemoryMapAcquisition, UefiMemoryMapBuffer,
    UefiOsHandoffComplete, UefiOsHandoffExhausted, UefiOsHandoffLedger, UefiOsHandoffMapAcquired,
    UefiOsHandoffMapRequired, UefiOsHandoffProgress, admit_uefi_exit_boot_services_execution,
    admit_uefi_get_memory_map_execution, bind_uefi_exit_boot_services_invocation,
    bind_uefi_get_memory_map_invocation, execute_uefi_exit_boot_services,
    execute_uefi_get_memory_map, join_lifecycle_scoped_uefi_get_memory_map_provider,
    prepare_uefi_get_memory_map_invocation,
};

/// Terminal resolution of one bounded handoff cycle.
#[must_use = "UEFI OS-handoff resolution retains live custody"]
pub enum UefiOsHandoffCycleResolution<'system_table, 'boot_services, 'buffer> {
    /// `ExitBootServices` returned `EFI_SUCCESS`: admission consumed the
    /// provider chain and the ledger sealed the completing transition. The
    /// buffer still holds the final map's occupied prefix for the OS-entry
    /// plan to walk; Boot Services custody exists nowhere.
    Complete {
        completion: UefiOsHandoffComplete,
        buffer: &'buffer mut UefiMemoryMapBuffer,
    },
    /// Every bounded attempt was spent on stale keys. Boot Services remain
    /// live: the exhaustion record, the unspent pending-exit custody, and the
    /// map buffer return for the invocation's firmware return path.
    Exhausted {
        exhaustion: UefiOsHandoffExhausted,
        pending_exit: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
    },
}

impl std::fmt::Debug for UefiOsHandoffCycleResolution<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Complete { completion, .. } => formatter
                .debug_struct("UefiOsHandoffCycleResolution::Complete")
                .field("handoff", &completion.handoff_id())
                .field("final_map", &completion.final_map())
                .field("receipt", &completion.receipt())
                .finish_non_exhaustive(),
            Self::Exhausted { exhaustion, .. } => formatter
                .debug_struct("UefiOsHandoffCycleResolution::Exhausted")
                .field("handoff", &exhaustion.handoff_id())
                .field("status", &exhaustion.status().value())
                .finish_non_exhaustive(),
        }
    }
}

/// Stage-exact rejection of one handoff cycle. Every variant returns the
/// complete custody live at the rejecting stage beside the edge's diagnostic,
/// so the caller can release provider custody, re-drive a corrected input, or
/// unwind the firmware return without losing any linear input.
#[must_use = "UEFI OS-handoff rejection returns all live custody"]
pub enum UefiOsHandoffCycleRejection<'system_table, 'boot_services, 'buffer> {
    /// The `GetMemoryMap` edge rejected during join, prepare, bind, execute,
    /// or admit: no acquisition evidence reached the ledger, the pending-exit
    /// invocation was only borrowed, and the arrival remains unspent.
    MapAcquisition {
        pending_exit: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        arrival: UefiOsHandoffMapRequired,
        diagnostic: ExternalRootDiagnostic,
    },
    /// The handoff ledger refused an admitted acquisition — a foreign, stale,
    /// or custody-lost arrival, cross-invocation evidence, or a retired
    /// snapshot/key pair. The unspent acquisition returns beside the arrival.
    MapAdmission {
        pending_exit: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        arrival: UefiOsHandoffMapRequired,
        acquisition: UefiMemoryMapAcquisition,
        diagnostic: ExternalRootDiagnostic,
    },
    /// The exit operand edge rejected before execution; the pending exit
    /// returns unbound beside the still-acquired attempt.
    ExitBinding {
        pending_exit: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        acquired: UefiOsHandoffMapAcquired,
        diagnostic: ExternalRootDiagnostic,
    },
    /// The bound exit invocation could not execute; the still-bound operands
    /// return beside the acquired attempt.
    ExitExecution {
        bound: BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        acquired: UefiOsHandoffMapAcquired,
        diagnostic: ExternalRootDiagnostic,
    },
    /// The executed exit returned a status outside its closed target table;
    /// the executed custody returns for the ledger-owned release route.
    ExitAdmission {
        execution: ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        acquired: UefiOsHandoffMapAcquired,
        diagnostic: ExternalRootDiagnostic,
    },
    /// The ledger rejected the provider result admitted from a stale-key
    /// exit; the pending-exit custody that admission returned comes back
    /// beside the acquired attempt and the unspent result.
    StaleTransition {
        pending_exit: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        acquired: UefiOsHandoffMapAcquired,
        result: UefiExitBootServicesProviderResult,
        diagnostic: ExternalRootDiagnostic,
    },
    /// The ledger rejected the provider result admitted from a successful
    /// exit. The provider chain was consumed inside admission, so only the
    /// acquired attempt and the unspent result return.
    ExitTransition {
        buffer: &'buffer mut UefiMemoryMapBuffer,
        acquired: UefiOsHandoffMapAcquired,
        result: UefiExitBootServicesProviderResult,
        diagnostic: ExternalRootDiagnostic,
    },
}

impl UefiOsHandoffCycleRejection<'_, '_, '_> {
    /// The rejecting stage's own diagnostic; custody identity stays inside the
    /// variant fields.
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        match self {
            Self::MapAcquisition { diagnostic, .. }
            | Self::MapAdmission { diagnostic, .. }
            | Self::ExitBinding { diagnostic, .. }
            | Self::ExitExecution { diagnostic, .. }
            | Self::ExitAdmission { diagnostic, .. }
            | Self::StaleTransition { diagnostic, .. }
            | Self::ExitTransition { diagnostic, .. } => diagnostic,
        }
    }
}

impl std::fmt::Debug for UefiOsHandoffCycleRejection<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stage = match self {
            Self::MapAcquisition { .. } => "MapAcquisition",
            Self::MapAdmission { .. } => "MapAdmission",
            Self::ExitBinding { .. } => "ExitBinding",
            Self::ExitExecution { .. } => "ExitExecution",
            Self::ExitAdmission { .. } => "ExitAdmission",
            Self::StaleTransition { .. } => "StaleTransition",
            Self::ExitTransition { .. } => "ExitTransition",
        };
        formatter
            .debug_struct("UefiOsHandoffCycleRejection")
            .field("stage", &stage)
            .field("diagnostic", self.diagnostic())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for UefiOsHandoffCycleRejection<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic().fmt(formatter)
    }
}

impl std::error::Error for UefiOsHandoffCycleRejection<'_, '_, '_> {}

/// Drive one bounded `GetMemoryMap`/`ExitBootServices` cycle to resolution.
///
/// For each ledger arrival this runs the only legal order: join the
/// lifecycle-scoped `GetMemoryMap` provider beneath `pending_exit`, plan,
/// bind `buffer`, execute once, and admit — growing `buffer` to each
/// firmware-reported requirement until the edge seals acquisition evidence —
/// then register that evidence with `handoff`, bind the pending exit
/// invocation to its exact key, execute and admit the attempt, and apply the
/// provider result. A stale key returns the pending-exit custody for the next
/// acquisition; the last stale key resolves to the target-authored exhaustion
/// status; success resolves to the non-returning completion.
///
/// `arrival` must be the handoff ledger's current map-required arrival: the
/// first one `UefiOsHandoffLedger::new` minted, or the fresh arrival inside a
/// `UefiOsHandoffProgress::Retry`. A foreign or stale arrival still performs
/// the physical acquisition — a read with no firmware side effect — before
/// the ledger rejects it with the unspent evidence returned.
///
/// # Safety
///
/// The caller must establish, for every service call this cycle performs, the
/// premises of `execute_uefi_get_memory_map` and
/// `execute_uefi_exit_boot_services`: the service addresses retained inside
/// `pending_exit` came from the live UEFI physical invocation and remain
/// callable for these operations, `buffer` is exclusively owned and writable
/// for the duration of each `GetMemoryMap` call, and a successful
/// `ExitBootServices` return permanently ends Boot Services custody for this
/// invocation. This remains the sole host-language unsafe premise; the
/// receipt, acquisition, and provider-result identities cannot be constructed
/// outside their edges.
pub unsafe fn drive_uefi_os_handoff_cycle<'system_table, 'boot_services, 'buffer>(
    firmware: &UefiApplicationFirmwareLedger<'system_table>,
    handoff: &mut UefiOsHandoffLedger,
    mut arrival: UefiOsHandoffMapRequired,
    mut pending_exit: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    mut buffer: &'buffer mut UefiMemoryMapBuffer,
) -> Result<
    UefiOsHandoffCycleResolution<'system_table, 'boot_services, 'buffer>,
    Box<UefiOsHandoffCycleRejection<'system_table, 'boot_services, 'buffer>>,
> {
    loop {
        // Acquisition leg: join/prepare/bind/execute/admit under the pending
        // exit's live custody, growing the buffer through BufferTooSmall
        // until the edge seals an acquisition. No custody leaves the caller's
        // ownership on this leg — the provider only borrows `pending_exit` —
        // so every rejection collapses back to the same three carriers.
        let acquisition = loop {
            let provider =
                match join_lifecycle_scoped_uefi_get_memory_map_provider(firmware, &pending_exit) {
                    Ok(provider) => provider,
                    Err(diagnostic) => {
                        return Err(Box::new(UefiOsHandoffCycleRejection::MapAcquisition {
                            pending_exit,
                            buffer,
                            arrival,
                            diagnostic,
                        }));
                    }
                };
            let invocation = match prepare_uefi_get_memory_map_invocation(provider) {
                Ok(invocation) => invocation,
                Err(error) => {
                    let (_provider, diagnostic) = error.into_parts();
                    return Err(Box::new(UefiOsHandoffCycleRejection::MapAcquisition {
                        pending_exit,
                        buffer,
                        arrival,
                        diagnostic,
                    }));
                }
            };
            let bound = match bind_uefi_get_memory_map_invocation(invocation, buffer) {
                Ok(bound) => bound,
                Err(error) => {
                    let (_invocation, returned_buffer, diagnostic) = error.into_parts();
                    return Err(Box::new(UefiOsHandoffCycleRejection::MapAcquisition {
                        pending_exit,
                        buffer: returned_buffer,
                        arrival,
                        diagnostic,
                    }));
                }
            };
            // SAFETY: the function's safety contract covers every GetMemoryMap
            // call this cycle performs; the bound carrier owns the exact
            // retained service address and exclusively held cells.
            let executed = match unsafe { execute_uefi_get_memory_map(bound) } {
                Ok(executed) => executed,
                Err(error) => {
                    let (bound, diagnostic) = error.into_parts();
                    let (_pending_exit, returned_buffer) = bound.into_pending_exit_invocation();
                    return Err(Box::new(UefiOsHandoffCycleRejection::MapAcquisition {
                        pending_exit,
                        buffer: returned_buffer,
                        arrival,
                        diagnostic,
                    }));
                }
            };
            match admit_uefi_get_memory_map_execution(executed) {
                Ok(UefiGetMemoryMapAttemptOutcome::Acquired {
                    buffer: returned,
                    acquisition,
                    ..
                }) => {
                    buffer = returned;
                    break acquisition;
                }
                Ok(UefiGetMemoryMapAttemptOutcome::BufferTooSmall {
                    buffer: returned,
                    required_map_bytes,
                    ..
                }) => {
                    // The edge only admits a requirement above the bound
                    // capacity, so each grow strictly advances the retry.
                    returned.grow(required_map_bytes);
                    buffer = returned;
                }
                Err(error) => {
                    let (executed, diagnostic) = error.into_parts();
                    let (_pending_exit, returned_buffer) = executed.into_pending_exit_invocation();
                    return Err(Box::new(UefiOsHandoffCycleRejection::MapAcquisition {
                        pending_exit,
                        buffer: returned_buffer,
                        arrival,
                        diagnostic,
                    }));
                }
            }
        };
        let acquired = match handoff.acquire_memory_map(arrival, acquisition) {
            Ok(acquired) => acquired,
            Err(error) => {
                let (arrival, acquisition, diagnostic) = error.into_parts();
                return Err(Box::new(UefiOsHandoffCycleRejection::MapAdmission {
                    pending_exit,
                    buffer,
                    arrival,
                    acquisition,
                    diagnostic,
                }));
            }
        };
        let bound = match bind_uefi_exit_boot_services_invocation(pending_exit, &acquired) {
            Ok(bound) => bound,
            Err(error) => {
                let (pending_exit, diagnostic) = error.into_parts();
                return Err(Box::new(UefiOsHandoffCycleRejection::ExitBinding {
                    pending_exit,
                    buffer,
                    acquired,
                    diagnostic,
                }));
            }
        };
        // SAFETY: the function's safety contract covers the exit execution;
        // the bound carrier holds the exact retained service, physical image
        // handle, and the key the just-acquired map sealed.
        let executed = match unsafe { execute_uefi_exit_boot_services(bound) } {
            Ok(executed) => executed,
            Err(error) => {
                let (bound, diagnostic) = error.into_parts();
                return Err(Box::new(UefiOsHandoffCycleRejection::ExitExecution {
                    bound,
                    buffer,
                    acquired,
                    diagnostic,
                }));
            }
        };
        let (returned_exit, result) = match admit_uefi_exit_boot_services_execution(executed) {
            Ok(UefiExitBootServicesAttemptOutcome::Retry {
                invocation, result, ..
            }) => (Some(invocation), result),
            Ok(UefiExitBootServicesAttemptOutcome::Exited { result, .. }) => (None, result),
            Err(error) => {
                let (execution, diagnostic) = error.into_parts();
                return Err(Box::new(UefiOsHandoffCycleRejection::ExitAdmission {
                    execution,
                    buffer,
                    acquired,
                    diagnostic,
                }));
            }
        };
        match handoff.apply_exit_boot_services_result(acquired, result) {
            Ok(UefiOsHandoffProgress::Retry(next)) => {
                arrival = next;
                // `Retry` progress can only come from a stale-key provider
                // result, whose admission returned the pending-exit custody.
                pending_exit =
                    returned_exit.expect("stale-key admission returns pending-exit custody");
            }
            Ok(UefiOsHandoffProgress::Exhausted(exhaustion)) => {
                return Ok(UefiOsHandoffCycleResolution::Exhausted {
                    exhaustion,
                    pending_exit: returned_exit
                        .expect("stale-key admission returns pending-exit custody"),
                    buffer,
                });
            }
            Ok(UefiOsHandoffProgress::Complete(completion)) => {
                debug_assert!(
                    returned_exit.is_none(),
                    "a successful exit consumed provider custody inside admission"
                );
                return Ok(UefiOsHandoffCycleResolution::Complete { completion, buffer });
            }
            Err(error) => {
                let (acquired, result, diagnostic) = error.into_parts();
                return Err(Box::new(match returned_exit {
                    Some(pending_exit) => UefiOsHandoffCycleRejection::StaleTransition {
                        pending_exit,
                        buffer,
                        acquired,
                        result,
                        diagnostic,
                    },
                    None => UefiOsHandoffCycleRejection::ExitTransition {
                        buffer,
                        acquired,
                        result,
                        diagnostic,
                    },
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExternalRootDiagnostic, PlannedUefiExitBootServicesInvocation,
        UefiApplicationFirmwareLedger, UefiMemoryMapBuffer, UefiOsHandoffCycleRejection,
        UefiOsHandoffCycleResolution, UefiOsHandoffLedger, UefiOsHandoffMapRequired,
        drive_uefi_os_handoff_cycle,
    };
    use crate::{
        LifecycleScopedUefiBootServicesProjection, UefiApplicationBootstrapLedgerId,
        UefiBootServicesPhaseLeaseId, UefiBootServicesTableOccurrenceId, UefiErrorStatus,
        UefiFirmwareSessionId, UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId,
        UefiOsHandoffId, UefiOsHandoffStackEvidenceId, UefiPhysicalInvocationId,
        UefiSystemTableOccurrenceId, join_lifecycle_scoped_uefi_exit_boot_services_provider,
        join_lifecycle_scoped_uefi_system_table, join_uefi_application_physical_arrival,
        prepare_uefi_application_bootstrap_adapter_invocation,
        prepare_uefi_exit_boot_services_invocation, project_uefi_application_boot_services,
    };
    use program_entry_plan::{
        ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
        UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
    };
    use std::collections::VecDeque;
    use std::ffi::c_void;
    use std::num::{NonZeroU32, NonZeroU64};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use target::{
        ProgramEntryPhysicalContractPackage, TargetProfile, UEFI_BOOT_SERVICES_SIGNATURE,
        UEFI_SYSTEM_TABLE_SIGNATURE, UefiBootServicesNativeField,
        plan_uefi_boot_services_native_layout, plan_uefi_system_table_native_layout,
        validate_uefi_boot_services_occurrence, validate_uefi_system_table_occurrence,
    };

    const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;
    const EFI_SUCCESS: u64 = 0;
    const EFI_INVALID_PARAMETER: u64 = EFI_STATUS_ERROR_BIT | 2;
    const EFI_BUFFER_TOO_SMALL: u64 = EFI_STATUS_ERROR_BIT | 5;

    /// One scripted `GetMemoryMap` firmware response.
    #[derive(Clone, Copy)]
    enum FakeGetResponse {
        Success {
            key: usize,
            map_bytes: usize,
            descriptor_size: usize,
            version: u64,
        },
        BufferTooSmall {
            required: usize,
        },
        Raw(u64),
    }

    static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static GET_RESPONSES: Mutex<VecDeque<FakeGetResponse>> = Mutex::new(VecDeque::new());
    static EXIT_RESPONSES: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::new());
    static OBSERVED_GET_CALLS: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_GET_CAPACITIES: Mutex<Vec<usize>> = Mutex::new(Vec::new());
    static OBSERVED_EXIT_KEYS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

    fn script_get(responses: &[FakeGetResponse]) {
        *GET_RESPONSES.lock().unwrap() = responses.iter().copied().collect();
    }

    fn script_exit(statuses: &[u64]) {
        *EXIT_RESPONSES.lock().unwrap() = statuses.iter().copied().collect();
    }

    fn reset_observers() {
        OBSERVED_GET_CALLS.store(0, Ordering::SeqCst);
        OBSERVED_GET_CAPACITIES.lock().unwrap().clear();
        OBSERVED_EXIT_KEYS.lock().unwrap().clear();
    }

    /// Each call consumes the next scripted step until one remains, which then
    /// repeats — a trailing success or failure answer stays live for every
    /// later call in the same drive.
    fn next_get_response() -> FakeGetResponse {
        let mut responses = GET_RESPONSES.lock().unwrap();
        let response = *responses.front().expect("scripted GetMemoryMap response");
        if responses.len() > 1 {
            responses.pop_front();
        }
        response
    }

    fn next_exit_status() -> u64 {
        let mut responses = EXIT_RESPONSES.lock().unwrap();
        let status = *responses.front().expect("scripted ExitBootServices status");
        if responses.len() > 1 {
            responses.pop_front();
        }
        status
    }

    /// Firmware-shaped fake: reads the in-out size cell, writes the scripted
    /// required extent on BufferTooSmall and the map bytes plus every output
    /// cell on success.
    unsafe extern "efiapi" fn fake_get_memory_map(
        memory_map_size: *mut usize,
        memory_map: *mut c_void,
        map_key: *mut usize,
        descriptor_size: *mut usize,
        descriptor_version: *mut u32,
    ) -> u64 {
        OBSERVED_GET_CALLS.fetch_add(1, Ordering::SeqCst);
        // SAFETY: the call cells belong to the bound test buffer for exactly
        // this call, matching the edge's safety contract.
        let capacity = unsafe { *memory_map_size };
        OBSERVED_GET_CAPACITIES.lock().unwrap().push(capacity);
        match next_get_response() {
            FakeGetResponse::Success {
                key,
                map_bytes,
                descriptor_size: stride,
                version,
            } => {
                let writable = map_bytes.min(capacity);
                if !memory_map.is_null() && writable != 0 {
                    // SAFETY: the edge guarantees `capacity` writable bytes.
                    unsafe { std::ptr::write_bytes(memory_map.cast::<u8>(), 0x5a, writable) };
                }
                // SAFETY: same exclusive output-cell custody as above.
                unsafe {
                    *memory_map_size = map_bytes;
                    *map_key = key;
                    *descriptor_size = stride;
                    *descriptor_version = version as u32;
                }
                EFI_SUCCESS
            }
            FakeGetResponse::BufferTooSmall { required } => {
                // SAFETY: same exclusive output-cell custody as above.
                unsafe { *memory_map_size = required };
                EFI_BUFFER_TOO_SMALL
            }
            FakeGetResponse::Raw(status) => status,
        }
    }

    unsafe extern "efiapi" fn fake_exit_boot_services(
        _image_handle: *mut c_void,
        map_key: usize,
    ) -> u64 {
        OBSERVED_EXIT_KEYS.lock().unwrap().push(map_key);
        next_exit_status()
    }

    fn fake_get_memory_map_address() -> u64 {
        fake_get_memory_map as *const () as usize as u64
    }

    fn fake_exit_boot_services_address() -> u64 {
        fake_exit_boot_services as *const () as usize as u64
    }

    fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(value).unwrap()
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for (index, byte) in bytes.iter().copied().enumerate() {
            let byte = if (16..20).contains(&index) { 0 } else { byte };
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    fn table(signature: u64, size: usize, pointer_offset: usize, pointer: u64) -> Vec<u8> {
        let mut bytes = vec![0; size];
        bytes[0..8].copy_from_slice(&signature.to_le_bytes());
        bytes[8..12].copy_from_slice(&((2_u32 << 16) | 100).to_le_bytes());
        bytes[12..16].copy_from_slice(&(size as u32).to_le_bytes());
        bytes[pointer_offset..pointer_offset + 8].copy_from_slice(&pointer.to_le_bytes());
        let crc = crc32(&bytes);
        bytes[16..20].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    /// Fabricated Boot Services table carrying both handoff-edge rows at their
    /// exact target-layout offsets.
    fn boot_table(get_memory_map: u64, exit_boot_services: u64) -> Vec<u8> {
        let layout = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap();
        let get_offset = layout
            .field_layout(UefiBootServicesNativeField::GetMemoryMap)
            .unwrap()
            .byte_offset() as usize;
        let exit_offset = layout
            .field_layout(UefiBootServicesNativeField::ExitBootServices)
            .unwrap()
            .byte_offset() as usize;
        let mut bytes = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            get_offset,
            get_memory_map,
        );
        bytes[exit_offset..exit_offset + 8].copy_from_slice(&exit_boot_services.to_le_bytes());
        let crc = crc32(&bytes);
        bytes[16..20].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    fn physical_contract() -> ProgramEntryPhysicalContractPlan {
        let expected = exact_uefi_x64_physical_boundary_entry_plan();
        ProgramEntryPhysicalContractPlan::new(
            TargetProfile::UefiX64.program_entry_slot(),
            UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
            ProgramEntryPhysicalContractPackage::UefiX64,
            exact_uefi_x64_physical_contract_package_source_digest(),
            1,
            vec![
                UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY.into(),
                UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY.into(),
            ],
            UEFI_X64_STATUS_TYPE_IDENTITY.into(),
            expected.contract_report_fingerprint(),
            expected.plan().clone(),
        )
        .unwrap()
    }

    fn ledger<'a>(base: u64) -> UefiApplicationFirmwareLedger<'a> {
        UefiApplicationFirmwareLedger::new(
            id(
                base,
                UefiApplicationBootstrapLedgerId::from_normalized_identity,
            ),
            id(base + 1, UefiFirmwareSessionId::from_normalized_identity),
            id(base + 2, UefiPhysicalInvocationId::from_normalized_identity),
        )
        .unwrap()
    }

    /// The firmware ledger minted by `ledger(base)` names `base + 2` as its
    /// physical invocation; every helper here shares that convention so the
    /// handoff ledger and provider edges join the same invocation.
    const INVOCATION_OFFSET: u64 = 2;

    fn projection_with_image_handle<'a>(
        ledger: &mut UefiApplicationFirmwareLedger<'a>,
        bytes: &'a [u8],
        base: u64,
        retain_physical_value: bool,
    ) -> LifecycleScopedUefiBootServicesProjection<'a> {
        let occurrence = id(
            base,
            crate::UefiImageHandleOccurrenceId::from_normalized_identity,
        );
        let image = if retain_physical_value {
            ledger.admit_image_handle_physical_input(
                occurrence,
                NonZeroU64::new(0x1000_0000 + base).unwrap(),
            )
        } else {
            ledger.admit_image_handle_occurrence(occurrence)
        }
        .unwrap();
        let integrity = validate_uefi_system_table_occurrence(
            plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
            bytes,
        )
        .unwrap();
        let provenance = ledger
            .admit_system_table_occurrence(
                id(
                    base + 1,
                    UefiSystemTableOccurrenceId::from_normalized_identity,
                ),
                integrity.table_bytes(),
            )
            .unwrap();
        let lease = ledger
            .acquire_boot_services_phase_lease(id(
                base + 2,
                UefiBootServicesPhaseLeaseId::from_normalized_identity,
            ))
            .unwrap();
        let scoped =
            join_lifecycle_scoped_uefi_system_table(ledger, integrity, provenance, lease).unwrap();
        let arrival =
            join_uefi_application_physical_arrival(ledger, image, scoped, physical_contract())
                .unwrap();
        let readiness =
            prepare_uefi_application_bootstrap_adapter_invocation(ledger, arrival).unwrap();
        project_uefi_application_boot_services(ledger, readiness).unwrap()
    }

    /// The pending exit invocation the cycle borrows for acquisition and
    /// consumes at the exit binding.
    fn planned_exit<'system_table, 'boot_services>(
        ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
        system: &'system_table [u8],
        boot: &'boot_services [u8],
        base: u64,
        boot_address: u64,
        retain_image_handle_value: bool,
    ) -> PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
        let projection =
            projection_with_image_handle(ledger, system, base, retain_image_handle_value);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            boot,
        )
        .unwrap();
        let provider = join_lifecycle_scoped_uefi_exit_boot_services_provider(
            ledger,
            projection,
            integrity,
            id(
                base + 3,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap();
        prepare_uefi_exit_boot_services_invocation(provider).unwrap()
    }

    /// The handoff ledger binds the *firmware ledger's* session and physical
    /// invocation: `session`/`invocation` are the firmware base's +1/+2 ids so
    /// executed acquisition evidence joins the same invocation.
    fn handoff(
        base: u64,
        session: u64,
        invocation: u64,
        attempts: u32,
    ) -> (UefiOsHandoffLedger, UefiOsHandoffMapRequired) {
        UefiOsHandoffLedger::new(
            id(base, UefiOsHandoffId::from_normalized_identity),
            id(session, UefiFirmwareSessionId::from_normalized_identity),
            id(
                invocation,
                UefiPhysicalInvocationId::from_normalized_identity,
            ),
            id(
                base + 3,
                UefiOsHandoffBootServicesId::from_normalized_identity,
            ),
            id(
                base + 4,
                UefiOsHandoffAllocationRosterId::from_normalized_identity,
            ),
            id(
                base + 5,
                UefiOsHandoffStackEvidenceId::from_normalized_identity,
            ),
            NonZeroU32::new(attempts).unwrap(),
            UefiErrorStatus::from_target_status(EFI_STATUS_ERROR_BIT).unwrap(),
        )
        .unwrap()
    }

    fn success(key: usize) -> FakeGetResponse {
        FakeGetResponse::Success {
            key,
            map_bytes: 96,
            descriptor_size: 48,
            version: 1,
        }
    }

    #[test]
    fn cycle_acquires_then_exits_with_the_acquired_key() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_get(&[success(0x5AFE_1001)]);
        script_exit(&[EFI_SUCCESS]);
        reset_observers();
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        let allocations = arrival.allocation_roster();
        let stack = arrival.surviving_stack();
        let mut map = UefiMemoryMapBuffer::with_capacity(128);

        // SAFETY: the fabricated tables retain the test-local efiapi fakes as
        // the service addresses; the buffer is exclusively owned for the drive.
        let resolution = unsafe {
            drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
        }
        .unwrap();
        let UefiOsHandoffCycleResolution::Complete { completion, buffer } = resolution else {
            panic!("a successful exit must complete the handoff")
        };
        assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1001]);
        assert_eq!(completion.physical_invocation().normalized_identity(), 12);
        assert_eq!(completion.allocation_roster(), allocations);
        assert_eq!(completion.surviving_stack(), stack);
        assert_ne!(completion.receipt().normalized_identity(), 0);
        // The buffer still holds the final map's occupied prefix.
        assert_eq!(buffer.occupied_map_bytes(), 96);
        assert!(buffer.occupied_bytes().iter().all(|byte| *byte == 0x5a));
    }

    #[test]
    fn grow_and_stale_key_retries_carry_the_newest_key_to_exit() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_get(&[
            FakeGetResponse::BufferTooSmall { required: 192 },
            success(0x5AFE_1002),
            success(0x5AFE_1003),
        ]);
        script_exit(&[EFI_INVALID_PARAMETER, EFI_SUCCESS]);
        reset_observers();
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        let mut map = UefiMemoryMapBuffer::default();

        // SAFETY: same fabricated custody as above.
        let resolution = unsafe {
            drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
        }
        .unwrap();
        let UefiOsHandoffCycleResolution::Complete { completion, buffer } = resolution else {
            panic!("the retried success must complete the handoff")
        };
        // The zero-capacity probe grew to the firmware requirement without
        // spending a handoff attempt; the stale first key drove a fresh
        // acquisition, so the exit saw exactly the two newest keys in order.
        assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 3);
        assert_eq!(*OBSERVED_GET_CAPACITIES.lock().unwrap(), [0, 192, 192]);
        assert_eq!(
            *OBSERVED_EXIT_KEYS.lock().unwrap(),
            [0x5AFE_1002, 0x5AFE_1003]
        );
        assert_eq!(buffer.capacity(), 192);
        assert_eq!(buffer.occupied_map_bytes(), 96);
        assert_eq!(completion.physical_invocation().normalized_identity(), 12);
    }

    #[test]
    fn exhausted_cycle_returns_live_provider_and_buffer_custody() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_get(&[
            FakeGetResponse::BufferTooSmall { required: 192 },
            success(0x5AFE_1004),
        ]);
        script_exit(&[EFI_INVALID_PARAMETER]);
        reset_observers();
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 1);
        let mut map = UefiMemoryMapBuffer::default();

        // SAFETY: same fabricated custody as above.
        let resolution = unsafe {
            drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
        }
        .unwrap();
        let UefiOsHandoffCycleResolution::Exhausted {
            exhaustion,
            pending_exit,
            buffer,
        } = resolution
        else {
            panic!("the single stale key must exhaust the bounded cycle")
        };
        // The grow retry never spent the only attempt: acquisition still ran
        // the probe and the grown call, and one stale exit exhausted the loop.
        assert_eq!(*OBSERVED_GET_CAPACITIES.lock().unwrap(), [0, 192]);
        assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1004]);
        assert_eq!(exhaustion.status().value(), EFI_STATUS_ERROR_BIT);
        assert_eq!(exhaustion.boot_services().normalized_identity(), 53);
        assert_eq!(buffer.occupied_map_bytes(), 96);

        // Exhaustion leaves Boot Services live: the returned pending-exit
        // custody still releases so the invocation can begin its firmware
        // return.
        let released = firmware
            .release_planned_uefi_exit_boot_services_invocation(pending_exit)
            .unwrap();
        assert_eq!(released.ledger, firmware.ledger_id());
        firmware.begin_firmware_return().unwrap();
    }

    #[test]
    fn acquisition_rejection_returns_complete_custody_for_a_redrive() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_get(&[FakeGetResponse::Raw(EFI_INVALID_PARAMETER)]);
        script_exit(&[EFI_SUCCESS]);
        reset_observers();
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        let mut map = UefiMemoryMapBuffer::with_capacity(128);

        // SAFETY: same fabricated custody as above.
        let rejection = unsafe {
            drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
        }
        .unwrap_err();
        let UefiOsHandoffCycleRejection::MapAcquisition {
            pending_exit,
            buffer,
            arrival,
            diagnostic,
        } = *rejection
        else {
            panic!("a rejected acquisition status must return unspent custody")
        };
        assert!(diagnostic.0.contains("InvalidParameter"));
        assert_eq!(arrival.remaining_attempts(), 2);

        // The returned custody still drives a clean cycle to completion.
        script_get(&[success(0x5AFE_1005)]);
        // SAFETY: the same fabricated custody re-drives with a fixed script.
        let resolution = unsafe {
            drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, buffer)
        }
        .unwrap();
        let UefiOsHandoffCycleResolution::Complete { completion, .. } = resolution else {
            panic!("the re-driven custody must complete the handoff")
        };
        assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1005]);
        assert_eq!(completion.physical_invocation().normalized_identity(), 12);
    }

    #[test]
    fn foreign_arrival_rejects_after_acquisition_with_unspent_evidence() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_get(&[success(0x5AFE_1006)]);
        script_exit(&[EFI_SUCCESS]);
        reset_observers();
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
        // An arrival minted by a different handoff ledger carries no custody
        // in this one; the acquisition executes first, then the ledger
        // rejects it with the unspent evidence returned.
        let (_foreign_handoff, foreign_arrival) = handoff(90, 91, 92, 2);
        let (mut handoff, _live_arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        let mut map = UefiMemoryMapBuffer::with_capacity(128);

        // SAFETY: same fabricated custody as above.
        let rejection = unsafe {
            drive_uefi_os_handoff_cycle(
                &firmware,
                &mut handoff,
                foreign_arrival,
                pending_exit,
                &mut map,
            )
        }
        .unwrap_err();
        let UefiOsHandoffCycleRejection::MapAdmission {
            acquisition,
            diagnostic,
            ..
        } = *rejection
        else {
            panic!("a foreign arrival must reject at ledger admission")
        };
        assert!(diagnostic.0.contains("foreign, stale, or has lost custody"));
        assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 1);
        assert!(OBSERVED_EXIT_KEYS.lock().unwrap().is_empty());
        assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_1006);
        assert_eq!(acquisition.physical_invocation().normalized_identity(), 12);
    }

    #[test]
    fn exit_status_outside_the_closed_table_returns_executed_custody() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_get(&[success(0x5AFE_1007)]);
        script_exit(&[EFI_STATUS_ERROR_BIT | 9]);
        reset_observers();
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        let mut map = UefiMemoryMapBuffer::with_capacity(128);

        // SAFETY: same fabricated custody as above.
        let rejection = unsafe {
            drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
        }
        .unwrap_err();
        let UefiOsHandoffCycleRejection::ExitAdmission {
            execution,
            diagnostic,
            ..
        } = *rejection
        else {
            panic!("an unknown exit status must return executed custody")
        };
        assert!(diagnostic.0.contains("closed target table"));
        assert_eq!(execution.map_key().normalized_identity(), 0x5AFE_1007);
        assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1007]);

        // The executed custody still releases through the firmware ledger.
        let released = firmware
            .release_executed_uefi_exit_boot_services_invocation(execution)
            .unwrap();
        assert_eq!(released.ledger, firmware.ledger_id());
        firmware.begin_firmware_return().unwrap();
    }

    #[test]
    fn address_free_image_handle_rejects_at_exit_binding_with_acquired_intact() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_get(&[success(0x5AFE_1008)]);
        script_exit(&[EFI_SUCCESS]);
        reset_observers();
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        // The pending exit retains no physical image-handle value, so the exit
        // operand edge cannot bind RCX; the acquired attempt returns intact.
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, false);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        let mut map = UefiMemoryMapBuffer::with_capacity(128);

        // SAFETY: same fabricated custody as above.
        let rejection = unsafe {
            drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
        }
        .unwrap_err();
        let UefiOsHandoffCycleRejection::ExitBinding {
            pending_exit,
            acquired,
            diagnostic,
            ..
        } = *rejection
        else {
            panic!("an address-free image handle must reject at exit binding")
        };
        assert!(diagnostic.0.contains("image-handle value"));
        assert_eq!(acquired.map_key().normalized_identity(), 0x5AFE_1008);
        assert!(OBSERVED_EXIT_KEYS.lock().unwrap().is_empty());

        let released = firmware
            .release_planned_uefi_exit_boot_services_invocation(pending_exit)
            .unwrap();
        assert_eq!(released.ledger, firmware.ledger_id());
        firmware.begin_firmware_return().unwrap();
    }

    #[test]
    fn the_cycle_composes_edges_without_minting_evidence() {
        let production = include_str!("os_handoff_cycle.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production cycle source");
        let compact = production
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        for forbidden in [
            "for_test",
            "stale_map_key(",
            "::succeeded(",
            "UefiMemoryMapAcquisition{",
            "UefiExitBootServicesProviderResult{",
            "UefiOsHandoffMapRequired{",
            "UefiOsHandoffMapAcquired{",
        ] {
            assert!(
                !compact.contains(forbidden),
                "forbidden handoff evidence authority appeared: {forbidden}"
            );
        }
    }
}
