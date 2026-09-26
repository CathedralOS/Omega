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

use crate::external_roots::ExternalRootDiagnostic;

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
mod tests;
