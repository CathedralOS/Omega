//! Executed get-memory-map invocations, acquisitions and admission.

use crate::identities::Fnv1a;
use crate::platform_bringup::uefi_bootstrap::PlannedUefiExitBootServicesInvocation;
use crate::platform_bringup::uefi_bootstrap::get_memory_map::invocation_planning::UefiGetMemoryMapFunction;
use crate::platform_bringup::uefi_bootstrap::get_memory_map::{
    BoundUefiGetMemoryMapInvocation, MIN_MEMORY_DESCRIPTOR_BYTES,
    PlannedUefiGetMemoryMapInvocation, UefiMemoryMapBuffer,
};
use crate::{
    ExternalRootDiagnostic, UefiFirmwareSessionId, UefiImageHandleOccurrenceId, UefiMemoryMapKeyId,
    UefiMemoryMapSnapshotId, UefiPhysicalInvocationId,
};
use program_entry_plan::UefiOsHandoffStatusRole;
use std::ffi::c_void;
use std::num::NonZeroU64;

/// Closed interpretation of the exact returned `EFI_STATUS`, derived from the
/// custody role the retained `GetMemoryMap` leg assigns the code.
/// `BufferTooSmall` is the grow-and-retry outcome; `Rejected` is a
/// closed-table row with no acquisition role; codes outside the leg's table
/// stay observable as unsupported execution results. Both rejecting shapes
/// retain their custody for release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UefiGetMemoryMapAttemptStatus {
    /// The leg's `MapAcquired` row.
    Success,
    /// The leg's `GrowMapBuffer` row.
    BufferTooSmall,
    /// The leg's `Reject` row.
    Rejected,
    Unknown,
}

/// Sealed evidence that the exact retained service was invoked once against
/// the bound buffer. The status and the four output cells are sealed copies;
/// admission replays them against the still-borrowed buffer before minting
/// acquisition evidence.
#[must_use = "executed UEFI GetMemoryMap custody must be admitted or released"]
pub struct ExecutedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>
{
    invocation:
        BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
    status_code: u64,
    map_size: usize,
    map_key: usize,
    descriptor_size: usize,
    descriptor_version: u32,
}

impl std::fmt::Debug for ExecutedUefiGetMemoryMapInvocation<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutedUefiGetMemoryMapInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("status_code", &self.status_code())
            .field("status", &self.status())
            .field("map_size", &self.map_size)
            .field("descriptor_size", &self.descriptor_size)
            .field("descriptor_version", &self.descriptor_version)
            .finish_non_exhaustive()
    }
}

impl ExecutedUefiGetMemoryMapInvocation<'_, '_, '_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }
    pub const fn status_code(&self) -> u64 {
        self.status_code
    }
    pub fn status(&self) -> UefiGetMemoryMapAttemptStatus {
        match self
            .invocation
            .invocation
            .plan
            .status_role(self.status_code)
        {
            Some(UefiOsHandoffStatusRole::MapAcquired) => UefiGetMemoryMapAttemptStatus::Success,
            Some(UefiOsHandoffStatusRole::GrowMapBuffer) => {
                UefiGetMemoryMapAttemptStatus::BufferTooSmall
            }
            Some(UefiOsHandoffStatusRole::Reject) => UefiGetMemoryMapAttemptStatus::Rejected,
            // The acquisition leg's closed table carries only the three rows
            // above; an exit-side role here is outside its contract.
            Some(_) | None => UefiGetMemoryMapAttemptStatus::Unknown,
        }
    }
}

impl<'pending_exit, 'system_table, 'boot_services, 'buffer>
    ExecutedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>
{
    /// Release executed custody without a provider outcome: the pending-exit
    /// borrow and the buffer return to the caller unchanged.
    pub fn into_pending_exit_invocation(
        self,
    ) -> (
        &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        &'buffer mut UefiMemoryMapBuffer,
    ) {
        (
            self.invocation.invocation.provider.pending_exit,
            self.invocation.buffer,
        )
    }
}

#[derive(Debug)]
#[must_use = "UEFI GetMemoryMap execution rejection retains bound provider custody"]
pub struct UefiGetMemoryMapExecutionError<'pending_exit, 'system_table, 'boot_services, 'buffer> {
    invocation:
        BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'pending_exit, 'system_table, 'boot_services, 'buffer>
    UefiGetMemoryMapExecutionError<'pending_exit, 'system_table, 'boot_services, 'buffer>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.diagnostic)
    }
}

impl std::fmt::Display for UefiGetMemoryMapExecutionError<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiGetMemoryMapExecutionError<'_, '_, '_, '_> {}

/// Invoke the exact retained UEFI `GetMemoryMap` service once.
///
/// # Safety
///
/// The target-runtime caller must establish that the retained numeric service
/// value came from the live UEFI physical invocation and remains callable for
/// this operation, and that the bound buffer's cells and descriptor storage
/// remain exclusively owned and writable for the duration of the call. On
/// success firmware must have written the occupied size, key, descriptor size,
/// and descriptor version cells and up to the reported size of map bytes; on
/// `EFI_BUFFER_TOO_SMALL` only the size cell carries the required extent. This
/// is the sole host-language unsafe premise; the resulting receipt cannot be
/// constructed directly.
pub unsafe fn execute_uefi_get_memory_map<'pending_exit, 'system_table, 'boot_services, 'buffer>(
    invocation: BoundUefiGetMemoryMapInvocation<
        'pending_exit,
        'system_table,
        'boot_services,
        'buffer,
    >,
) -> Result<
    ExecutedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
    Box<UefiGetMemoryMapExecutionError<'pending_exit, 'system_table, 'boot_services, 'buffer>>,
> {
    if !invocation.invocation.retains_exact_plan() {
        return Err(Box::new(UefiGetMemoryMapExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI GetMemoryMap execution operands drifted after binding".into(),
            ),
        }));
    }
    let Ok(service_address) = usize::try_from(invocation._service.get()) else {
        return Err(Box::new(UefiGetMemoryMapExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI GetMemoryMap service address does not fit the runtime pointer carrier".into(),
            ),
        }));
    };
    let memory_map = if invocation.buffer.map.is_empty() {
        std::ptr::null_mut()
    } else {
        invocation.buffer.map.as_mut_ptr().cast::<c_void>()
    };
    // SAFETY: the function-pointer validity and exact EFI ABI are the explicit
    // caller obligations above. The bound carrier owns the only route to these
    // private operands; the raw cell pointers borrow the exclusively held
    // buffer for exactly this call.
    let service: UefiGetMemoryMapFunction = unsafe { std::mem::transmute(service_address) };
    // SAFETY: upheld by the same target-runtime contract. A null map pointer
    // is the zero-capacity probe: the size cell reads zero and firmware
    // answers EFI_BUFFER_TOO_SMALL without touching descriptor storage.
    let status_code = unsafe {
        service(
            &mut invocation.buffer.map_size as *mut usize,
            memory_map,
            &mut invocation.buffer.map_key as *mut usize,
            &mut invocation.buffer.descriptor_size as *mut usize,
            &mut invocation.buffer.descriptor_version as *mut u32,
        )
    };
    let (map_size, map_key, descriptor_size, descriptor_version) = (
        invocation.buffer.map_size,
        invocation.buffer.map_key,
        invocation.buffer.descriptor_size,
        invocation.buffer.descriptor_version,
    );
    Ok(ExecutedUefiGetMemoryMapInvocation {
        invocation,
        status_code,
        map_size,
        map_key,
        descriptor_size,
        descriptor_version,
    })
}

/// Sole-issuance acquisition evidence minted by one admitted `GetMemoryMap`
/// execution. It binds the firmware session and physical invocation of the
/// borrowed exit custody, the derived snapshot identity, the exact physical
/// map key, and the descriptor geometry of the occupied map extent. The
/// handoff ledger consumes it once to form `UefiOsHandoffMapAcquired`; no
/// other construction exists, so the exit edge can only be fed by a real
/// firmware acquisition of the same invocation.
#[must_use = "UEFI memory-map acquisition evidence must feed the handoff ledger"]
pub struct UefiMemoryMapAcquisition {
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    snapshot: UefiMemoryMapSnapshotId,
    map_key: UefiMemoryMapKeyId,
    map_bytes: usize,
    descriptor_size: usize,
    descriptor_version: u32,
}

impl std::fmt::Debug for UefiMemoryMapAcquisition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiMemoryMapAcquisition")
            .field("session", &self.session)
            .field("invocation", &self.invocation)
            .field("snapshot", &self.snapshot)
            .field("map_key", &self.map_key)
            .field("map_bytes", &self.map_bytes)
            .field("descriptor_size", &self.descriptor_size)
            .field("descriptor_version", &self.descriptor_version)
            .finish_non_exhaustive()
    }
}

impl UefiMemoryMapAcquisition {
    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.session
    }
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation
    }
    pub const fn snapshot(&self) -> UefiMemoryMapSnapshotId {
        self.snapshot
    }
    /// The exact `UINTN` key the firmware wrote for this map. Its normalized
    /// identity is the physical operand the exit edge binds, so the key that
    /// reaches `ExitBootServices` always names the most recent acquisition.
    pub const fn map_key(&self) -> UefiMemoryMapKeyId {
        self.map_key
    }
    /// Bytes of map storage the firmware occupied inside the bound buffer.
    pub const fn map_bytes(&self) -> usize {
        self.map_bytes
    }
    pub const fn descriptor_size(&self) -> usize {
        self.descriptor_size
    }
    pub const fn descriptor_version(&self) -> u32 {
        self.descriptor_version
    }
}

/// Exact outcome of one admitted `GetMemoryMap` attempt. Both variants return
/// the complete provider and buffer custody; only the sealed acquisition
/// evidence differs.
#[must_use = "UEFI GetMemoryMap attempt outcome must feed the handoff ledger or release custody"]
pub enum UefiGetMemoryMapAttemptOutcome<'pending_exit, 'system_table, 'boot_services, 'buffer> {
    /// `EFI_SUCCESS`: the buffer holds the occupied map prefix and the
    /// acquisition evidence carries the exact key the next exit attempt must
    /// present.
    Acquired {
        invocation: PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        acquisition: UefiMemoryMapAcquisition,
    },
    /// `EFI_BUFFER_TOO_SMALL`: the firmware wrote the required extent into the
    /// size cell. Grow the buffer to at least `required_map_bytes` and rebind;
    /// no handoff attempt was spent.
    BufferTooSmall {
        invocation: PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
        required_map_bytes: usize,
    },
}

impl std::fmt::Debug for UefiGetMemoryMapAttemptOutcome<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Acquired { acquisition, .. } => formatter
                .debug_struct("UefiGetMemoryMapAttemptOutcome::Acquired")
                .field("acquisition", acquisition)
                .finish_non_exhaustive(),
            Self::BufferTooSmall {
                required_map_bytes, ..
            } => formatter
                .debug_struct("UefiGetMemoryMapAttemptOutcome::BufferTooSmall")
                .field("required_map_bytes", required_map_bytes)
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Debug)]
#[must_use = "UEFI GetMemoryMap attempt rejection retains executed custody"]
pub struct UefiGetMemoryMapAttemptError<'pending_exit, 'system_table, 'boot_services, 'buffer> {
    execution:
        ExecutedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'pending_exit, 'system_table, 'boot_services, 'buffer>
    UefiGetMemoryMapAttemptError<'pending_exit, 'system_table, 'boot_services, 'buffer>
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
        ExecutedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
        ExternalRootDiagnostic,
    ) {
        (self.execution, self.diagnostic)
    }
}

impl std::fmt::Display for UefiGetMemoryMapAttemptError<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiGetMemoryMapAttemptError<'_, '_, '_, '_> {}

/// Consume one exact execution receipt and classify its sealed status. This
/// is the sole route minting `UefiMemoryMapAcquisition`: a success whose
/// sealed cells violate the firmware contract (empty or over-capacity map,
/// sub-descriptor geometry, unaligned extent, zero key or version) rejects
/// with executed custody retained rather than minting unusable evidence.
pub fn admit_uefi_get_memory_map_execution<
    'pending_exit,
    'system_table,
    'boot_services,
    'buffer,
>(
    execution: ExecutedUefiGetMemoryMapInvocation<
        'pending_exit,
        'system_table,
        'boot_services,
        'buffer,
    >,
) -> Result<
    UefiGetMemoryMapAttemptOutcome<'pending_exit, 'system_table, 'boot_services, 'buffer>,
    Box<UefiGetMemoryMapAttemptError<'pending_exit, 'system_table, 'boot_services, 'buffer>>,
> {
    let reject = |execution, message: &'static str| {
        Err(Box::new(UefiGetMemoryMapAttemptError {
            execution,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };
    if execution.invocation.buffer.map_size != execution.map_size
        || execution.invocation.buffer.map_key != execution.map_key
        || execution.invocation.buffer.descriptor_size != execution.descriptor_size
        || execution.invocation.buffer.descriptor_version != execution.descriptor_version
    {
        return reject(
            execution,
            "UEFI GetMemoryMap output cells drifted after provider execution",
        );
    }
    match execution.status() {
        UefiGetMemoryMapAttemptStatus::Success => {
            if execution.map_size == 0
                || execution.map_size > execution.invocation.buffer.capacity()
            {
                return reject(
                    execution,
                    "UEFI GetMemoryMap success reported an empty or over-capacity map extent",
                );
            }
            if execution.descriptor_size < MIN_MEMORY_DESCRIPTOR_BYTES
                || !execution.map_size.is_multiple_of(execution.descriptor_size)
            {
                return reject(
                    execution,
                    "UEFI GetMemoryMap success reported drifted descriptor geometry",
                );
            }
            if execution.descriptor_version == 0 {
                return reject(
                    execution,
                    "UEFI GetMemoryMap success reported a zero descriptor version",
                );
            }
            let Ok(raw_map_key) = u64::try_from(execution.map_key) else {
                return reject(
                    execution,
                    "UEFI GetMemoryMap map key does not fit the normalized identity carrier",
                );
            };
            let Some(raw_map_key) = NonZeroU64::new(raw_map_key) else {
                return reject(
                    execution,
                    "UEFI GetMemoryMap success reported a zero map key that cannot name custody",
                );
            };
            let map_key = UefiMemoryMapKeyId::from_normalized_identity(raw_map_key.get())
                .expect("nonzero map key is a valid normalized identity");
            let provider = &execution.invocation.invocation.provider;
            let mut hash = Fnv1a::new();
            hash.string("omega.uefi-get-memory-map-acquisition.v1");
            hash.u64(provider.physical_invocation().normalized_identity());
            hash.u64(provider.boot_services_occurrence().normalized_identity());
            hash.u64(execution.invocation.buffer.map.as_ptr() as usize as u64);
            hash.u64(execution.map_key as u64);
            hash.u64(execution.map_size as u64);
            hash.u64(execution.descriptor_size as u64);
            hash.u64(u64::from(execution.descriptor_version));
            let snapshot = UefiMemoryMapSnapshotId::from_normalized_identity(hash.finish() | 1)
                .expect("snapshot identity forced nonzero");
            let acquisition = UefiMemoryMapAcquisition {
                session: provider.firmware_session(),
                invocation: provider.physical_invocation(),
                snapshot,
                map_key,
                map_bytes: execution.map_size,
                descriptor_size: execution.descriptor_size,
                descriptor_version: execution.descriptor_version,
            };
            let ExecutedUefiGetMemoryMapInvocation { invocation, .. } = execution;
            invocation.buffer.occupied = acquisition.map_bytes;
            let BoundUefiGetMemoryMapInvocation {
                invocation, buffer, ..
            } = invocation;
            Ok(UefiGetMemoryMapAttemptOutcome::Acquired {
                invocation,
                buffer,
                acquisition,
            })
        }
        UefiGetMemoryMapAttemptStatus::BufferTooSmall => {
            if execution.map_size <= execution.invocation.buffer.capacity() {
                return reject(
                    execution,
                    "UEFI GetMemoryMap BufferTooSmall did not report a growth requirement",
                );
            }
            let required_map_bytes = execution.map_size;
            let ExecutedUefiGetMemoryMapInvocation { invocation, .. } = execution;
            let BoundUefiGetMemoryMapInvocation {
                invocation, buffer, ..
            } = invocation;
            Ok(UefiGetMemoryMapAttemptOutcome::BufferTooSmall {
                invocation,
                buffer,
                required_map_bytes,
            })
        }
        UefiGetMemoryMapAttemptStatus::Rejected => reject(
            execution,
            "UEFI GetMemoryMap returned a rejecting status from its closed target table",
        ),
        UefiGetMemoryMapAttemptStatus::Unknown => reject(
            execution,
            "UEFI GetMemoryMap returned a status outside its closed target table",
        ),
    }
}

#[cfg(test)]
impl UefiMemoryMapAcquisition {
    /// Test-only minting inside the module family: unit tests for the ledger
    /// and the exit edge need already-executed acquisition evidence without
    /// standing up a fabricated Boot Services table. Production code has no
    /// route here; `pub(super)` keeps it inside `uefi_bootstrap`.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::platform_bringup::uefi_bootstrap) fn for_test(
        session: UefiFirmwareSessionId,
        invocation: UefiPhysicalInvocationId,
        snapshot: UefiMemoryMapSnapshotId,
        map_key: UefiMemoryMapKeyId,
        map_bytes: usize,
        descriptor_size: usize,
        descriptor_version: u32,
    ) -> Self {
        Self {
            session,
            invocation,
            snapshot,
            map_key,
            map_bytes,
            descriptor_size,
            descriptor_version,
        }
    }
}
