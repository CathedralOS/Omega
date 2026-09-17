//! The lifecycle-scoped get-memory-map provider and its join.

use crate::platform_bringup::uefi_bootstrap::{
    PlannedUefiExitBootServicesInvocation, UefiApplicationFirmwareLedger,
};
use crate::{
    ExternalRootDiagnostic, UefiBootServicesTableOccurrenceId, UefiFirmwareSessionId,
    UefiImageHandleOccurrenceId, UefiPhysicalInvocationId,
};
use program_entry_plan::{UEFI_GET_MEMORY_MAP_SERVICE_IDENTITY, plan_uefi_os_handoff_invocation};
use std::num::NonZeroU64;
use target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldLayout,
    plan_uefi_boot_services_native_layout,
};

/// Exact pending-exit borrow plus the private `GetMemoryMap` slot read from
/// the same validated Boot Services occurrence the exit provider sealed. The
/// carrier is non-clone and exposes report coordinates only; the service
/// function address and the borrowed operand cells remain private.
#[must_use = "UEFI GetMemoryMap provider borrows pending-exit invocation custody"]
pub struct LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services> {
    pub(crate) pending_exit:
        &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    pub(super) field: UefiBootServicesNativeFieldLayout,
    pub(super) get_memory_map: NonZeroU64,
}

impl std::fmt::Debug for LifecycleScopedUefiGetMemoryMapProvider<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleScopedUefiGetMemoryMapProvider")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("boot_services_occurrence", &self.boot_services_occurrence())
            .field("field_ordinal", &self.field_ordinal())
            .field("field_byte_offset", &self.field_byte_offset())
            .finish_non_exhaustive()
    }
}

impl LifecycleScopedUefiGetMemoryMapProvider<'_, '_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.pending_exit.physical_invocation()
    }
    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.pending_exit
            .provider
            .projection
            .readiness
            .arrival
            .firmware_session()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.pending_exit.image_handle_occurrence()
    }
    pub const fn boot_services_occurrence(&self) -> UefiBootServicesTableOccurrenceId {
        self.pending_exit.provider.occurrence
    }
    pub const fn field_ordinal(&self) -> u8 {
        self.field.ordinal()
    }
    pub const fn field_byte_offset(&self) -> u32 {
        self.field.byte_offset()
    }
    pub const fn field_byte_size(&self) -> u32 {
        self.field.byte_size()
    }
    pub const fn field_alignment(&self) -> u32 {
        self.field.alignment()
    }
    pub const fn service_identity(&self) -> &'static str {
        UEFI_GET_MEMORY_MAP_SERVICE_IDENTITY
    }
}

impl<'pending_exit, 'system_table, 'boot_services>
    LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>
{
    /// End acquisition custody by returning the borrowed pending-exit
    /// invocation unchanged. Every later custody level offers the same route.
    pub fn into_pending_exit_invocation(
        self,
    ) -> &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
        self.pending_exit
    }
}

/// Seal the `GetMemoryMap` row of the exact Boot Services occurrence already
/// retained beneath a pending `ExitBootServices` invocation. The pending
/// invocation must still be live under `ledger` and retain its exact planned
/// leg, and the sealed row must be the one the planned `GetMemoryMap` leg
/// names; the new provider only borrows that custody, so a stale, foreign,
/// or drifted invocation rejects without consuming anything.
pub fn join_lifecycle_scoped_uefi_get_memory_map_provider<
    'pending_exit,
    'system_table,
    'boot_services,
>(
    ledger: &UefiApplicationFirmwareLedger<'system_table>,
    invocation: &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
) -> Result<
    LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
    ExternalRootDiagnostic,
> {
    let provider = &invocation.provider;
    if !ledger.matches_image_handle(&provider.projection.readiness.arrival.image_handle)
        || !ledger.matches_provenance(
            &provider
                .projection
                .readiness
                .arrival
                .system_table
                .provenance,
        )
        || !ledger.matches_lease(
            &provider
                .projection
                .readiness
                .arrival
                .system_table
                .phase_lease,
        )
    {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap provider borrows a different or inactive physical invocation".into(),
        ));
    }
    if !invocation.retains_exact_plan() {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap provider borrows a drifted pending ExitBootServices plan".into(),
        ));
    }
    let expected = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64)
        .expect("closed UEFI x64 target must retain Boot Services layout");
    if !provider.integrity.layout().matches_exact_plan(&expected) {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap provider does not retain the exact Boot Services layout".into(),
        ));
    }
    let Some(field) = expected.field_layout(UefiBootServicesNativeField::GetMemoryMap) else {
        return Err(ExternalRootDiagnostic(
            "UEFI Boot Services layout has no GetMemoryMap row".into(),
        ));
    };
    let plan = plan_uefi_os_handoff_invocation(TargetProfile::UefiX64).map_err(|error| {
        ExternalRootDiagnostic(format!(
            "UEFI OS-handoff invocation plan rejected: {}",
            error.diagnostic()
        ))
    })?;
    if field != plan.get_memory_map().service_field() {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap row drifted from the planned handoff leg".into(),
        ));
    }
    let start = field.byte_offset() as usize;
    let bytes = &provider.integrity.table_bytes()[start..start + field.byte_size() as usize];
    let value = u64::from_le_bytes(bytes.try_into().expect("GetMemoryMap width replayed"));
    let Some(get_memory_map) = NonZeroU64::new(value) else {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap service pointer is null during the Boot-Services-live phase".into(),
        ));
    };
    Ok(LifecycleScopedUefiGetMemoryMapProvider {
        pending_exit: invocation,
        field,
        get_memory_map,
    })
}
