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

use std::ffi::c_void;
use std::num::NonZeroU64;

use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl, MachineRegister,
    ValidatedBoundaryEntryPlan, ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, ValidatedUefiBootServicesHeaderIntegrity,
    plan_uefi_boot_services_native_layout,
};

use super::{
    LifecycleScopedUefiBootServicesProjection, ReleasedUefiSystemTableScope,
    UefiApplicationFirmwareLedger, UefiExitBootServicesProviderResult, UefiOsHandoffMapAcquired,
};
use crate::identities::Fnv1a;
use crate::{
    ExternalRootDiagnostic, UefiBootServicesTableOccurrenceId, UefiExitBootServicesReceiptId,
    UefiImageHandleOccurrenceId, UefiMemoryMapKeyId, UefiOsHandoffId, UefiPhysicalInvocationId,
};

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

/// Exact Boot Services occurrence and private `ExitBootServices` slot retained
/// beneath the physical-arrival lease. The carrier is non-clone and exposes
/// report coordinates only; the service function address remains private.
#[must_use = "UEFI ExitBootServices provider retains physical-arrival and phase custody"]
pub struct LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services> {
    pub(super) projection: LifecycleScopedUefiBootServicesProjection<'system_table>,
    pub(super) integrity: ValidatedUefiBootServicesHeaderIntegrity<'boot_services>,
    pub(super) occurrence: UefiBootServicesTableOccurrenceId,
    _table_address: NonZeroU64,
    field: UefiBootServicesNativeFieldLayout,
    exit_boot_services: NonZeroU64,
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
        SERVICE_IDENTITY
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
/// correspondence premise; layout, header integrity, and service geometry are
/// independently replayed before the provider carrier is formed.
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
    if (
        field.ordinal(),
        field.byte_offset(),
        field.byte_size(),
        field.alignment(),
        field.kind(),
    ) != (
        EXIT_BOOT_SERVICES_FIELD_ORDINAL,
        EXIT_BOOT_SERVICES_FIELD_OFFSET,
        EXIT_BOOT_SERVICES_FIELD_SIZE,
        EXIT_BOOT_SERVICES_FIELD_ALIGNMENT,
        UefiBootServicesNativeFieldKind::FunctionPointer,
    ) {
        return reject_join(
            projection,
            integrity,
            occurrence,
            table_address,
            "UEFI ExitBootServices row drifted from exact target geometry",
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

/// One exact two-operand invocation joined to the live provider carrier. The
/// retained plan fixes RCX/RDX inputs, RAX status, shadow space, and clobbers
/// without performing the firmware call.
#[must_use = "planned UEFI ExitBootServices invocation retains provider and physical custody"]
pub struct PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    pub(super) provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    plan: ValidatedBoundaryEntryPlan,
}

impl std::fmt::Debug for PlannedUefiExitBootServicesInvocation<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlannedUefiExitBootServicesInvocation")
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

impl PlannedUefiExitBootServicesInvocation<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.provider.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.provider.image_handle_occurrence()
    }
    pub const fn service_identity(&self) -> &'static str {
        self.provider.service_identity()
    }
    pub const fn plan(&self) -> &BoundaryEntryPlan {
        self.plan.plan()
    }
    pub fn calling_plan_report_fingerprint(&self) -> u64 {
        self.plan.contract_report_fingerprint()
    }
    pub fn calling_plan_commitment(&self) -> [u8; 32] {
        self.plan.contract_commitment_digest()
    }
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices planning rejection retains provider custody"]
pub struct UefiExitBootServicesInvocationPlanningError<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}
impl<'system_table, 'boot_services>
    UefiExitBootServicesInvocationPlanningError<'system_table, 'boot_services>
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
impl std::fmt::Display for UefiExitBootServicesInvocationPlanningError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiExitBootServicesInvocationPlanningError<'_, '_> {}

/// Consume the live provider into the exact target-authored ExitBootServices
/// call shape. The evaluated plan fixes the RCX/RDX operand placement and RAX
/// status under the Microsoft-x64 policy; its exactness is replayed here and
/// at every later custody transition.
pub fn prepare_uefi_exit_boot_services_invocation<'system_table, 'boot_services>(
    provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
) -> Result<
    PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    Box<UefiExitBootServicesInvocationPlanningError<'system_table, 'boot_services>>,
> {
    let plan = match evaluate_exit_boot_services_plan() {
        Ok(plan) => plan,
        Err(diagnostic) => {
            return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
                provider,
                diagnostic,
            }));
        }
    };
    if let Err(diagnostic) = validate_exact_call_shape(plan.plan()) {
        return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
            provider,
            diagnostic,
        }));
    }
    if (
        provider.field.ordinal(),
        provider.field.byte_offset(),
        provider.field.byte_size(),
        provider.field.alignment(),
        provider.field.kind(),
    ) != (
        EXIT_BOOT_SERVICES_FIELD_ORDINAL,
        EXIT_BOOT_SERVICES_FIELD_OFFSET,
        EXIT_BOOT_SERVICES_FIELD_SIZE,
        EXIT_BOOT_SERVICES_FIELD_ALIGNMENT,
        UefiBootServicesNativeFieldKind::FunctionPointer,
    ) {
        return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI ExitBootServices provider service row drifted before planning".into(),
            ),
        }));
    }
    Ok(PlannedUefiExitBootServicesInvocation { provider, plan })
}

/// Concrete, still-uninvoked operands for one exact acquired-map
/// `ExitBootServices` attempt.
///
/// The carrier keeps the physical image handle, service function, and map key
/// private. Public observations are limited to identities and ABI
/// destinations; neither firmware operand can be reinterpreted as storage
/// authority. The bound attempt belongs to one exact
/// [`UefiOsHandoffMapAcquired`] of the same physical invocation.
#[must_use = "bound UEFI ExitBootServices operands retain provider and attempt custody"]
pub struct BoundUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    _handle: NonZeroU64,
    _service: NonZeroU64,
    map_key: UefiMemoryMapKeyId,
    handoff: UefiOsHandoffId,
}

impl std::fmt::Debug for BoundUefiExitBootServicesInvocation<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundUefiExitBootServicesInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("handoff", &self.handoff)
            .field("map_key", &self.map_key)
            .field("argument_destinations", &self.argument_destinations())
            .finish_non_exhaustive()
    }
}

impl BoundUefiExitBootServicesInvocation<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }
    pub const fn service_identity(&self) -> &'static str {
        self.invocation.service_identity()
    }
    pub const fn handoff_id(&self) -> UefiOsHandoffId {
        self.handoff
    }
    pub const fn map_key(&self) -> UefiMemoryMapKeyId {
        self.map_key
    }
    pub const fn argument_destinations(&self) -> [MachineRegister; 2] {
        [MachineRegister::X86Rcx, MachineRegister::X86Rdx]
    }
    pub fn calling_plan_report_fingerprint(&self) -> u64 {
        self.invocation.calling_plan_report_fingerprint()
    }
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices operand rejection retains provider custody"]
pub struct UefiExitBootServicesInvocationBindingError<'system_table, 'boot_services> {
    invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}
impl<'system_table, 'boot_services>
    UefiExitBootServicesInvocationBindingError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.diagnostic)
    }
}
impl std::fmt::Display for UefiExitBootServicesInvocationBindingError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiExitBootServicesInvocationBindingError<'_, '_> {}

/// Bind the exact acquired map and retained physical image-handle value to the
/// RCX/RDX plan. The acquired carrier proves the key under the handoff's
/// current attempt; an acquired map from a different physical invocation, a
/// drifted plan, or address-free image-handle provenance cannot cross this
/// edge and rejects without consuming either input.
pub fn bind_uefi_exit_boot_services_invocation<'system_table, 'boot_services>(
    invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    acquired: &UefiOsHandoffMapAcquired,
) -> Result<
    BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    Box<UefiExitBootServicesInvocationBindingError<'system_table, 'boot_services>>,
> {
    let reject = |invocation, message: &'static str| {
        Err(Box::new(UefiExitBootServicesInvocationBindingError {
            invocation,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };
    if !matches_exact_uefi_x64_call_plan(&invocation.plan)
        || validate_exact_call_shape(invocation.plan.plan()).is_err()
    {
        return reject(
            invocation,
            "UEFI ExitBootServices operand binding plan drifted from its exact call shape",
        );
    }
    if acquired.physical_invocation() != invocation.provider.physical_invocation() {
        return reject(
            invocation,
            "UEFI ExitBootServices attempt bound a map acquired under a different physical invocation",
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
            "UEFI ExitBootServices operand binding requires the exact physical image-handle value",
        );
    };
    let service = invocation.provider.exit_boot_services;
    Ok(BoundUefiExitBootServicesInvocation {
        invocation,
        _handle: handle,
        _service: service,
        map_key: acquired.map_key(),
        handoff: acquired.handoff_id(),
    })
}

type UefiExitBootServicesFunction =
    unsafe extern "efiapi" fn(image_handle: *mut c_void, map_key: usize) -> u64;

/// Closed interpretation of the exact returned `EFI_STATUS`. Unknown values
/// remain observable as unsupported execution results and retain their
/// custody for release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UefiExitBootServicesAttemptStatus {
    Success,
    InvalidParameter,
    Unknown,
}

/// Sealed evidence that the exact retained service was invoked once with its
/// retained operands. The raw status is private evidence behind the closed
/// classification.
#[must_use = "executed UEFI ExitBootServices custody must be admitted or released"]
pub struct ExecutedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    invocation: BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
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
    pub const fn status(&self) -> UefiExitBootServicesAttemptStatus {
        match self.status_code {
            EFI_SUCCESS => UefiExitBootServicesAttemptStatus::Success,
            EFI_INVALID_PARAMETER => UefiExitBootServicesAttemptStatus::InvalidParameter,
            _ => UefiExitBootServicesAttemptStatus::Unknown,
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
    if !matches_exact_uefi_x64_call_plan(&invocation.invocation.plan)
        || validate_exact_call_shape(invocation.invocation.plan.plan()).is_err()
    {
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
    /// `EFI_INVALID_PARAMETER`: the map/key pair is stale. Provider custody
    /// returns for a fresh acquire and bind; the result feeds the ledger's
    /// stale-key transition.
    Retry {
        invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        status_code: u64,
        result: UefiExitBootServicesProviderResult,
    },
    /// `EFI_SUCCESS`: boot-scoped services were consumed. The result feeds
    /// the ledger's completing transition; no provider custody survives.
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
        UefiExitBootServicesAttemptStatus::InvalidParameter => {
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

#[cfg(test)]
mod tests {
    use super::{
        EFI_INVALID_PARAMETER, EFI_STATUS_ERROR_BIT, EFI_SUCCESS, EXIT_BOOT_SERVICES_FIELD_OFFSET,
        ExternalRootDiagnostic, LifecycleScopedUefiBootServicesProjection,
        LifecycleScopedUefiExitBootServicesProvider, NonZeroU64, TargetProfile,
        UefiApplicationFirmwareLedger, UefiBootServicesTableOccurrenceId,
        UefiExitBootServicesAttemptOutcome, UefiImageHandleOccurrenceId, UefiMemoryMapKeyId,
        UefiOsHandoffMapAcquired, UefiPhysicalInvocationId,
        admit_uefi_exit_boot_services_execution, bind_uefi_exit_boot_services_invocation, c_void,
        execute_uefi_exit_boot_services, join_lifecycle_scoped_uefi_exit_boot_services_provider,
        plan_uefi_boot_services_native_layout, prepare_uefi_exit_boot_services_invocation,
    };
    use crate::{
        UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId, UefiErrorStatus,
        UefiFirmwareSessionId, UefiMemoryMapAcquisition, UefiMemoryMapSnapshotId,
        UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId, UefiOsHandoffId,
        UefiOsHandoffLedger, UefiOsHandoffMapRequired, UefiOsHandoffProgress,
        UefiOsHandoffStackEvidenceId, UefiSystemTableOccurrenceId,
        join_lifecycle_scoped_uefi_system_table, join_uefi_application_physical_arrival,
        prepare_uefi_application_bootstrap_adapter_invocation,
        project_uefi_application_boot_services,
    };
    use program_entry_plan::{
        ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
        UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
    };
    use std::num::NonZeroU32;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use target::{
        ProgramEntryPhysicalContractPackage, UEFI_BOOT_SERVICES_SIGNATURE,
        UEFI_SYSTEM_TABLE_SIGNATURE, plan_uefi_system_table_native_layout,
        validate_uefi_boot_services_occurrence, validate_uefi_system_table_occurrence,
    };

    static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static FAKE_STATUS: AtomicU64 = AtomicU64::new(0);
    static OBSERVED_HANDLE: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_MAP_KEY: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_CALLS: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "efiapi" fn fake_exit_boot_services(
        image_handle: *mut c_void,
        map_key: usize,
    ) -> u64 {
        OBSERVED_CALLS.fetch_add(1, Ordering::SeqCst);
        OBSERVED_HANDLE.store(image_handle as usize, Ordering::SeqCst);
        OBSERVED_MAP_KEY.store(map_key, Ordering::SeqCst);
        FAKE_STATUS.load(Ordering::SeqCst)
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

    fn boot_table(service_address: u64) -> Vec<u8> {
        table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            EXIT_BOOT_SERVICES_FIELD_OFFSET as usize,
            service_address,
        )
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
    /// handoff ledger and provider edge join the same invocation.
    const INVOCATION_OFFSET: u64 = 2;

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

    fn provider<'system_table, 'boot_services>(
        ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
        system: &'system_table [u8],
        boot: &'boot_services [u8],
        base: u64,
        boot_address: u64,
    ) -> LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services> {
        let projection = projection(ledger, system, base);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            boot,
        )
        .unwrap();
        join_lifecycle_scoped_uefi_exit_boot_services_provider(
            ledger,
            projection,
            integrity,
            id(
                base + 3,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap()
    }

    fn handoff(
        base: u64,
        invocation: u64,
        attempts: u32,
    ) -> (UefiOsHandoffLedger, UefiOsHandoffMapRequired) {
        UefiOsHandoffLedger::new(
            id(base, UefiOsHandoffId::from_normalized_identity),
            id(base + 1, UefiFirmwareSessionId::from_normalized_identity),
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

    fn acquire(
        ledger: &mut UefiOsHandoffLedger,
        arrival: UefiOsHandoffMapRequired,
        base: u64,
    ) -> UefiOsHandoffMapAcquired {
        let acquisition = UefiMemoryMapAcquisition::for_test(
            ledger.firmware_session(),
            ledger.physical_invocation(),
            id(base, UefiMemoryMapSnapshotId::from_normalized_identity),
            id(base + 1, UefiMemoryMapKeyId::from_normalized_identity),
            96,
            48,
            1,
        );
        ledger.acquire_memory_map(arrival, acquisition).unwrap()
    }

    #[test]
    fn successful_attempt_consumes_boot_services_and_completes_the_handoff() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        FAKE_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        OBSERVED_CALLS.store(0, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(fake_exit_boot_services_address());
        let mut firmware = ledger(10);
        let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
        let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
        let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 2);
        let allocations = arrival.allocation_roster();
        let stack = arrival.surviving_stack();
        let acquired = acquire(&mut handoff, arrival, 40);
        let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
        assert_eq!(bound.map_key().normalized_identity(), 41);
        // SAFETY: the bound service address is the test-local efiapi fake baked
        // into the fabricated Boot Services table above.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        assert_eq!(OBSERVED_HANDLE.load(Ordering::SeqCst), 0x1000_0000 + 13);
        assert_eq!(OBSERVED_MAP_KEY.load(Ordering::SeqCst), 41);
        let UefiExitBootServicesAttemptOutcome::Exited {
            status_code,
            result,
        } = admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("EFI_SUCCESS must exit the handoff attempt")
        };
        assert_eq!(status_code, EFI_SUCCESS);
        let UefiOsHandoffProgress::Complete(complete) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("successful provider result must complete the handoff")
        };
        assert_eq!(complete.allocation_roster(), allocations);
        assert_eq!(complete.surviving_stack(), stack);
        assert_eq!(complete.final_map().normalized_identity(), 40);
        assert_ne!(complete.receipt().normalized_identity(), 0);
        assert_eq!(OBSERVED_CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn stale_key_returns_complete_provider_custody_and_retries_to_success() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        FAKE_STATUS.store(EFI_INVALID_PARAMETER, Ordering::SeqCst);
        OBSERVED_CALLS.store(0, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(fake_exit_boot_services_address());
        let mut firmware = ledger(10);
        let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
        let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
        let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 2);
        let acquired = acquire(&mut handoff, arrival, 40);
        let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        assert_eq!(OBSERVED_MAP_KEY.load(Ordering::SeqCst), 41);
        let UefiExitBootServicesAttemptOutcome::Retry {
            invocation,
            status_code,
            result,
        } = admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("EFI_INVALID_PARAMETER must return provider custody for retry")
        };
        assert_eq!(status_code, EFI_INVALID_PARAMETER);
        let UefiOsHandoffProgress::Retry(retry) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("the first stale key must retry")
        };
        assert_eq!(retry.remaining_attempts(), 1);

        // The returned custody binds the fresh acquired map and drives the
        // successful second attempt through the same provider edge.
        let acquired = acquire(&mut handoff, retry, 42);
        let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
        FAKE_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        // SAFETY: same retained fake service address.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        assert_eq!(OBSERVED_MAP_KEY.load(Ordering::SeqCst), 43);
        let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
            admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("the retried attempt must exit on success")
        };
        let UefiOsHandoffProgress::Complete(complete) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("the retried success must complete the handoff")
        };
        assert_eq!(complete.final_map().normalized_identity(), 42);
        assert_eq!(OBSERVED_CALLS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn exhausted_attempt_keeps_provider_custody_releasable_for_firmware_return() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        FAKE_STATUS.store(EFI_INVALID_PARAMETER, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(fake_exit_boot_services_address());
        let mut firmware = ledger(10);
        let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
        let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
        let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 1);
        let acquired = acquire(&mut handoff, arrival, 40);
        let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        let UefiExitBootServicesAttemptOutcome::Retry {
            invocation, result, ..
        } = admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("stale key must return provider custody")
        };
        let UefiOsHandoffProgress::Exhausted(exhausted) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("the final stale key must exhaust")
        };
        assert_eq!(exhausted.boot_services().normalized_identity(), 53);
        assert_eq!(exhausted.status().value(), EFI_STATUS_ERROR_BIT);

        // Exhaustion leaves Boot Services live: the returned provider releases
        // its scope and the invocation can still begin its firmware return.
        let released = firmware
            .release_planned_uefi_exit_boot_services_invocation(invocation)
            .unwrap();
        assert_eq!(released.ledger, firmware.ledger_id());
        firmware.begin_firmware_return().unwrap();
    }

    #[test]
    fn foreign_ledger_and_correspondence_drift_reject_before_any_binding() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(fake_exit_boot_services_address());
        let mut owner = ledger(10);
        let foreign = ledger(60);
        let owner_projection = projection(&mut owner, &system, 13);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            &boot,
        )
        .unwrap();
        let error = join_lifecycle_scoped_uefi_exit_boot_services_provider(
            &foreign,
            owner_projection,
            integrity,
            id(
                16,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap_err();
        assert!(error.diagnostic().0.contains("different or inactive"));

        // A Boot Services occurrence address that does not correspond to the
        // projected System Table field rejects with custody returned. One
        // ledger admits exactly one image-handle occurrence, so the returned
        // projection threads through each rejection.
        let (returned_projection, integrity, occurrence, _, _) = error.into_parts();
        let error = join_lifecycle_scoped_uefi_exit_boot_services_provider(
            &owner,
            returned_projection,
            integrity,
            occurrence,
            NonZeroU64::new(boot_address + 8).unwrap(),
        )
        .unwrap_err();
        assert!(error.diagnostic().0.contains("does not correspond"));

        // A null service pointer cannot leave the Boot-Services-live phase.
        let null_boot = boot_table(0);
        let (returned_projection, _, occurrence, _, _) = error.into_parts();
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            &null_boot,
        )
        .unwrap();
        let error = join_lifecycle_scoped_uefi_exit_boot_services_provider(
            &owner,
            returned_projection,
            integrity,
            occurrence,
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap_err();
        assert!(error.diagnostic().0.contains("is null"));
        let (returned_projection, _, _, _, _) = error.into_parts();
        let released = owner
            .release_lifecycle_scoped_boot_services_projection(returned_projection)
            .unwrap();
        assert_eq!(released.ledger, owner.ledger_id());
        owner.begin_firmware_return().unwrap();
    }

    #[test]
    fn acquired_map_from_a_different_invocation_and_address_free_handle_reject() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(fake_exit_boot_services_address());
        let mut firmware = ledger(10);
        let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
        let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();

        // The acquired carrier under a foreign physical invocation cannot bind
        // this provider's operands; the planned custody returns unchanged.
        let (mut foreign_handoff, foreign_arrival) = handoff(70, 999, 1);
        let foreign_acquired = acquire(&mut foreign_handoff, foreign_arrival, 80);
        let error =
            bind_uefi_exit_boot_services_invocation(invocation, &foreign_acquired).unwrap_err();
        assert!(
            error
                .diagnostic()
                .0
                .contains("different physical invocation")
        );
        let (invocation, _) = error.into_parts();

        // A map acquired under the provider's invocation binds cleanly.
        let (mut owner_handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 1);
        let acquired = acquire(&mut owner_handoff, arrival, 90);
        let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
        let released = firmware
            .release_bound_uefi_exit_boot_services_invocation(bound)
            .unwrap();
        assert_eq!(released.ledger, firmware.ledger_id());

        // Address-free image-handle provenance cannot cross the operand edge.
        // A firmware ledger admits one image-handle occurrence, so this case
        // needs its own invocation ledger.
        let mut address_free_firmware = ledger(60);
        let projection =
            projection_with_image_handle(&mut address_free_firmware, &system, 43, false);
        let integrity = validate_uefi_boot_services_occurrence(
            plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
            &boot,
        )
        .unwrap();
        let provider = join_lifecycle_scoped_uefi_exit_boot_services_provider(
            &address_free_firmware,
            projection,
            integrity,
            id(
                46,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap();
        let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
        let (mut address_free_handoff, address_free_arrival) =
            handoff(90, 60 + INVOCATION_OFFSET, 1);
        let address_free_acquired = acquire(&mut address_free_handoff, address_free_arrival, 95);
        let error = bind_uefi_exit_boot_services_invocation(invocation, &address_free_acquired)
            .unwrap_err();
        assert!(error.diagnostic().0.contains("image-handle value"));
    }

    #[test]
    fn unknown_status_retains_executed_custody_for_release() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        FAKE_STATUS.store(EFI_STATUS_ERROR_BIT | 9, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(fake_exit_boot_services_address());
        let mut firmware = ledger(10);
        let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
        let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
        let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 1);
        let acquired = acquire(&mut handoff, arrival, 40);
        let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        let error = admit_uefi_exit_boot_services_execution(executed).unwrap_err();
        assert!(error.diagnostic().0.contains("closed target table"));
        let (execution, _) = error.into_parts();
        let released = firmware
            .release_executed_uefi_exit_boot_services_invocation(execution)
            .unwrap();
        assert_eq!(released.ledger, firmware.ledger_id());
        firmware.begin_firmware_return().unwrap();
    }

    #[test]
    fn success_leaves_no_provider_for_post_exit_use() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        FAKE_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(fake_exit_boot_services_address());
        let mut firmware = ledger(10);
        let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
        let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
        let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 2);
        let acquired = acquire(&mut handoff, arrival, 40);
        let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
            admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("EFI_SUCCESS must exit the handoff attempt")
        };
        // The Exited variant carries no provider carrier: after consumption no
        // binding, execution, or release edge remains expressible for this
        // invocation. The ledger-side forged-arrival rejection is witnessed in
        // os_handoff.rs's completed_handoff_cannot_reenter test.
        let UefiOsHandoffProgress::Complete(_) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("success must complete the handoff")
        };
    }

    #[test]
    fn provider_result_issuance_stays_module_family_private() {
        for source in [
            include_str!("os_handoff.rs"),
            include_str!("exit_boot_services.rs"),
            include_str!("get_memory_map.rs"),
            include_str!("get_memory_map/memory_map_buffer.rs"),
            include_str!("get_memory_map/provider_lifecycle.rs"),
            include_str!("get_memory_map/invocation_planning.rs"),
            include_str!("get_memory_map/execution.rs"),
        ] {
            let production = source
                .split("#[cfg(test)]")
                .next()
                .expect("production source");
            let compact = production
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>();
            for forbidden in [
                "pubfnstale_map_key",
                "pubfnsucceeded",
                "pub(crate)fnstale_map_key",
                "pub(crate)fnsucceeded",
                "pubfnfor_test",
                "pub(crate)fnfor_test",
                "implCloneforUefiOsHandoff",
                "implCloneforUefiExitBootServices",
                "implCloneforUefiMemoryMap",
                "implCloneforUefiGetMemoryMap",
            ] {
                assert!(
                    !compact.contains(forbidden),
                    "forbidden handoff authority surface appeared: {forbidden}"
                );
            }
        }
        let provider_source = include_str!("exit_boot_services.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(
            !provider_source.contains("UefiExitBootServicesProviderResultKind"),
            "the provider edge must mint results only through the ledger's private constructors"
        );
    }
}
