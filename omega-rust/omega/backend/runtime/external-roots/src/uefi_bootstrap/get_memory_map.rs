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

use std::ffi::c_void;
use std::num::NonZeroU64;

use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl, MachineRegister,
    ValidatedBoundaryEntryPlan, ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, plan_uefi_boot_services_native_layout,
};

use super::exit_boot_services::{evaluate_exit_boot_services_plan, validate_exact_call_shape};
use super::{PlannedUefiExitBootServicesInvocation, UefiApplicationFirmwareLedger};
use crate::{
    ExternalRootDiagnostic, Fnv1a, UefiBootServicesTableOccurrenceId, UefiFirmwareSessionId,
    UefiImageHandleOccurrenceId, UefiMemoryMapKeyId, UefiMemoryMapSnapshotId,
    UefiPhysicalInvocationId,
};

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

/// Growable, custody-scoped storage for one `GetMemoryMap` call family.
///
/// The carrier owns the descriptor storage and the four call cells privately;
/// the only writer past construction is the executed provider edge. Callers
/// observe capacity, the firmware-occupied map prefix, and nothing else — the
/// sealed outputs reach consumers only through `UefiMemoryMapAcquisition`. A
/// zero-capacity buffer is the UEFI probe shape: firmware reports the required
/// size through the size cell and returns `EFI_BUFFER_TOO_SMALL`.
#[must_use = "UEFI memory-map buffer custody must bind into a provider call or stay owned"]
pub struct UefiMemoryMapBuffer {
    map: Vec<u8>,
    map_size: usize,
    map_key: usize,
    descriptor_size: usize,
    descriptor_version: u32,
    occupied: usize,
}

impl UefiMemoryMapBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: vec![0; capacity],
            map_size: 0,
            map_key: 0,
            descriptor_size: 0,
            descriptor_version: 0,
            occupied: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.map.len()
    }

    /// Grow storage to at least the firmware-reported requirement. The
    /// previously occupied map prefix is retired: after a grow the buffer
    /// holds no live map until a later acquisition succeeds.
    pub fn grow(&mut self, required_map_bytes: usize) {
        if self.map.len() < required_map_bytes {
            self.map.resize(required_map_bytes, 0);
        }
        self.occupied = 0;
    }

    /// The byte prefix firmware reported written by the last admitted
    /// successful acquisition; empty before any success and after `grow`.
    pub fn occupied_bytes(&self) -> &[u8] {
        &self.map[..self.occupied]
    }

    pub const fn occupied_map_bytes(&self) -> usize {
        self.occupied
    }
}

impl Default for UefiMemoryMapBuffer {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

impl std::fmt::Debug for UefiMemoryMapBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiMemoryMapBuffer")
            .field("capacity", &self.capacity())
            .field("occupied", &self.occupied)
            .finish_non_exhaustive()
    }
}

/// Exact pending-exit borrow plus the private `GetMemoryMap` slot read from
/// the same validated Boot Services occurrence the exit provider sealed. The
/// carrier is non-clone and exposes report coordinates only; the service
/// function address and the borrowed operand cells remain private.
#[must_use = "UEFI GetMemoryMap provider borrows pending-exit invocation custody"]
pub struct LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services> {
    pending_exit:
        &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    field: UefiBootServicesNativeFieldLayout,
    get_memory_map: NonZeroU64,
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
        SERVICE_IDENTITY
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
/// retained beneath a pending `ExitBootServices` invocation. The pending plan
/// must still be live under `ledger` and replay-exact; the new provider only
/// borrows that custody, so a stale, foreign, or drifted invocation rejects
/// without consuming anything.
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
    if !pending_exit_plan_is_exact(invocation) {
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
    if (
        field.ordinal(),
        field.byte_offset(),
        field.byte_size(),
        field.alignment(),
        field.kind(),
    ) != (
        GET_MEMORY_MAP_FIELD_ORDINAL,
        GET_MEMORY_MAP_FIELD_OFFSET,
        GET_MEMORY_MAP_FIELD_SIZE,
        GET_MEMORY_MAP_FIELD_ALIGNMENT,
        UefiBootServicesNativeFieldKind::FunctionPointer,
    ) {
        return Err(ExternalRootDiagnostic(
            "UEFI GetMemoryMap row drifted from exact target geometry".into(),
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

/// One exact five-operand acquisition plan joined to the live provider
/// carrier. The retained plan fixes the RCX/RDX/R8/R9 operand registers, the
/// stack-resident descriptor-version slot, RAX status, shadow space, and
/// clobbers without performing the firmware call.
#[must_use = "planned UEFI GetMemoryMap invocation retains provider and pending-exit custody"]
pub struct PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services> {
    provider: LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
    plan: ValidatedBoundaryEntryPlan,
}

impl std::fmt::Debug for PlannedUefiGetMemoryMapInvocation<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlannedUefiGetMemoryMapInvocation")
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

impl PlannedUefiGetMemoryMapInvocation<'_, '_, '_> {
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

impl<'pending_exit, 'system_table, 'boot_services>
    PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>
{
    pub fn into_pending_exit_invocation(
        self,
    ) -> &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
        self.provider.pending_exit
    }
}

#[derive(Debug)]
#[must_use = "UEFI GetMemoryMap planning rejection retains provider custody"]
pub struct UefiGetMemoryMapInvocationPlanningError<'pending_exit, 'system_table, 'boot_services> {
    provider: LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}
impl<'pending_exit, 'system_table, 'boot_services>
    UefiGetMemoryMapInvocationPlanningError<'pending_exit, 'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.provider, self.diagnostic)
    }
}
impl std::fmt::Display for UefiGetMemoryMapInvocationPlanningError<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiGetMemoryMapInvocationPlanningError<'_, '_, '_> {}

/// Consume the live provider into the exact target-authored GetMemoryMap call
/// shape. The evaluated plan fixes the four register operands and the
/// stack-resident descriptor-version operand under the Microsoft-x64 policy;
/// its exactness is replayed here and at every later custody transition.
pub fn prepare_uefi_get_memory_map_invocation<'pending_exit, 'system_table, 'boot_services>(
    provider: LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
) -> Result<
    PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    Box<UefiGetMemoryMapInvocationPlanningError<'pending_exit, 'system_table, 'boot_services>>,
> {
    let plan = match evaluate_get_memory_map_plan() {
        Ok(plan) => plan,
        Err(diagnostic) => {
            return Err(Box::new(UefiGetMemoryMapInvocationPlanningError {
                provider,
                diagnostic,
            }));
        }
    };
    if let Err(diagnostic) = validate_exact_get_memory_map_call_shape(plan.plan()) {
        return Err(Box::new(UefiGetMemoryMapInvocationPlanningError {
            provider,
            diagnostic,
        }));
    }
    if !pending_exit_plan_is_exact(provider.pending_exit) {
        return Err(Box::new(UefiGetMemoryMapInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI GetMemoryMap provider lost its exact pending exit plan before planning"
                    .into(),
            ),
        }));
    }
    if (
        provider.field.ordinal(),
        provider.field.byte_offset(),
        provider.field.byte_size(),
        provider.field.alignment(),
        provider.field.kind(),
    ) != (
        GET_MEMORY_MAP_FIELD_ORDINAL,
        GET_MEMORY_MAP_FIELD_OFFSET,
        GET_MEMORY_MAP_FIELD_SIZE,
        GET_MEMORY_MAP_FIELD_ALIGNMENT,
        UefiBootServicesNativeFieldKind::FunctionPointer,
    ) {
        return Err(Box::new(UefiGetMemoryMapInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI GetMemoryMap provider service row drifted before planning".into(),
            ),
        }));
    }
    Ok(PlannedUefiGetMemoryMapInvocation { provider, plan })
}

/// Concrete, still-uninvoked operands for one exact map acquisition: the
/// private service address and the exclusively borrowed buffer/cell storage.
/// Public observations are limited to identities and ABI destinations.
#[must_use = "bound UEFI GetMemoryMap operands retain provider and buffer custody"]
pub struct BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer> {
    invocation: PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    buffer: &'buffer mut UefiMemoryMapBuffer,
    _service: NonZeroU64,
}

impl std::fmt::Debug for BoundUefiGetMemoryMapInvocation<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundUefiGetMemoryMapInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("service_identity", &self.service_identity())
            .field("buffer_capacity", &self.buffer.capacity())
            .field("argument_destinations", &self.argument_destinations())
            .finish_non_exhaustive()
    }
}

impl BoundUefiGetMemoryMapInvocation<'_, '_, '_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }
    pub const fn service_identity(&self) -> &'static str {
        self.invocation.service_identity()
    }
    /// The four register destinations; the fifth operand occupies the first
    /// stack slot above the shadow area and has no register destination.
    pub const fn argument_destinations(&self) -> [MachineRegister; 4] {
        [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
        ]
    }
    pub fn calling_plan_report_fingerprint(&self) -> u64 {
        self.invocation.calling_plan_report_fingerprint()
    }
}

impl<'pending_exit, 'system_table, 'boot_services, 'buffer>
    BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>
{
    /// End acquisition custody before execution: the pending-exit borrow and
    /// the buffer return to the caller unchanged.
    pub fn into_pending_exit_invocation(
        self,
    ) -> (
        &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        &'buffer mut UefiMemoryMapBuffer,
    ) {
        (self.invocation.provider.pending_exit, self.buffer)
    }
}

#[derive(Debug)]
#[must_use = "UEFI GetMemoryMap operand rejection retains provider and buffer custody"]
pub struct UefiGetMemoryMapInvocationBindingError<
    'pending_exit,
    'system_table,
    'boot_services,
    'buffer,
> {
    invocation: PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    buffer: &'buffer mut UefiMemoryMapBuffer,
    diagnostic: ExternalRootDiagnostic,
}
impl<'pending_exit, 'system_table, 'boot_services, 'buffer>
    UefiGetMemoryMapInvocationBindingError<'pending_exit, 'system_table, 'boot_services, 'buffer>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
        &'buffer mut UefiMemoryMapBuffer,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.buffer, self.diagnostic)
    }
}
impl std::fmt::Display for UefiGetMemoryMapInvocationBindingError<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiGetMemoryMapInvocationBindingError<'_, '_, '_, '_> {}

/// Bind one custody-scoped buffer to the retained five-operand plan. Binding
/// re-arms the call cells: the in-out size cell takes the buffer capacity and
/// every output cell clears, so outputs read back after execution can only
/// name this attempt. A drifted plan or service row rejects without consuming
/// either input.
pub fn bind_uefi_get_memory_map_invocation<
    'pending_exit,
    'system_table,
    'boot_services,
    'buffer,
>(
    invocation: PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    buffer: &'buffer mut UefiMemoryMapBuffer,
) -> Result<
    BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
    Box<
        UefiGetMemoryMapInvocationBindingError<
            'pending_exit,
            'system_table,
            'boot_services,
            'buffer,
        >,
    >,
> {
    let reject = |invocation, buffer, message: &'static str| {
        Err(Box::new(UefiGetMemoryMapInvocationBindingError {
            invocation,
            buffer,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };
    if !matches_exact_uefi_x64_call_plan(&invocation.plan)
        || validate_exact_get_memory_map_call_shape(invocation.plan.plan()).is_err()
        || !pending_exit_plan_is_exact(invocation.provider.pending_exit)
    {
        return reject(
            invocation,
            buffer,
            "UEFI GetMemoryMap operand binding plan drifted from its exact call shape",
        );
    }
    if (
        invocation.provider.field.ordinal(),
        invocation.provider.field.byte_offset(),
        invocation.provider.field.byte_size(),
        invocation.provider.field.alignment(),
        invocation.provider.field.kind(),
    ) != (
        GET_MEMORY_MAP_FIELD_ORDINAL,
        GET_MEMORY_MAP_FIELD_OFFSET,
        GET_MEMORY_MAP_FIELD_SIZE,
        GET_MEMORY_MAP_FIELD_ALIGNMENT,
        UefiBootServicesNativeFieldKind::FunctionPointer,
    ) {
        return reject(
            invocation,
            buffer,
            "UEFI GetMemoryMap provider service row drifted before binding",
        );
    }
    // Re-arm the in-out and output cells so every post-call value is sealed
    // by this exact attempt; stale outputs from an earlier call cannot
    // masquerade as fresh acquisition evidence.
    buffer.map_size = buffer.map.len();
    buffer.map_key = 0;
    buffer.descriptor_size = 0;
    buffer.descriptor_version = 0;
    buffer.occupied = 0;
    Ok(BoundUefiGetMemoryMapInvocation {
        _service: invocation.provider.get_memory_map,
        invocation,
        buffer,
    })
}

type UefiGetMemoryMapFunction = unsafe extern "efiapi" fn(
    memory_map_size: *mut usize,
    memory_map: *mut c_void,
    map_key: *mut usize,
    descriptor_size: *mut usize,
    descriptor_version: *mut u32,
) -> u64;

/// Closed interpretation of the exact returned `EFI_STATUS`. `BufferTooSmall`
/// is the grow-and-retry outcome; unknown values stay observable as
/// unsupported execution results and retain their custody for release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UefiGetMemoryMapAttemptStatus {
    Success,
    BufferTooSmall,
    InvalidParameter,
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
    pub const fn status(&self) -> UefiGetMemoryMapAttemptStatus {
        match self.status_code {
            EFI_SUCCESS => UefiGetMemoryMapAttemptStatus::Success,
            EFI_BUFFER_TOO_SMALL => UefiGetMemoryMapAttemptStatus::BufferTooSmall,
            EFI_INVALID_PARAMETER => UefiGetMemoryMapAttemptStatus::InvalidParameter,
            _ => UefiGetMemoryMapAttemptStatus::Unknown,
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
    if !matches_exact_uefi_x64_call_plan(&invocation.invocation.plan)
        || validate_exact_get_memory_map_call_shape(invocation.invocation.plan.plan()).is_err()
    {
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
        UefiGetMemoryMapAttemptStatus::InvalidParameter => {
            reject(execution, "UEFI GetMemoryMap returned InvalidParameter")
        }
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
    pub(super) fn for_test(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LifecycleScopedUefiBootServicesProjection, UefiApplicationBootstrapLedgerId,
        UefiBootServicesPhaseLeaseId, UefiErrorStatus, UefiExitBootServicesAttemptOutcome,
        UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId, UefiOsHandoffId,
        UefiOsHandoffLedger, UefiOsHandoffMapRequired, UefiOsHandoffProgress,
        UefiOsHandoffStackEvidenceId, UefiSystemTableOccurrenceId,
        admit_uefi_exit_boot_services_execution, bind_uefi_exit_boot_services_invocation,
        execute_uefi_exit_boot_services, join_lifecycle_scoped_uefi_exit_boot_services_provider,
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
    use std::num::NonZeroU32;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use target::{
        ProgramEntryPhysicalContractPackage, UEFI_BOOT_SERVICES_SIGNATURE,
        UEFI_SYSTEM_TABLE_SIGNATURE, plan_uefi_boot_services_native_layout,
        plan_uefi_system_table_native_layout, validate_uefi_boot_services_occurrence,
        validate_uefi_system_table_occurrence,
    };

    static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static FAKE_GET_STATUS: AtomicU64 = AtomicU64::new(0);
    static FAKE_EXIT_STATUS: AtomicU64 = AtomicU64::new(0);
    static FAKE_REQUIRED_BYTES: AtomicUsize = AtomicUsize::new(0);
    static FAKE_MAP_BYTES: AtomicUsize = AtomicUsize::new(0);
    static FAKE_MAP_KEY: AtomicUsize = AtomicUsize::new(0);
    static FAKE_DESCRIPTOR_SIZE: AtomicUsize = AtomicUsize::new(0);
    static FAKE_DESCRIPTOR_VERSION: AtomicU64 = AtomicU64::new(0);
    static OBSERVED_GET_CALLS: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_CAPACITY: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_MAP_POINTER: AtomicUsize = AtomicUsize::new(0);
    static OBSERVED_EXIT_KEY: AtomicUsize = AtomicUsize::new(0);

    /// Firmware-shaped fake: reads the in-out size cell, writes the scripted
    /// required extent on BufferTooSmall and the map bytes plus all output
    /// cells on success.
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
        OBSERVED_CAPACITY.store(capacity, Ordering::SeqCst);
        OBSERVED_MAP_POINTER.store(memory_map as usize, Ordering::SeqCst);
        let status = FAKE_GET_STATUS.load(Ordering::SeqCst);
        if status == EFI_SUCCESS {
            let bytes = FAKE_MAP_BYTES.load(Ordering::SeqCst);
            // A misreporting "firmware" may claim an extent beyond capacity;
            // only capacity-bounded bytes are ever written so admission can
            // witness the over-capacity report without scribbling.
            let writable = bytes.min(capacity);
            if !memory_map.is_null() && writable != 0 {
                // SAFETY: the edge guarantees `capacity` writable bytes here.
                unsafe { std::ptr::write_bytes(memory_map.cast::<u8>(), 0x5a, writable) };
            }
            // SAFETY: same exclusive output-cell custody as above.
            unsafe {
                *memory_map_size = bytes;
                *map_key = FAKE_MAP_KEY.load(Ordering::SeqCst);
                *descriptor_size = FAKE_DESCRIPTOR_SIZE.load(Ordering::SeqCst);
                *descriptor_version = FAKE_DESCRIPTOR_VERSION.load(Ordering::SeqCst) as u32;
            }
        } else if status == EFI_BUFFER_TOO_SMALL {
            // SAFETY: same exclusive output-cell custody as above.
            unsafe { *memory_map_size = FAKE_REQUIRED_BYTES.load(Ordering::SeqCst) };
        }
        status
    }

    unsafe extern "efiapi" fn fake_exit_boot_services(
        _image_handle: *mut c_void,
        map_key: usize,
    ) -> u64 {
        OBSERVED_EXIT_KEY.store(map_key, Ordering::SeqCst);
        FAKE_EXIT_STATUS.load(Ordering::SeqCst)
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

    /// Fabricated Boot Services table carrying both handoff-edge rows:
    /// GetMemoryMap at its exact offset and ExitBootServices at its own.
    fn boot_table(get_memory_map: u64, exit_boot_services: u64) -> Vec<u8> {
        let layout = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap();
        let exit_offset = layout
            .field_layout(UefiBootServicesNativeField::ExitBootServices)
            .unwrap()
            .byte_offset() as usize;
        let mut bytes = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            GET_MEMORY_MAP_FIELD_OFFSET as usize,
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

    fn projection<'a>(
        ledger: &mut UefiApplicationFirmwareLedger<'a>,
        bytes: &'a [u8],
        base: u64,
    ) -> LifecycleScopedUefiBootServicesProjection<'a> {
        let occurrence = id(base, UefiImageHandleOccurrenceId::from_normalized_identity);
        let image = ledger
            .admit_image_handle_physical_input(
                occurrence,
                NonZeroU64::new(0x1000_0000 + base).unwrap(),
            )
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

    /// The pending exit invocation the acquisition edge borrows: a complete
    /// provider join and plan against the fabricated tables.
    fn planned_exit<'system_table, 'boot_services>(
        ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
        system: &'system_table [u8],
        boot: &'boot_services [u8],
        base: u64,
        boot_address: u64,
    ) -> PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
        let projection = projection(ledger, system, base);
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

    fn script_success(key: usize, map_bytes: usize, descriptor_size: usize, version: u64) {
        FAKE_GET_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        FAKE_MAP_BYTES.store(map_bytes, Ordering::SeqCst);
        FAKE_MAP_KEY.store(key, Ordering::SeqCst);
        FAKE_DESCRIPTOR_SIZE.store(descriptor_size, Ordering::SeqCst);
        FAKE_DESCRIPTOR_VERSION.store(version, Ordering::SeqCst);
    }

    /// One acquire leg: join the provider beneath the pending exit, plan,
    /// bind the buffer, execute, and admit. Returns the admitted outcome.
    fn acquire_outcome<'p, 's, 'b, 'buffer>(
        firmware: &UefiApplicationFirmwareLedger<'s>,
        pending_exit: &'p PlannedUefiExitBootServicesInvocation<'s, 'b>,
        buffer: &'buffer mut UefiMemoryMapBuffer,
    ) -> UefiGetMemoryMapAttemptOutcome<'p, 's, 'b, 'buffer> {
        let provider =
            join_lifecycle_scoped_uefi_get_memory_map_provider(firmware, pending_exit).unwrap();
        let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
        let bound = bind_uefi_get_memory_map_invocation(invocation, buffer).unwrap();
        // SAFETY: the retained service address is the test-local efiapi fake
        // baked into the fabricated Boot Services table.
        let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
        admit_uefi_get_memory_map_execution(executed).unwrap()
    }

    #[test]
    fn successful_acquisition_seals_key_and_geometry_and_feeds_exit() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_success(0x5AFE_0041, 96, 48, 1);
        FAKE_EXIT_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        OBSERVED_GET_CALLS.store(0, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);

        let mut buffer = UefiMemoryMapBuffer::with_capacity(128);
        let storage = buffer.map.as_ptr() as usize;
        let UefiGetMemoryMapAttemptOutcome::Acquired {
            buffer,
            acquisition,
            ..
        } = acquire_outcome(&firmware, &pending_exit, &mut buffer)
        else {
            panic!("scripted success must mint acquisition evidence")
        };
        assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(OBSERVED_CAPACITY.load(Ordering::SeqCst), 128);
        assert_eq!(OBSERVED_MAP_POINTER.load(Ordering::SeqCst), storage);
        assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_0041);
        assert_eq!(acquisition.map_bytes(), 96);
        assert_eq!(acquisition.descriptor_size(), 48);
        assert_eq!(acquisition.descriptor_version(), 1);
        assert_eq!(acquisition.firmware_session(), handoff.firmware_session());
        assert_eq!(
            acquisition.physical_invocation(),
            pending_exit.physical_invocation()
        );
        assert!(buffer.occupied_bytes().iter().all(|byte| *byte == 0x5a));
        assert_eq!(buffer.occupied_map_bytes(), 96);

        // The same key and descriptor identity reach the exit binding; the
        // successful exit completes the handoff with this snapshot.
        let acquired = handoff.acquire_memory_map(arrival, acquisition).unwrap();
        assert_eq!(acquired.descriptor_size(), 48);
        assert_eq!(acquired.descriptor_version(), 1);
        let snapshot = acquired.snapshot();
        let bound = bind_uefi_exit_boot_services_invocation(pending_exit, &acquired).unwrap();
        assert_eq!(bound.map_key().normalized_identity(), 0x5AFE_0041);
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        assert_eq!(OBSERVED_EXIT_KEY.load(Ordering::SeqCst), 0x5AFE_0041);
        let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
            admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("EFI_SUCCESS must exit the handoff attempt")
        };
        let UefiOsHandoffProgress::Complete(complete) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("successful provider result must complete the handoff")
        };
        assert_eq!(complete.final_map(), snapshot);
    }

    #[test]
    fn buffer_too_small_reports_required_then_grow_and_rebind_succeeds() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        FAKE_GET_STATUS.store(EFI_BUFFER_TOO_SMALL, Ordering::SeqCst);
        FAKE_REQUIRED_BYTES.store(192, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);

        // The zero-capacity probe shape: null map pointer, empty size cell.
        let mut probe = UefiMemoryMapBuffer::default();
        let UefiGetMemoryMapAttemptOutcome::BufferTooSmall {
            buffer: probe,
            required_map_bytes,
            ..
        } = acquire_outcome(&firmware, &pending_exit, &mut probe)
        else {
            panic!("undersized buffer must return the required extent")
        };
        assert_eq!(required_map_bytes, 192);
        assert_eq!(OBSERVED_CAPACITY.load(Ordering::SeqCst), 0);
        assert_eq!(OBSERVED_MAP_POINTER.load(Ordering::SeqCst), 0);
        probe.grow(required_map_bytes);
        assert_eq!(probe.capacity(), 192);
        assert_eq!(probe.occupied_map_bytes(), 0);

        // The same custody rebinds after growth; a too-small report that
        // fits the new capacity is a contract violation, witnessed below.
        FAKE_GET_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        script_success(0x5AFE_0042, 96, 48, 1);
        let UefiGetMemoryMapAttemptOutcome::Acquired {
            buffer: probe,
            acquisition,
            ..
        } = acquire_outcome(&firmware, &pending_exit, probe)
        else {
            panic!("the grown buffer must acquire")
        };
        assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_0042);
        assert_eq!(probe.occupied_map_bytes(), 96);
        assert_eq!(OBSERVED_CAPACITY.load(Ordering::SeqCst), 192);
    }

    #[test]
    fn stale_exit_key_reacquires_a_fresh_map_and_retries_to_success() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        script_success(0x5AFE_0043, 96, 48, 1);
        FAKE_EXIT_STATUS.store(EFI_INVALID_PARAMETER, Ordering::SeqCst);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
        let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        let mut buffer = UefiMemoryMapBuffer::with_capacity(128);

        // First attempt: key A exits stale; the handoff retires it and the
        // exit edge returns the complete provider custody for retry.
        let UefiGetMemoryMapAttemptOutcome::Acquired {
            buffer: map,
            acquisition,
            ..
        } = acquire_outcome(&firmware, &pending_exit, &mut buffer)
        else {
            panic!("the first acquisition must succeed")
        };
        let acquired = handoff.acquire_memory_map(arrival, acquisition).unwrap();
        let bound = bind_uefi_exit_boot_services_invocation(pending_exit, &acquired).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        assert_eq!(OBSERVED_EXIT_KEY.load(Ordering::SeqCst), 0x5AFE_0043);
        let UefiExitBootServicesAttemptOutcome::Retry {
            invocation: pending_exit,
            result,
            ..
        } = admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("stale key must return provider custody")
        };
        let UefiOsHandoffProgress::Retry(retry) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("the first stale key must retry")
        };

        // A reacquired map carrying the retired key rejects before custody is
        // spent: the acquisition evidence returns with the arrival intact.
        let UefiGetMemoryMapAttemptOutcome::Acquired {
            buffer: map,
            acquisition,
            ..
        } = acquire_outcome(&firmware, &pending_exit, map)
        else {
            panic!("the second acquisition must succeed")
        };
        let error = handoff.acquire_memory_map(retry, acquisition).unwrap_err();
        assert!(error.diagnostic().0.contains("retired"));
        let (retry, _stale_acquisition, _) = error.into_parts();

        // Firmware's newest key feeds the retried exit and completes.
        script_success(0x5AFE_0044, 96, 48, 1);
        let UefiGetMemoryMapAttemptOutcome::Acquired { acquisition, .. } =
            acquire_outcome(&firmware, &pending_exit, map)
        else {
            panic!("the fresh acquisition must succeed")
        };
        let acquired = handoff.acquire_memory_map(retry, acquisition).unwrap();
        assert_eq!(acquired.map_key().normalized_identity(), 0x5AFE_0044);
        let bound = bind_uefi_exit_boot_services_invocation(pending_exit, &acquired).unwrap();
        FAKE_EXIT_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        // SAFETY: same retained fake service address.
        let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
        assert_eq!(OBSERVED_EXIT_KEY.load(Ordering::SeqCst), 0x5AFE_0044);
        let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
            admit_uefi_exit_boot_services_execution(executed).unwrap()
        else {
            panic!("the retried attempt must exit on success")
        };
        let UefiOsHandoffProgress::Complete(_) = handoff
            .apply_exit_boot_services_result(acquired, result)
            .unwrap()
        else {
            panic!("the retried success must complete the handoff")
        };
        assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn foreign_invocation_and_session_reject_before_acquisition_use() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut owner = ledger(10);
        let foreign = ledger(60);
        let pending_exit = planned_exit(&mut owner, &system, &boot, 13, boot_address);

        // A firmware ledger that does not own the borrowed invocation cannot
        // join the acquisition edge.
        let error = join_lifecycle_scoped_uefi_get_memory_map_provider(&foreign, &pending_exit)
            .unwrap_err();
        assert!(error.0.contains("different or inactive"));

        // Evidence minted under a different physical invocation or firmware
        // session cannot feed this handoff's arrival even when every other
        // identity matches; both legs reject before custody is spent.
        let (mut handoff, mut arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
        for (session, invocation) in [
            (
                handoff.firmware_session(),
                id(999, UefiPhysicalInvocationId::from_normalized_identity),
            ),
            (
                id(999, UefiFirmwareSessionId::from_normalized_identity),
                handoff.physical_invocation(),
            ),
        ] {
            let foreign_acquisition = UefiMemoryMapAcquisition::for_test(
                session,
                invocation,
                id(70, UefiMemoryMapSnapshotId::from_normalized_identity),
                id(71, UefiMemoryMapKeyId::from_normalized_identity),
                96,
                48,
                1,
            );
            let error = handoff
                .acquire_memory_map(arrival, foreign_acquisition)
                .unwrap_err();
            assert!(
                error
                    .diagnostic()
                    .0
                    .contains("different physical invocation")
            );
            let (returned_arrival, _acquisition, _) = error.into_parts();
            arrival = returned_arrival;
        }

        // The same arrival with exact-invocation evidence still acquires.
        script_success(0x5AFE_0045, 96, 48, 1);
        let mut buffer = UefiMemoryMapBuffer::with_capacity(128);
        let UefiGetMemoryMapAttemptOutcome::Acquired { acquisition, .. } =
            acquire_outcome(&owner, &pending_exit, &mut buffer)
        else {
            panic!("the exact-invocation acquisition must succeed")
        };
        let _acquired = handoff.acquire_memory_map(arrival, acquisition).unwrap();
    }

    #[test]
    fn rejected_and_malformed_statuses_retain_executed_custody() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
        let mut buffer = UefiMemoryMapBuffer::with_capacity(128);
        let mut pending = &pending_exit;
        let mut map: &mut UefiMemoryMapBuffer = &mut buffer;

        // InvalidParameter and statuses outside the closed target table both
        // reject with executed custody; each release returns the identical
        // pending-exit borrow and the buffer.
        for (status, fragment) in [
            (EFI_INVALID_PARAMETER, "InvalidParameter"),
            (EFI_STATUS_ERROR_BIT | 9, "closed target table"),
        ] {
            FAKE_GET_STATUS.store(status, Ordering::SeqCst);
            let provider =
                join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
            let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
            let bound = bind_uefi_get_memory_map_invocation(invocation, map).unwrap();
            // SAFETY: the retained service address is the test-local fake.
            let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
            let error = admit_uefi_get_memory_map_execution(executed).unwrap_err();
            assert!(
                error.diagnostic().0.contains(fragment),
                "missing {fragment}"
            );
            let (executed, _) = error.into_parts();
            let (returned_pending, returned_map) = executed.into_pending_exit_invocation();
            assert!(std::ptr::eq(returned_pending, pending));
            pending = returned_pending;
            map = returned_map;
        }

        // A successful status whose sealed cells violate the firmware
        // contract never mints acquisition evidence.
        FAKE_GET_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
        for (key, bytes, descriptor_size, version, fragment) in [
            (0, 96, 48, 1, "zero map key"),
            (0x5AFE_0050, 96, 32, 1, "descriptor geometry"),
            (0x5AFE_0051, 100, 48, 1, "descriptor geometry"),
            (0x5AFE_0052, 256, 48, 1, "empty or over-capacity"),
            (0x5AFE_0053, 96, 48, 0, "zero descriptor version"),
        ] {
            script_success(key, bytes, descriptor_size, version);
            let provider =
                join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
            let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
            let bound = bind_uefi_get_memory_map_invocation(invocation, map).unwrap();
            // SAFETY: the retained service address is the test-local fake.
            let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
            let error = admit_uefi_get_memory_map_execution(executed).unwrap_err();
            assert!(
                error.diagnostic().0.contains(fragment),
                "missing {fragment}"
            );
            let (executed, _) = error.into_parts();
            let (returned_pending, returned_map) = executed.into_pending_exit_invocation();
            pending = returned_pending;
            map = returned_map;
        }

        // BufferTooSmall without a real growth requirement rejects the same
        // way; the post-execution cell-drift check cannot fire because the
        // executed carrier still exclusively holds the buffer between
        // execution and admission.
        FAKE_GET_STATUS.store(EFI_BUFFER_TOO_SMALL, Ordering::SeqCst);
        FAKE_REQUIRED_BYTES.store(64, Ordering::SeqCst);
        let provider =
            join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
        let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
        let bound = bind_uefi_get_memory_map_invocation(invocation, map).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
        let error = admit_uefi_get_memory_map_execution(executed).unwrap_err();
        assert!(
            error
                .diagnostic()
                .0
                .contains("did not report a growth requirement")
        );
        let (executed, _) = error.into_parts();
        let (pending, map) = executed.into_pending_exit_invocation();

        // After every rejection the same custody still drives a clean
        // acquisition.
        script_success(0x5AFE_0054, 96, 48, 1);
        let UefiGetMemoryMapAttemptOutcome::Acquired { acquisition, .. } =
            acquire_outcome(&firmware, pending, map)
        else {
            panic!("custody returned by rejections must still acquire")
        };
        assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_0054);
    }

    #[test]
    fn custody_releases_return_the_identical_pending_exit_invocation() {
        let _firmware = FIRMWARE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = boot_table(
            fake_get_memory_map_address(),
            fake_exit_boot_services_address(),
        );
        let mut firmware = ledger(10);
        let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
        let mut buffer = UefiMemoryMapBuffer::with_capacity(128);

        let provider =
            join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, &pending_exit).unwrap();
        let pending = provider.into_pending_exit_invocation();
        assert!(std::ptr::eq(pending, &pending_exit));

        let provider =
            join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
        let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
        let pending = invocation.into_pending_exit_invocation();
        assert!(std::ptr::eq(pending, &pending_exit));

        let provider =
            join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
        let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
        let bound = bind_uefi_get_memory_map_invocation(invocation, &mut buffer).unwrap();
        let (pending, _buffer) = bound.into_pending_exit_invocation();
        assert!(std::ptr::eq(pending, &pending_exit));
    }

    #[test]
    fn acquisition_issuance_stays_module_family_private() {
        let production = include_str!("get_memory_map.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production GetMemoryMap source");
        let compact = production
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(compact.contains(
            "pubstructUefiMemoryMapAcquisition{session:UefiFirmwareSessionId,invocation:UefiPhysicalInvocationId,"
        ));
        for forbidden in [
            "pubfnfor_test",
            "pub(crate)fnfor_test",
            "implCloneforUefiMemoryMap",
            "implCloneforUefiGetMemoryMap",
            "UefiExitBootServicesProviderResultKind",
        ] {
            assert!(
                !compact.contains(forbidden),
                "forbidden acquisition authority surface appeared: {forbidden}"
            );
        }
    }
}
