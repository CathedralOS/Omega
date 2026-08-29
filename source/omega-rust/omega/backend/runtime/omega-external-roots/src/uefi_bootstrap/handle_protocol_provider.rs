//! Lifecycle-scoped UEFI `HandleProtocol` correspondence.
//!
//! This rung joins the System Table's private BootServices pointer to one
//! exact, CRC-validated `EFI_BOOT_SERVICES` occurrence and its target-owned
//! `HandleProtocol` row. A provider-success admission can then establish
//! Loaded Image base/size correspondence for the exact physical image-handle
//! occurrence. No raw pointer is exposed or invoked here, and no `Extent`,
//! semantic root, shell, adapter, installation, or native execution is
//! created.

use std::num::NonZeroU64;

use omega_target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, ValidatedUefiBootServicesHeaderIntegrity,
    plan_uefi_boot_services_native_layout,
};

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

/// Stable structured identity of the UEFI Loaded Image protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiProtocolGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub const UEFI_LOADED_IMAGE_PROTOCOL_GUID: UefiProtocolGuid = UefiProtocolGuid {
    data1: 0x5b1b_31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
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
    _handle_protocol: NonZeroU64,
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
    if !ledger.matches_image_handle(&projection.arrival.image_handle)
        || !ledger.matches_provenance(&projection.arrival.system_table.provenance)
        || !ledger.matches_lease(&projection.arrival.system_table.phase_lease)
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
        _handle_protocol: handle_protocol,
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

/// Closed provider outcome admitted for the exact Loaded Image query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UefiHandleProtocolLoadedImageOutcome {
    Success {
        interface_address: u64,
        image_base: u64,
        image_size: u64,
    },
    Unsupported,
    InvalidParameter,
}

/// Non-root correspondence established by one admitted successful provider
/// outcome for the exact physical image handle and Loaded Image GUID.
#[must_use = "Loaded Image correspondence retains HandleProtocol provider and physical custody"]
pub struct LifecycleScopedUefiLoadedImageCorrespondence<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    _interface_address: NonZeroU64,
    image_base: NonZeroU64,
    image_size: NonZeroU64,
}

impl std::fmt::Debug for LifecycleScopedUefiLoadedImageCorrespondence<'_, '_> {
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

impl LifecycleScopedUefiLoadedImageCorrespondence<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.provider.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.provider.image_handle_occurrence()
    }
    pub const fn protocol(&self) -> UefiProtocolGuid {
        UEFI_LOADED_IMAGE_PROTOCOL_GUID
    }
    pub const fn image_base(&self) -> u64 {
        self.image_base.get()
    }
    pub const fn image_size(&self) -> u64 {
        self.image_size.get()
    }
    pub const fn image_end_exclusive(&self) -> u64 {
        self.image_base.get() + self.image_size.get()
    }
}

#[derive(Debug)]
#[must_use = "UEFI HandleProtocol call rejection retains provider custody"]
pub struct UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    outcome: UefiHandleProtocolLoadedImageOutcome,
    diagnostic: ExternalRootDiagnostic,
}
impl<'system_table, 'boot_services>
    UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services>
{
    pub const fn outcome(&self) -> UefiHandleProtocolLoadedImageOutcome {
        self.outcome
    }
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
        UefiHandleProtocolLoadedImageOutcome,
        ExternalRootDiagnostic,
    ) {
        (self.provider, self.outcome, self.diagnostic)
    }
}
impl std::fmt::Display for UefiHandleProtocolLoadedImageCallError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiHandleProtocolLoadedImageCallError<'_, '_> {}

/// Consume one prepared provider occurrence and admit its exact Loaded Image
/// result. Success validates non-null, nonempty, non-wrapping geometry; all
/// other outcomes return the complete provider for retry or release.
pub fn admit_uefi_loaded_image_handle_protocol_outcome<'system_table, 'boot_services>(
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    outcome: UefiHandleProtocolLoadedImageOutcome,
) -> Result<
    LifecycleScopedUefiLoadedImageCorrespondence<'system_table, 'boot_services>,
    Box<UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services>>,
> {
    let UefiHandleProtocolLoadedImageOutcome::Success {
        interface_address,
        image_base,
        image_size,
    } = outcome
    else {
        return reject_call(
            provider,
            outcome,
            "UEFI HandleProtocol did not return Loaded Image correspondence",
        );
    };
    let Some(interface_address) = NonZeroU64::new(interface_address) else {
        return reject_call(
            provider,
            outcome,
            "UEFI HandleProtocol success returned a null Loaded Image interface",
        );
    };
    let Some(image_base) = NonZeroU64::new(image_base) else {
        return reject_call(
            provider,
            outcome,
            "UEFI Loaded Image success returned a null image base",
        );
    };
    let Some(image_size) = NonZeroU64::new(image_size) else {
        return reject_call(
            provider,
            outcome,
            "UEFI Loaded Image success returned an empty image",
        );
    };
    if image_base.get().checked_add(image_size.get()).is_none() {
        return reject_call(
            provider,
            outcome,
            "UEFI Loaded Image geometry wraps the u64 address carrier",
        );
    }
    Ok(LifecycleScopedUefiLoadedImageCorrespondence {
        provider,
        _interface_address: interface_address,
        image_base,
        image_size,
    })
}

fn reject_call<'system_table, 'boot_services>(
    provider: LifecycleScopedUefiHandleProtocolProvider<'system_table, 'boot_services>,
    outcome: UefiHandleProtocolLoadedImageOutcome,
    message: impl Into<String>,
) -> Result<
    LifecycleScopedUefiLoadedImageCorrespondence<'system_table, 'boot_services>,
    Box<UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services>>,
> {
    Err(Box::new(UefiHandleProtocolLoadedImageCallError {
        provider,
        outcome,
        diagnostic: ExternalRootDiagnostic(message.into()),
    }))
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
        if !self.matches_image_handle(&provider.projection.arrival.image_handle)
            || !self.matches_provenance(&provider.projection.arrival.system_table.provenance)
            || !self.matches_lease(&provider.projection.arrival.system_table.phase_lease)
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

    pub fn release_lifecycle_scoped_loaded_image_correspondence<'boot_services>(
        &mut self,
        correspondence: LifecycleScopedUefiLoadedImageCorrespondence<'system_table, 'boot_services>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiHandleProtocolProviderReleaseError<'system_table, 'boot_services>>,
    > {
        self.release_lifecycle_scoped_handle_protocol_provider(correspondence.provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId, UefiFirmwareSessionId,
        UefiSystemTableOccurrenceId, join_lifecycle_scoped_uefi_system_table,
        join_uefi_application_physical_arrival, project_uefi_application_boot_services,
    };
    use omega_program_entry_plan::{
        ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
        UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
    };
    use omega_target::{
        ProgramEntryPhysicalContractPackage, UEFI_BOOT_SERVICES_SIGNATURE,
        UEFI_SYSTEM_TABLE_SIGNATURE, plan_uefi_system_table_native_layout,
        validate_uefi_boot_services_occurrence, validate_uefi_system_table_occurrence,
    };

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
        let image = ledger
            .admit_image_handle_occurrence(id(
                base,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
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
        project_uefi_application_boot_services(ledger, arrival).unwrap()
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

    #[test]
    fn exact_handle_protocol_success_establishes_non_root_loaded_image_correspondence() {
        let boot_address = 0x401000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x402000);
        let mut ledger = ledger(10);
        let projection = projection(&mut ledger, &system, 20);
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
                23,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap();
        assert_eq!(provider.field_byte_offset(), 152);
        assert_eq!(provider.protocol(), UEFI_LOADED_IMAGE_PROTOCOL_GUID);
        let loaded = admit_uefi_loaded_image_handle_protocol_outcome(
            provider,
            UefiHandleProtocolLoadedImageOutcome::Success {
                interface_address: 0x403000,
                image_base: 0x100000,
                image_size: 0x20000,
            },
        )
        .unwrap();
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
    fn failed_or_malformed_outcomes_return_provider_for_retry() {
        let boot_address = 0x601000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x602000);
        let mut ledger = ledger(100);
        let projection = projection(&mut ledger, &system, 110);
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
                113,
                UefiBootServicesTableOccurrenceId::from_normalized_identity,
            ),
            NonZeroU64::new(boot_address).unwrap(),
        )
        .unwrap();
        let error = admit_uefi_loaded_image_handle_protocol_outcome(
            provider,
            UefiHandleProtocolLoadedImageOutcome::Unsupported,
        )
        .unwrap_err();
        assert_eq!(
            error.outcome(),
            UefiHandleProtocolLoadedImageOutcome::Unsupported
        );
        let (provider, _, _) = error.into_parts();
        let outcome = UefiHandleProtocolLoadedImageOutcome::Success {
            interface_address: 1,
            image_base: u64::MAX - 1,
            image_size: 2,
        };
        let error = admit_uefi_loaded_image_handle_protocol_outcome(provider, outcome).unwrap_err();
        assert!(error.diagnostic().0.contains("wraps"));
        let (provider, _, _) = error.into_parts();
        ledger
            .release_lifecycle_scoped_handle_protocol_provider(provider)
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
        ] {
            assert!(
                !compact.contains(forbidden),
                "forbidden provider API appeared: {forbidden}"
            );
        }
        assert!(!compact.contains("implCloneforLifecycleScopedUefiHandleProtocolProvider"));
        assert!(!compact.contains("implCloneforLifecycleScopedUefiLoadedImageCorrespondence"));
    }
}
