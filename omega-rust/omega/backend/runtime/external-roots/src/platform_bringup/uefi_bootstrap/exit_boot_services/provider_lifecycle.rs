//! The lifecycle-scoped exit-boot-services provider, its join and release.

use crate::platform_bringup::uefi_bootstrap::exit_boot_services::{
    BoundUefiExitBootServicesInvocation, ExecutedUefiExitBootServicesInvocation,
    PlannedUefiExitBootServicesInvocation,
};
use crate::platform_bringup::uefi_bootstrap::{
    LifecycleScopedUefiBootServicesProjection, ReleasedUefiSystemTableScope,
    UefiApplicationFirmwareLedger,
};
use crate::{
    ExternalRootDiagnostic, UefiBootServicesTableOccurrenceId, UefiImageHandleOccurrenceId,
    UefiPhysicalInvocationId,
};
use program_entry_plan::{
    UEFI_EXIT_BOOT_SERVICES_SERVICE_IDENTITY, plan_uefi_os_handoff_invocation,
};
use std::num::NonZeroU64;
use target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldLayout,
    ValidatedUefiBootServicesHeaderIntegrity, plan_uefi_boot_services_native_layout,
};

/// Exact Boot Services occurrence and private `ExitBootServices` slot retained
/// beneath the physical-arrival lease. The carrier is non-clone and exposes
/// report coordinates only; the service function address remains private.
#[must_use = "UEFI ExitBootServices provider retains physical-arrival and phase custody"]
pub struct LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services> {
    pub(crate) projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    pub(crate) integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    pub(crate) occurrence: UefiBootServicesTableOccurrenceId,
    _table_address: NonZeroU64,
    pub(crate) field: UefiBootServicesNativeFieldLayout,
    pub(crate) exit_boot_services: NonZeroU64,
}

impl std::fmt::Debug for LifecycleScopedUefiExitBootServicesProvider<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleScopedUefiExitBootServicesProvider")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("boot_services_occurrence", &self.boot_services_occurrence())
            .field("field_ordinal", &self.field_ordinal())
            .field("field_byte_offset", &self.field_byte_offset())
            .finish_non_exhaustive()
    }
}

impl LifecycleScopedUefiExitBootServicesProvider<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.projection.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.projection.image_handle_occurrence()
    }
    pub const fn boot_services_occurrence(&self) -> UefiBootServicesTableOccurrenceId {
        self.occurrence
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
        UEFI_EXIT_BOOT_SERVICES_SERVICE_IDENTITY
    }
    pub const fn boot_services_revision(&self) -> u32 {
        self.integrity.revision()
    }
    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.integrity
            .layout()
            .non_authoritative_layout_report_fingerprint()
    }
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices provider rejection retains every composition input"]
pub struct UefiExitBootServicesProviderJoinError<'system_table, 'boot_services> {
    projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    occurrence: UefiBootServicesTableOccurrenceId,
    table_address: NonZeroU64,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services>
    UefiExitBootServicesProviderJoinError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiBootServicesProjection<'system_table>,
        ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
        UefiBootServicesTableOccurrenceId,
        NonZeroU64,
        ExternalRootDiagnostic,
    ) {
        (
            self.projection,
            self.integrity,
            self.occurrence,
            self.table_address,
            self.diagnostic,
        )
    }
}

impl std::fmt::Display for UefiExitBootServicesProviderJoinError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiExitBootServicesProviderJoinError<'_, '_> {}

/// Join an admitted Boot Services occurrence to the exact private pointer
/// projected from the physical System Table. `table_address` is an admitted
/// correspondence premise; layout, header integrity, and the service row the
/// planned `ExitBootServices` leg names are independently replayed before
/// the provider carrier is formed.
pub fn join_lifecycle_scoped_uefi_exit_boot_services_provider<'system_table, 'boot_services>(
    ledger: &UefiApplicationFirmwareLedger<'system_table>,
    projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    occurrence: UefiBootServicesTableOccurrenceId,
    table_address: NonZeroU64,
) -> Result<
    LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    Box<UefiExitBootServicesProviderJoinError<'system_table, 'boot_services>>,
> {
    if !ledger.matches_image_handle(&projection.readiness.arrival.image_handle)
        || !ledger.matches_provenance(&projection.readiness.arrival.system_table.provenance)
        || !ledger.matches_lease(&projection.readiness.arrival.system_table.phase_lease)
    {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI ExitBootServices provider belongs to a different or inactive physical invocation",
        );
    }
    if table_address != projection.boot_services_table {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI Boot Services occurrence address does not correspond to the System Table BootServices field",
        );
    }
    let expected = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64)
        .expect("closed UEFI x64 target must retain Boot Services layout");
    if !integrity.layout().matches_exact_plan(&expected) {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI ExitBootServices provider does not retain the exact Boot Services layout",
        );
    }
    let Some(field) = expected.field_layout(UefiBootServicesNativeField::ExitBootServices) else {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI Boot Services layout has no ExitBootServices row",
        );
    };
    let plan = match plan_uefi_os_handoff_invocation(TargetProfile::UefiX64) {
        Ok(plan) => plan,
        Err(error) => {
            return reject_join(
                projection,
                integrity,
                occurrence,
                table_address,
                format!(
                    "UEFI OS-handoff invocation plan rejected: {}",
                    error.diagnostic()
                ),
            );
        }
    };
    if field != plan.exit_boot_services().service_field() {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI ExitBootServices row drifted from the planned handoff leg",
        );
    }
    let start = field.byte_offset() as usize;
    let bytes = &integrity.table_bytes()[start..start + field.byte_size() as usize];
    let value = u64::from_le_bytes(bytes.try_into().expect("ExitBootServices width replayed"));
    let Some(exit_boot_services) = NonZeroU64::new(value) else {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI ExitBootServices service pointer is null during the Boot-Services-live phase",
        );
    };
    Ok(LifecycleScopedUefiExitBootServicesProvider {
        projection,
        integrity,
        occurrence,
        _table_address: table_address,
        field,
        exit_boot_services,
    })
}

fn reject_join<'system_table, 'boot_services>(
    projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    occurrence: UefiBootServicesTableOccurrenceId,
    table_address: NonZeroU64,
    message: impl Into<String>,
) -> Result<
    LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    Box<UefiExitBootServicesProviderJoinError<'system_table, 'boot_services>>,
> {
    Err(Box::new(UefiExitBootServicesProviderJoinError {
        projection,
        integrity,
        occurrence,
        table_address,
        diagnostic: ExternalRootDiagnostic(message.into()),
    }))
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices provider release rejection retains complete provider custody"]
pub struct UefiExitBootServicesProviderReleaseError<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services>
    UefiExitBootServicesProviderReleaseError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.provider, self.diagnostic)
    }
}

impl std::fmt::Display for UefiExitBootServicesProviderReleaseError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiExitBootServicesProviderReleaseError<'_, '_> {}

impl<'system_table> UefiApplicationFirmwareLedger<'system_table> {
    /// Release the provider back to live Boot Services custody. This is the
    /// exhaustion and pre-exit abort route: an exhausted retry loop returns
    /// its provider unchanged so the invocation can still complete a firmware
    /// return.
    pub fn release_lifecycle_scoped_exit_boot_services_provider<'boot_services>(
        &mut self,
        provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiExitBootServicesProviderReleaseError<'system_table, 'boot_services>>,
    > {
        if !self.matches_image_handle(&provider.projection.readiness.arrival.image_handle)
            || !self.matches_provenance(
                &provider
                    .projection
                    .readiness
                    .arrival
                    .system_table
                    .provenance,
            )
            || !self.matches_lease(
                &provider
                    .projection
                    .readiness
                    .arrival
                    .system_table
                    .phase_lease,
            )
        {
            return Err(Box::new(UefiExitBootServicesProviderReleaseError {
                provider,
                diagnostic: ExternalRootDiagnostic(
                    "UEFI ExitBootServices provider belongs to a different firmware ledger".into(),
                ),
            }));
        }
        Ok(self
            .release_lifecycle_scoped_boot_services_projection(provider.projection)
            .expect("provider ownership replayed before delegating release"))
    }

    pub fn release_planned_uefi_exit_boot_services_invocation<'boot_services>(
        &mut self,
        invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiExitBootServicesProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_lifecycle_scoped_exit_boot_services_provider(invocation.provider)
    }

    pub fn release_bound_uefi_exit_boot_services_invocation<'boot_services>(
        &mut self,
        invocation: BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiExitBootServicesProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_planned_uefi_exit_boot_services_invocation(invocation.invocation)
    }

    pub fn release_executed_uefi_exit_boot_services_invocation<'boot_services>(
        &mut self,
        execution: ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiExitBootServicesProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_bound_uefi_exit_boot_services_invocation(execution.invocation)
    }
}
