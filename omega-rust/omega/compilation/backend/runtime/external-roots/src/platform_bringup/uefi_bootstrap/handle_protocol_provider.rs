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

use calling_conventions::MachineRegister;
use program_entry_plan::{UefiHandleProtocolInvocationPlan, plan_uefi_handle_protocol_invocation};
use target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, ValidatedUefiBootServicesHeaderIntegrity,
    ValidatedUefiLoadedImageGeometry, plan_uefi_boot_services_native_layout,
};
pub use target::{UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiProtocolGuid};

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
pub use execution::{
    ExecutedUefiHandleProtocolInvocation, UefiHandleProtocolExecutionError,
    UefiHandleProtocolExecutionStatus, UefiHandleProtocolInterfaceOutputSlot,
    UefiHandleProtocolLoadedImageCallError, admit_uefi_loaded_image_handle_protocol_execution,
    execute_uefi_loaded_image_handle_protocol,
};

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
mod tests;
