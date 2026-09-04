//! Lifecycle-scoped UEFI `HandleProtocol` correspondence.
//!
//! This rung joins the System Table's private BootServices pointer to one
//! exact, CRC-validated `EFI_BOOT_SERVICES` occurrence and its target-owned
//! `HandleProtocol` row. The exact physical handle, Loaded Image GUID, service
//! address, and one zeroed interface-output slot are sealed as concrete call
//! operands. A target-runtime-only execution edge invokes that exact function
//! and retains its status and output before the target-owned Loaded Image
//! layout may derive image geometry. No raw pointer is publicly exposed, and
//! no `Extent`, semantic root, shell, adapter, or installation is created.

use std::num::NonZeroU64;

use omega_calling_conventions::MachineRegister;
use omega_program_entry_plan::{
    UefiHandleProtocolInvocationPlan, plan_uefi_handle_protocol_invocation,
};
use omega_target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, ValidatedUefiBootServicesHeaderIntegrity,
    ValidatedUefiLoadedImageGeometry, plan_uefi_boot_services_native_layout,
};
pub use omega_target::{UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiProtocolGuid};

use super::{
    LifecycleScopedUefiBootServicesProjection, ReleasedUefiSystemTableScope,
    UefiApplicationFirmwareLedger,
};
use crate::{
    ExternalRootDiagnostic, UefiBootServicesTableOccurrenceId, UefiImageHandleOccurrenceId,
    UefiPhysicalInvocationId,
};

const HANDLE_PROTOCOL_FIELD_ORDINAL: u8 = 21;
const HANDLE_PROTOCOL_FIELD_OFFSET: u32 = 152;
const HANDLE_PROTOCOL_FIELD_SIZE: u32 = 8;
const HANDLE_PROTOCOL_FIELD_ALIGNMENT: u32 = 8;

mod execution;
pub use execution::*;

/// Exact Boot Services occurrence and private `HandleProtocol` slot retained
/// beneath the physical-arrival lease. The carrier is non-clone and exposes
/// report coordinates only; the service function address remains private.
#[must_use = "UEFI HandleProtocol provider retains physical-arrival and phase custody"]
pub struct LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services> {
    projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    occurrence: UefiBootServicesTableOccurrenceId,
    _table_address: NonZeroU64,
    field: UefiBootServicesNativeFieldLayout,
    handle_protocol: NonZeroU64,
}

impl std::fmt::Debug for LifecycleScopedUefiHandleProtocolProvider<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleScopedUefiHandleProtocolProvider")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("boot_services_occurrence", &self.boot_services_occurrence())
            .field("field_ordinal", &self.field_ordinal())
            .field("field_byte_offset", &self.field_byte_offset())
            .finish_non_exhaustive()
    }
}

impl LifecycleScopedUefiHandleProtocolProvider<'_, '_> {
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
    pub const fn protocol(&self) -> UefiProtocolGuid {
        UEFI_LOADED_IMAGE_PROTOCOL_GUID
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
#[must_use = "UEFI HandleProtocol provider rejection retains every composition input"]
pub struct UefiHandleProtocolProviderJoinError<'system_table, 'boot_services> {
    projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    occurrence: UefiBootServicesTableOccurrenceId,
    table_address: NonZeroU64,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services>
    UefiHandleProtocolProviderJoinError<'system_table, 'boot_services>
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
impl std::fmt::Display for UefiHandleProtocolProviderJoinError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiHandleProtocolProviderJoinError<'_, '_> {}

/// Join an admitted Boot Services occurrence to the exact private pointer
/// projected from the physical System Table. `table_address` is an admitted
/// correspondence premise; layout, header integrity, and service geometry are
/// independently replayed before the provider carrier is formed.
pub fn join_lifecycle_scoped_uefi_handle_protocol_provider<'system_table, 'boot_services>(
    ledger: &UefiApplicationFirmwareLedger<'system_table>,
    projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    occurrence: UefiBootServicesTableOccurrenceId,
    table_address: NonZeroU64,
) -> Result<
    LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    Box<UefiHandleProtocolProviderJoinError<'system_table, 'boot_services>>,
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
            "UEFI HandleProtocol provider belongs to a different or inactive physical invocation",
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
            "UEFI HandleProtocol provider does not retain the exact Boot Services layout",
        );
    }
    let Some(field) = expected.field_layout(UefiBootServicesNativeField::HandleProtocol) else {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI Boot Services layout has no HandleProtocol row",
        );
    };
    if (
        field.ordinal(),
        field.byte_offset(),
        field.byte_size(),
        field.alignment(),
        field.kind(),
    ) != (
        HANDLE_PROTOCOL_FIELD_ORDINAL,
        HANDLE_PROTOCOL_FIELD_OFFSET,
        HANDLE_PROTOCOL_FIELD_SIZE,
        HANDLE_PROTOCOL_FIELD_ALIGNMENT,
        UefiBootServicesNativeFieldKind::FunctionPointer,
    ) {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI HandleProtocol row drifted from exact target geometry",
        );
    }
    let start = field.byte_offset() as usize;
    let bytes = &integrity.table_bytes()[start..start + field.byte_size() as usize];
    let value = u64::from_le_bytes(bytes.try_into().expect("HandleProtocol width replayed"));
    let Some(handle_protocol) = NonZeroU64::new(value) else {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI HandleProtocol service pointer is null during the Boot-Services-live phase",
        );
    };
    Ok(LifecycleScopedUefiHandleProtocolProvider {
        projection,
        integrity,
        occurrence,
        _table_address: table_address,
        field,
        handle_protocol,
    })
}

fn reject_join<'system_table, 'boot_services>(
    projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    occurrence: UefiBootServicesTableOccurrenceId,
    table_address: NonZeroU64,
    message: impl Into<String>,
) -> Result<
    LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    Box<UefiHandleProtocolProviderJoinError<'system_table, 'boot_services>>,
> {
    Err(Box::new(UefiHandleProtocolProviderJoinError {
        projection,
        integrity,
        occurrence,
        table_address,
        diagnostic: ExternalRootDiagnostic(message.into()),
    }))
}

/// One exact address-free outbound invocation plan joined to the live provider
/// carrier. This remains planning custody only: it contains no argument
/// pointer values, output slot, call edge, bytes, or execution evidence.
#[must_use = "planned UEFI HandleProtocol invocation retains provider and physical custody"]
pub struct PlannedUefiHandleProtocolInvocation<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    plan: UefiHandleProtocolInvocationPlan,
}

impl std::fmt::Debug for PlannedUefiHandleProtocolInvocation<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlannedUefiHandleProtocolInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("service_identity", &self.service_identity())
            .field(
                "calling_plan_report_fingerprint",
                &self.calling_plan_report_fingerprint(),
            )
            .finish_non_exhaustive()
    }
}

impl PlannedUefiHandleProtocolInvocation<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.provider.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.provider.image_handle_occurrence()
    }
    pub const fn service_identity(&self) -> &'static str {
        self.plan.service_identity()
    }
    pub const fn protocol(&self) -> UefiProtocolGuid {
        self.plan.protocol()
    }
    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.plan.calling_plan_report_fingerprint()
    }
    pub const fn plan(&self) -> &UefiHandleProtocolInvocationPlan {
        &self.plan
    }
}

#[derive(Debug)]
#[must_use = "UEFI HandleProtocol planning rejection retains provider custody"]
pub struct UefiHandleProtocolInvocationPlanningError<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}
impl<'system_table, 'boot_services>
    UefiHandleProtocolInvocationPlanningError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.provider, self.diagnostic)
    }
}
impl std::fmt::Display for UefiHandleProtocolInvocationPlanningError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiHandleProtocolInvocationPlanningError<'_, '_> {}

/// Consume the live provider into the exact target-authored HandleProtocol
/// call shape. The plan fixes RCX/RDX/R8 inputs, RAX status, shadow space,
/// clobbers, Loaded Image GUID, and the closed status table without performing
/// the firmware call.
pub fn prepare_uefi_loaded_image_handle_protocol_invocation<'system_table, 'boot_services>(
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
) -> Result<
    PlannedUefiHandleProtocolInvocation<'system_table, 'boot_services>,
    Box<UefiHandleProtocolInvocationPlanningError<'system_table, 'boot_services>>,
> {
    let plan = match plan_uefi_handle_protocol_invocation(TargetProfile::UefiX64) {
        Ok(plan) => plan,
        Err(error) => {
            return Err(Box::new(UefiHandleProtocolInvocationPlanningError {
                provider,
                diagnostic: ExternalRootDiagnostic(format!(
                    "UEFI HandleProtocol invocation plan rejected: {}",
                    error.diagnostic()
                )),
            }));
        }
    };
    if !plan.matches_exact_uefi_x64_plan()
        || plan.service_field() != provider.field
        || plan.protocol() != UEFI_LOADED_IMAGE_PROTOCOL_GUID
    {
        return Err(Box::new(UefiHandleProtocolInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI HandleProtocol invocation plan drifted from its lifecycle provider".into(),
            ),
        }));
    }
    Ok(PlannedUefiHandleProtocolInvocation { provider, plan })
}

/// Concrete, still-uninvoked operands for one exact Loaded Image
/// `HandleProtocol` query.
///
/// The carrier keeps the physical image handle, service function, GUID
/// address, and mutable interface-output slot private. Public observations are
/// limited to identities and ABI destinations, so neither firmware pointer can
/// be reinterpreted as storage authority. The output slot remains borrowed
/// until the provider outcome is admitted or the carrier is released.
#[must_use = "bound UEFI HandleProtocol operands retain provider and output-slot custody"]
pub struct BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output> {
    invocation: PlannedUefiHandleProtocolInvocation<'system_table, 'boot_services>,
    _handle: NonZeroU64,
    _service: NonZeroU64,
    protocol: &'static UefiProtocolGuid,
    interface_output: &'output mut UefiHandleProtocolInterfaceOutputSlot,
}

impl std::fmt::Debug for BoundUefiHandleProtocolInvocation<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundUefiHandleProtocolInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("service_identity", &self.service_identity())
            .field("protocol", &self.protocol())
            .field("argument_destinations", &self.argument_destinations())
            .finish_non_exhaustive()
    }
}

impl BoundUefiHandleProtocolInvocation<'_, '_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }

    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }

    pub const fn service_identity(&self) -> &'static str {
        self.invocation.service_identity()
    }

    pub const fn protocol(&self) -> UefiProtocolGuid {
        *self.protocol
    }

    pub const fn argument_destinations(&self) -> [MachineRegister; 3] {
        [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
        ]
    }

    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.invocation.calling_plan_report_fingerprint()
    }
}

#[derive(Debug)]
#[must_use = "UEFI HandleProtocol operand rejection retains provider and output-slot custody"]
pub struct UefiHandleProtocolInvocationBindingError<'system_table, 'boot_services, 'output> {
    invocation: PlannedUefiHandleProtocolInvocation<'system_table, 'boot_services>,
    interface_output: &'output mut UefiHandleProtocolInterfaceOutputSlot,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services, 'output>
    UefiHandleProtocolInvocationBindingError<'system_table, 'boot_services, 'output>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PlannedUefiHandleProtocolInvocation<'system_table, 'boot_services>,
        &'output mut UefiHandleProtocolInterfaceOutputSlot,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.interface_output, self.diagnostic)
    }
}

impl std::fmt::Display for UefiHandleProtocolInvocationBindingError<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiHandleProtocolInvocationBindingError<'_, '_, '_> {}

/// Bind the exact physical image-handle value and one fresh interface-output
/// slot to the retained RCX/RDX/R8 plan. Address-free test/planning provenance
/// cannot cross this edge, and a slot containing stale output rejects without
/// consuming either input.
pub fn bind_uefi_loaded_image_handle_protocol_invocation<'system_table, 'boot_services, 'output>(
    invocation: PlannedUefiHandleProtocolInvocation<'system_table, 'boot_services>,
    interface_output: &'output mut UefiHandleProtocolInterfaceOutputSlot,
) -> Result<
    BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    Box<UefiHandleProtocolInvocationBindingError<'system_table, 'boot_services, 'output>>,
> {
    let reject = |invocation, interface_output, message: &'static str| {
        Err(Box::new(UefiHandleProtocolInvocationBindingError {
            invocation,
            interface_output,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };
    if !invocation.plan.matches_exact_uefi_x64_plan()
        || invocation.plan.service_field() != invocation.provider.field
        || invocation.plan.protocol() != UEFI_LOADED_IMAGE_PROTOCOL_GUID
    {
        return reject(
            invocation,
            interface_output,
            "UEFI HandleProtocol operand binding plan drifted from its lifecycle provider",
        );
    }
    let Some(handle) = invocation
        .provider
        .projection
        .readiness
        .arrival
        .image_handle
        .opaque_handle
    else {
        return reject(
            invocation,
            interface_output,
            "UEFI HandleProtocol operand binding requires the exact physical image-handle value",
        );
    };
    if !interface_output.is_empty() {
        return reject(
            invocation,
            interface_output,
            "UEFI HandleProtocol interface-output slot must be zero before invocation",
        );
    }
    if std::mem::size_of::<UefiProtocolGuid>() != 16
        || std::mem::align_of::<UefiProtocolGuid>() != 4
    {
        return reject(
            invocation,
            interface_output,
            "UEFI HandleProtocol GUID carrier does not have the exact native layout",
        );
    }
    let service = invocation.provider.handle_protocol;
    Ok(BoundUefiHandleProtocolInvocation {
        invocation,
        _handle: handle,
        _service: service,
        protocol: &UEFI_LOADED_IMAGE_PROTOCOL_GUID,
        interface_output,
    })
}

/// Non-root correspondence established only from one executed successful
/// provider call for the exact physical image handle and Loaded Image GUID.
#[must_use = "Loaded Image correspondence retains HandleProtocol provider and physical custody"]
pub struct LifecycleScopedUefiLoadedImageCorrespondence<'system_table, 'boot_services, 'output> {
    execution: ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    geometry: ValidatedUefiLoadedImageGeometry,
}

impl std::fmt::Debug for LifecycleScopedUefiLoadedImageCorrespondence<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleScopedUefiLoadedImageCorrespondence")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("image_base", &self.image_base())
            .field("image_size", &self.image_size())
            .finish_non_exhaustive()
    }
}

impl LifecycleScopedUefiLoadedImageCorrespondence<'_, '_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.execution.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.execution.image_handle_occurrence()
    }
    pub const fn protocol(&self) -> UefiProtocolGuid {
        UEFI_LOADED_IMAGE_PROTOCOL_GUID
    }
    pub const fn image_base(&self) -> u64 {
        self.geometry.image_base()
    }
    pub const fn image_size(&self) -> u64 {
        self.geometry.image_size()
    }
    pub const fn image_end_exclusive(&self) -> u64 {
        self.geometry.image_end_exclusive()
    }
}

#[derive(Debug)]
#[must_use = "UEFI provider release rejection retains complete provider custody"]
pub struct UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}
impl<'system_table, 'boot_services>
    UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.provider, self.diagnostic)
    }
}
impl std::fmt::Display for UefiHandleProtocolProviderReleaseError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiHandleProtocolProviderReleaseError<'_, '_> {}

impl<'system_table> UefiApplicationFirmwareLedger<'system_table> {
    pub fn release_lifecycle_scoped_handle_protocol_provider<'boot_services>(
        &mut self,
        provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services>>,
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
            return Err(Box::new(UefiHandleProtocolProviderReleaseError {
                provider,
                diagnostic: ExternalRootDiagnostic(
                    "UEFI HandleProtocol provider belongs to a different firmware ledger".into(),
                ),
            }));
        }
        Ok(self
            .release_lifecycle_scoped_boot_services_projection(provider.projection)
            .expect("provider ownership replayed before delegating release"))
    }

    pub fn release_lifecycle_scoped_loaded_image_correspondence<'boot_services, 'output>(
        &mut self,
        correspondence: LifecycleScopedUefiLoadedImageCorrespondence<
            'system_table,
            'boot_services,
            'output,
        >,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_executed_uefi_handle_protocol_invocation(correspondence.execution)
    }

    pub fn release_executed_uefi_handle_protocol_invocation<'boot_services, 'output>(
        &mut self,
        execution: ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_bound_uefi_handle_protocol_invocation(execution.invocation)
    }

    pub fn release_bound_uefi_handle_protocol_invocation<'boot_services, 'output>(
        &mut self,
        invocation: BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_planned_uefi_handle_protocol_invocation(invocation.invocation)
    }

    pub fn release_planned_uefi_handle_protocol_invocation<'boot_services>(
        &mut self,
        invocation: PlannedUefiHandleProtocolInvocation<'system_table, 'boot_services>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_lifecycle_scoped_handle_protocol_provider(invocation.provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId, UefiFirmwareSessionId,
        UefiSystemTableOccurrenceId, join_lifecycle_scoped_uefi_system_table,
        join_uefi_application_physical_arrival,
        prepare_uefi_application_bootstrap_adapter_invocation,
        project_uefi_application_boot_services,
    };
    use omega_program_entry_plan::{
        ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
        UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, UefiHandleProtocolStatus,
        exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
    };
    use omega_target::{
        ProgramEntryPhysicalContractPackage, UEFI_BOOT_SERVICES_SIGNATURE,
        UEFI_LOADED_IMAGE_PROTOCOL_REVISION, UEFI_SYSTEM_TABLE_SIGNATURE,
        plan_uefi_system_table_native_layout, validate_uefi_boot_services_occurrence,
        validate_uefi_system_table_occurrence,
    };
    use std::ffi::c_void;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static FAKE_STATUS: AtomicU64 = AtomicU64::new(0);
    static FAKE_INTERFACE: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_HANDLE: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_PROTOCOL_DATA1: AtomicU64 = AtomicU64::new(0);
    static OBSERVED_OUTPUT_SLOT: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "efiapi" fn fake_handle_protocol(
        handle: *mut c_void,
        protocol: *const UefiProtocolGuid,
        interface: *mut *mut c_void,
    ) -> u64 {
        OBSERVED_HANDLE.store(handle as usize, Ordering::SeqCst);
        OBSERVED_OUTPUT_SLOT.store(interface as usize, Ordering::SeqCst);
        // SAFETY: execution tests invoke the fake only with the exact static
        // GUID pointer retained by the bound carrier.
        let data1 = unsafe { (*protocol).data1 };
        OBSERVED_PROTOCOL_DATA1.store(u64::from(data1), Ordering::SeqCst);
        let output = FAKE_INTERFACE.load(Ordering::SeqCst) as *mut c_void;
        // SAFETY: the bound carrier passes its live opaque output slot.
        unsafe { *interface = output };
        FAKE_STATUS.load(Ordering::SeqCst)
    }

    #[repr(align(8))]
    struct LoadedImageBytes([u8; 96]);

    #[repr(align(8))]
    struct MisalignedLoadedImageBytes([u8; 97]);

    fn loaded_image_bytes(image_base: u64, image_size: u64) -> Box<LoadedImageBytes> {
        let mut bytes = Box::new(LoadedImageBytes([0; 96]));
        bytes.0[0..4].copy_from_slice(&UEFI_LOADED_IMAGE_PROTOCOL_REVISION.to_le_bytes());
        bytes.0[64..72].copy_from_slice(&image_base.to_le_bytes());
        bytes.0[72..80].copy_from_slice(&image_size.to_le_bytes());
        bytes
    }

    fn fake_handle_protocol_address() -> u64 {
        fake_handle_protocol as *const () as usize as u64
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
    fn projection<'a>(
        ledger: &mut UefiApplicationFirmwareLedger<'a>,
        bytes: &'a [u8],
        base: u64,
    ) -> LifecycleScopedUefiBootServicesProjection<'a> {
        projection_with_image_handle(ledger, bytes, base, true)
    }
    fn projection_with_image_handle<'a>(
        ledger: &mut UefiApplicationFirmwareLedger<'a>,
        bytes: &'a [u8],
        base: u64,
        retain_physical_value: bool,
    ) -> LifecycleScopedUefiBootServicesProjection<'a> {
        let occurrence = id(base, UefiImageHandleOccurrenceId::from_normalized_identity);
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

    fn bound_invocation<'system_table, 'boot_services, 'output>(
        ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
        system: &'system_table [u8],
        boot: &'boot_services [u8],
        base: u64,
        boot_address: u64,
        interface_output: &'output mut UefiHandleProtocolInterfaceOutputSlot,
    ) -> BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output> {
        let projection = projection(ledger, system, base);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            boot,
        )
        .unwrap();
        let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
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
        let invocation = prepare_uefi_loaded_image_handle_protocol_invocation(provider).unwrap();
        bind_uefi_loaded_image_handle_protocol_invocation(invocation, interface_output).unwrap()
    }

    #[test]
    fn exact_handle_protocol_success_establishes_non_root_loaded_image_correspondence() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            152,
            fake_handle_protocol_address(),
        );
        let loaded_image = loaded_image_bytes(0x100000, 0x20000);
        FAKE_STATUS.store(0, Ordering::SeqCst);
        FAKE_INTERFACE.store(loaded_image.0.as_ptr() as usize, Ordering::SeqCst);
        let mut ledger = ledger(10);
        let mut interface_output = UefiHandleProtocolInterfaceOutputSlot::empty();
        let output_slot_address = (&raw mut interface_output.0) as usize;
        let invocation = bound_invocation(
            &mut ledger,
            &system,
            &boot,
            20,
            boot_address,
            &mut interface_output,
        );
        assert_eq!(
            invocation.argument_destinations(),
            [
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R8,
            ]
        );
        assert_eq!(invocation._handle.get(), 0x1000_0014);
        assert_eq!(invocation._service.get(), fake_handle_protocol_address());
        assert!(std::ptr::eq(
            invocation.protocol,
            &UEFI_LOADED_IMAGE_PROTOCOL_GUID
        ));
        // SAFETY: the fake service and retained handle satisfy the executor's
        // test contract, and `loaded_image` remains live through admission.
        let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
        assert_eq!(
            execution.status(),
            UefiHandleProtocolExecutionStatus::Success
        );
        assert_eq!(execution.status_code(), 0);
        assert!(!execution.interface_output_is_null());
        assert_eq!(OBSERVED_HANDLE.load(Ordering::SeqCst), 0x1000_0014);
        assert_eq!(
            OBSERVED_PROTOCOL_DATA1.load(Ordering::SeqCst),
            u64::from(UEFI_LOADED_IMAGE_PROTOCOL_GUID.data1)
        );
        assert_eq!(
            OBSERVED_OUTPUT_SLOT.load(Ordering::SeqCst),
            output_slot_address
        );
        let loaded = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap();
        assert_eq!(
            (
                loaded.image_base(),
                loaded.image_size(),
                loaded.image_end_exclusive()
            ),
            (0x100000, 0x20000, 0x120000)
        );
        ledger
            .release_lifecycle_scoped_loaded_image_correspondence(loaded)
            .unwrap();
        ledger.begin_firmware_return().unwrap();
    }

    #[test]
    fn concrete_operand_binding_rejects_address_free_stale_and_mismatched_inputs() {
        let boot_address = 0x411000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x412000);
        let mut address_free_ledger = ledger(130);
        let address_free_projection =
            projection_with_image_handle(&mut address_free_ledger, &system, 140, false);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            &boot,
        )
        .unwrap();
        let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
            &address_free_ledger,
            address_free_projection,
            integrity,
            id(
                143,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap();
        let invocation = prepare_uefi_loaded_image_handle_protocol_invocation(provider).unwrap();
        let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
        let error =
            bind_uefi_loaded_image_handle_protocol_invocation(invocation, &mut output).unwrap_err();
        assert!(error.diagnostic().0.contains("physical image-handle value"));
        let (invocation, _, _) = error.into_parts();
        address_free_ledger
            .release_planned_uefi_handle_protocol_invocation(invocation)
            .unwrap();

        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x412000);
        let mut ledger = ledger(160);
        let projection = projection(&mut ledger, &system, 170);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            &boot,
        )
        .unwrap();
        let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
            &ledger,
            projection,
            integrity,
            id(
                173,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap();
        let invocation = prepare_uefi_loaded_image_handle_protocol_invocation(provider).unwrap();
        let mut stale_output = UefiHandleProtocolInterfaceOutputSlot(7_usize as *mut c_void);
        let error =
            bind_uefi_loaded_image_handle_protocol_invocation(invocation, &mut stale_output)
                .unwrap_err();
        assert!(error.diagnostic().0.contains("must be zero"));
        let (invocation, output, _) = error.into_parts();
        output.0 = std::ptr::null_mut();
        let invocation =
            bind_uefi_loaded_image_handle_protocol_invocation(invocation, output).unwrap();
        ledger
            .release_bound_uefi_handle_protocol_invocation(invocation)
            .unwrap();
    }

    #[test]
    fn address_mismatch_and_null_service_reject_with_complete_custody() {
        let boot_address = 0x501000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x502000);
        let mut owner = ledger(40);
        let projected = projection(&mut owner, &system, 50);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            &boot,
        )
        .unwrap();
        let error = join_lifecycle_scoped_uefi_handle_protocol_provider(
            &owner,
            projected,
            integrity,
            id(
                53,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address + 8).unwrap(),
        )
        .unwrap_err();
        let (projected, integrity, occurrence, _, _) = error.into_parts();
        let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
            &owner,
            projected,
            integrity,
            occurrence,
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap();
        owner
            .release_lifecycle_scoped_handle_protocol_provider(provider)
            .unwrap();

        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0);
        let mut ledger = ledger(70);
        let projection = projection(&mut ledger, &system, 80);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            &boot,
        )
        .unwrap();
        let error = join_lifecycle_scoped_uefi_handle_protocol_provider(
            &ledger,
            projection,
            integrity,
            id(
                83,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap_err();
        assert!(error.diagnostic().0.contains("pointer is null"));
        let (projection, _, _, _, _) = error.into_parts();
        ledger
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap();
    }

    #[test]
    fn closed_and_unknown_statuses_return_complete_execution_custody() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let statuses = [
            (
                UefiHandleProtocolStatus::InvalidParameter,
                UefiHandleProtocolExecutionStatus::InvalidParameter,
            ),
            (
                UefiHandleProtocolStatus::Unsupported,
                UefiHandleProtocolExecutionStatus::Unsupported,
            ),
        ];
        for (index, (planned, expected)) in statuses.into_iter().enumerate() {
            let boot_address = 0x601000 + (index as u64 * 0x1000);
            let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
            let boot = table(
                UEFI_BOOT_SERVICES_SIGNATURE,
                376,
                152,
                fake_handle_protocol_address(),
            );
            let mut ledger = ledger(300 + index as u64 * 20);
            let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
            let retained_error_output = loaded_image_bytes(0x100000, 0x20000);
            let invocation = bound_invocation(
                &mut ledger,
                &system,
                &boot,
                310 + index as u64 * 20,
                boot_address,
                &mut output,
            );
            let code = invocation.invocation.plan.status_code(planned);
            FAKE_STATUS.store(code, Ordering::SeqCst);
            FAKE_INTERFACE.store(retained_error_output.0.as_ptr() as usize, Ordering::SeqCst);
            // SAFETY: the exact test fake is the retained service and performs
            // no output dereference beyond the bound slot.
            let execution =
                unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
            let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
            assert_eq!(error.status(), expected);
            assert_eq!(error.status_code(), code);
            let (execution, _) = error.into_parts();
            assert!(!execution.interface_output_is_null());
            ledger
                .release_executed_uefi_handle_protocol_invocation(execution)
                .unwrap();
        }

        let boot_address = 0x604000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            152,
            fake_handle_protocol_address(),
        );
        let mut unknown_status_ledger = ledger(350);
        let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
        let invocation = bound_invocation(
            &mut unknown_status_ledger,
            &system,
            &boot,
            360,
            boot_address,
            &mut output,
        );
        FAKE_STATUS.store(0x1234, Ordering::SeqCst);
        FAKE_INTERFACE.store(0, Ordering::SeqCst);
        // SAFETY: the exact test fake is the retained service.
        let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
        let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
        assert_eq!(error.status(), UefiHandleProtocolExecutionStatus::Unknown);
        assert_eq!(error.status_code(), 0x1234);
        let (execution, _) = error.into_parts();
        unknown_status_ledger
            .release_executed_uefi_handle_protocol_invocation(execution)
            .unwrap();

        let boot_address = 0x706000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            152,
            fake_handle_protocol_address(),
        );
        let misaligned = MisalignedLoadedImageBytes([0; 97]);
        let mut alignment_ledger = ledger(500);
        let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
        let invocation = bound_invocation(
            &mut alignment_ledger,
            &system,
            &boot,
            510,
            boot_address,
            &mut output,
        );
        FAKE_STATUS.store(0, Ordering::SeqCst);
        FAKE_INTERFACE.store(
            // SAFETY: the 97-byte allocation leaves a readable 96-byte suffix.
            unsafe { misaligned.0.as_ptr().add(1) } as usize,
            Ordering::SeqCst,
        );
        // SAFETY: the deliberately misaligned interface remains readable for
        // the required 96-byte prefix and is rejected before target decoding.
        let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
        let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
        assert!(error.diagnostic().0.contains("not aligned"));
        let (execution, _) = error.into_parts();
        alignment_ledger
            .release_executed_uefi_handle_protocol_invocation(execution)
            .unwrap();
    }

    #[test]
    fn null_stale_and_malformed_outputs_cannot_establish_correspondence() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (index, loaded_image) in [
            None,
            Some(loaded_image_bytes(0, 0x20000)),
            Some(loaded_image_bytes(u64::MAX - 1, 2)),
        ]
        .into_iter()
        .enumerate()
        {
            let boot_address = 0x701000 + index as u64 * 0x1000;
            let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
            let boot = table(
                UEFI_BOOT_SERVICES_SIGNATURE,
                376,
                152,
                fake_handle_protocol_address(),
            );
            let mut ledger = ledger(400 + index as u64 * 20);
            let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
            let invocation = bound_invocation(
                &mut ledger,
                &system,
                &boot,
                410 + index as u64 * 20,
                boot_address,
                &mut output,
            );
            FAKE_STATUS.store(0, Ordering::SeqCst);
            FAKE_INTERFACE.store(
                loaded_image
                    .as_ref()
                    .map_or(0, |bytes| bytes.0.as_ptr() as usize),
                Ordering::SeqCst,
            );
            // SAFETY: non-null test buffers retain the exact readable prefix
            // for the duration of receipt admission.
            let execution =
                unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
            if index == 1 {
                execution.invocation.interface_output.0 = std::ptr::null_mut();
            }
            let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
            let diagnostic = &error.diagnostic().0;
            match index {
                0 => assert!(diagnostic.contains("null Loaded Image interface")),
                1 => assert!(diagnostic.contains("output slot drifted")),
                2 => assert!(diagnostic.contains("wraps")),
                _ => unreachable!(),
            }
            let (execution, _) = error.into_parts();
            ledger
                .release_executed_uefi_handle_protocol_invocation(execution)
                .unwrap();
        }

        let boot_address = 0x705000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            152,
            fake_handle_protocol_address(),
        );
        let mut bad_revision = loaded_image_bytes(0x100000, 0x20000);
        bad_revision.0[0..4].copy_from_slice(&0_u32.to_le_bytes());
        let mut ledger = ledger(470);
        let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
        let invocation =
            bound_invocation(&mut ledger, &system, &boot, 480, boot_address, &mut output);
        FAKE_STATUS.store(0, Ordering::SeqCst);
        FAKE_INTERFACE.store(bad_revision.0.as_ptr() as usize, Ordering::SeqCst);
        // SAFETY: `bad_revision` retains a readable exact-size layout buffer.
        let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
        let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
        assert!(error.diagnostic().0.contains("revision"));
        let (execution, _) = error.into_parts();
        ledger
            .release_executed_uefi_handle_protocol_invocation(execution)
            .unwrap();
    }

    #[test]
    fn provider_public_surface_has_no_raw_function_or_authority_projection() {
        let source = include_str!("handle_protocol_provider.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let compact: String = source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for forbidden in [
            "pubfnhandle_protocol",
            "pubfnfunction",
            "pubfnbytes",
            "psi_extents::Extent",
            "NativeExecution",
            "PhysicalShell",
            "UefiHandleProtocolLoadedImageOutcome",
            "admit_uefi_loaded_image_handle_protocol_outcome",
            "pubfninterface_address",
            "pubfnservice_address",
            "pubfnhandle_address",
        ] {
            assert!(
                !compact.contains(forbidden),
                "forbidden provider API appeared: {forbidden}"
            );
        }
        assert!(!compact.contains("implCloneforLifecycleScopedUefiHandleProtocolProvider"));
        assert!(!compact.contains("implCloneforLifecycleScopedUefiLoadedImageCorrespondence"));
        assert!(!compact.contains("implCloneforPlannedUefiHandleProtocolInvocation"));
        assert!(!compact.contains("implCloneforBoundUefiHandleProtocolInvocation"));
        let execution_source = include_str!("handle_protocol_provider/execution.rs");
        let execution_compact: String = execution_source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        assert!(!execution_compact.contains("implCloneforUefiHandleProtocolInterfaceOutputSlot"));
        assert!(!execution_compact.contains("implCloneforExecutedUefiHandleProtocolInvocation"));
        assert!(execution_compact.contains(
            "admit_uefi_loaded_image_handle_protocol_execution<'system_table,'boot_services,'output>(execution:ExecutedUefiHandleProtocolInvocation"
        ));
    }
}
