//! Lifecycle-bound projection of the UEFI System Table BootServices field.
//!
//! This is exact field correspondence, not callable firmware authority. The
//! raw field value remains private until a later selected provider joins its
//! own table occurrence and service postconditions.

use std::num::NonZeroU64;

use program_entry_plan::exact_uefi_x64_physical_boundary_entry_plan;
use target::{
    TargetProfile, UefiSystemTableNativeField, UefiSystemTableNativeFieldKind,
    UefiSystemTableNativeFieldLayout, plan_uefi_system_table_native_layout,
};

use super::{
    UefiApplicationBootstrapAdapterInvocationReadiness, UefiApplicationBootstrapLedgerId,
    UefiApplicationFirmwareLedger, UefiBootServicesPhaseLeaseId, UefiFirmwareSessionId,
    UefiImageHandleOccurrenceId, UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
};
use crate::ExternalRootDiagnostic;

const BOOT_SERVICES_FIELD_ORDINAL: u8 = 15;
const BOOT_SERVICES_FIELD_OFFSET: u32 = 96;
const BOOT_SERVICES_FIELD_SIZE: u32 = 8;
const BOOT_SERVICES_FIELD_ALIGNMENT: u32 = 8;

/// Exact lifecycle-bound correspondence for the System Table BootServices
/// field. It is deliberately non-clone and exposes report observations only.
#[must_use = "UEFI Boot Services projection retains physical-arrival and phase custody"]
pub struct LifecycleScopedUefiBootServicesProjection<'occurrence> {
    pub(super) readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    pub(super) field: UefiSystemTableNativeFieldLayout,
    pub(super) boot_services_table: NonZeroU64,
}

impl std::fmt::Debug for LifecycleScopedUefiBootServicesProjection<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleScopedUefiBootServicesProjection")
            .field("ledger", &self.ledger_id())
            .field("session", &self.firmware_session())
            .field("invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("system_table_occurrence", &self.system_table_occurrence())
            .field("phase_lease", &self.phase_lease_id())
            .field("field_ordinal", &self.field_ordinal())
            .field("field_byte_offset", &self.field_byte_offset())
            .field("field_byte_size", &self.field_byte_size())
            .field("field_alignment", &self.field_alignment())
            .finish_non_exhaustive()
    }
}

impl LifecycleScopedUefiBootServicesProjection<'_> {
    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.readiness.arrival.ledger_id()
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.readiness.arrival.firmware_session()
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.readiness.arrival.physical_invocation()
    }

    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.readiness.arrival.image_handle_occurrence()
    }

    pub const fn system_table_occurrence(&self) -> UefiSystemTableOccurrenceId {
        self.readiness.arrival.system_table_occurrence()
    }

    pub const fn phase_lease_id(&self) -> UefiBootServicesPhaseLeaseId {
        self.readiness.arrival.system_table.phase_lease_id()
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

    pub fn physical_requirement_identity(&self) -> &str {
        self.readiness
            .arrival
            .physical_contract
            .requirement_identity()
    }

    pub const fn physical_calling_plan_report_fingerprint(&self) -> u64 {
        self.readiness
            .arrival
            .physical_contract
            .calling_plan_report_fingerprint()
    }

    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.readiness
            .arrival
            .system_table
            .layout()
            .non_authoritative_layout_report_fingerprint()
    }
}

/// Recoverable projection failure retaining the complete physical arrival.
#[derive(Debug)]
#[must_use = "UEFI Boot Services projection rejection retains physical-arrival custody"]
pub struct UefiBootServicesProjectionError<'occurrence> {
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiBootServicesProjectionError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
        ExternalRootDiagnostic,
    ) {
        (self.readiness, self.diagnostic)
    }
}

impl std::fmt::Display for UefiBootServicesProjectionError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiBootServicesProjectionError<'_> {}

/// Recoverable release failure retaining the complete field projection.
#[derive(Debug)]
#[must_use = "UEFI Boot Services release rejection retains projection custody"]
pub struct UefiBootServicesProjectionReleaseError<'occurrence> {
    pub(super) projection: LifecycleScopedUefiBootServicesProjection<'occurrence>,
    pub(super) diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiBootServicesProjectionReleaseError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiBootServicesProjection<'occurrence>,
        ExternalRootDiagnostic,
    ) {
        (self.projection, self.diagnostic)
    }
}

impl std::fmt::Display for UefiBootServicesProjectionReleaseError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiBootServicesProjectionReleaseError<'_> {}

/// Consume adapter-invocation readiness and project only its lifecycle-bound
/// BootServices field correspondence. The physical arrival cannot bypass this
/// composition edge, and the selected provider remains a later admission.
pub fn project_uefi_application_boot_services<'occurrence>(
    ledger: &UefiApplicationFirmwareLedger<'occurrence>,
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
) -> Result<
    LifecycleScopedUefiBootServicesProjection<'occurrence>,
    Box<UefiBootServicesProjectionError<'occurrence>>,
> {
    if !ledger.matches_image_handle(&readiness.arrival.image_handle)
        || !ledger.matches_provenance(&readiness.arrival.system_table.provenance)
        || !ledger.matches_lease(&readiness.arrival.system_table.phase_lease)
    {
        return reject(
            readiness,
            "UEFI Boot Services projection belongs to a different or inactive physical invocation",
        );
    }
    if !readiness
        .arrival
        .physical_contract
        .matches_exact_uefi_x64_physical_contract()
    {
        return reject(
            readiness,
            "UEFI Boot Services projection does not retain the exact physical entry contract",
        );
    }
    let expected_physical_plan = exact_uefi_x64_physical_boundary_entry_plan();
    if readiness.physical_calling_plan_commitment()
        != &expected_physical_plan.contract_commitment_digest()
    {
        return reject(
            readiness,
            "UEFI Boot Services projection adapter-readiness commitment drifted",
        );
    }
    let expected_layout = plan_uefi_system_table_native_layout(TargetProfile::UefiX64)
        .expect("the closed UEFI x64 target must retain its system-table layout");
    if !readiness
        .arrival
        .system_table
        .layout()
        .matches_exact_plan(&expected_layout)
    {
        return reject(
            readiness,
            "UEFI Boot Services projection does not retain the exact system-table layout",
        );
    }
    let Some(field) = expected_layout.field_layout(UefiSystemTableNativeField::BootServices) else {
        return reject(
            readiness,
            "UEFI x64 system-table layout has no BootServices field",
        );
    };
    if field.ordinal() != BOOT_SERVICES_FIELD_ORDINAL
        || field.byte_offset() != BOOT_SERVICES_FIELD_OFFSET
        || field.byte_size() != BOOT_SERVICES_FIELD_SIZE
        || field.alignment() != BOOT_SERVICES_FIELD_ALIGNMENT
        || field.kind() != UefiSystemTableNativeFieldKind::Pointer
    {
        return reject(
            readiness,
            "UEFI x64 BootServices field geometry drifted from the target-owned layout",
        );
    }
    let start = field.byte_offset() as usize;
    let end = start + field.byte_size() as usize;
    let Some(bytes) = readiness
        .arrival
        .system_table
        .integrity
        .table_bytes()
        .get(start..end)
    else {
        return reject(
            readiness,
            "UEFI system-table occurrence does not cover the BootServices field",
        );
    };
    let value = u64::from_le_bytes(bytes.try_into().expect("field width was replayed above"));
    let Some(boot_services_table) = NonZeroU64::new(value) else {
        return reject(
            readiness,
            "UEFI BootServices field is null during the Boot-Services-live phase",
        );
    };
    Ok(LifecycleScopedUefiBootServicesProjection {
        readiness,
        field,
        boot_services_table,
    })
}

fn reject<'occurrence>(
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    message: impl Into<String>,
) -> Result<
    LifecycleScopedUefiBootServicesProjection<'occurrence>,
    Box<UefiBootServicesProjectionError<'occurrence>>,
> {
    Err(Box::new(UefiBootServicesProjectionError {
        readiness,
        diagnostic: ExternalRootDiagnostic(message.into()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        UefiApplicationBootstrapLedgerId, UefiApplicationPhysicalArrival,
        UefiBootServicesPhaseLeaseId, UefiFirmwareSessionId, UefiImageHandleOccurrenceId,
        UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
        join_lifecycle_scoped_uefi_system_table, join_uefi_application_physical_arrival,
        prepare_uefi_application_bootstrap_adapter_invocation,
    };
    use program_entry_plan::{
        ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
        UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
    };
    use target::{
        ProgramEntryPhysicalContractPackage, UEFI_SYSTEM_TABLE_SIGNATURE,
        validate_uefi_system_table_occurrence,
    };

    const REVISION: u32 = (2 << 16) | 100;
    const BOOT_SERVICES_TEST_ADDRESS: u64 = 0x0000_0000_0040_1000;

    fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(value).unwrap()
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

    fn physical_contract() -> ProgramEntryPhysicalContractPlan {
        let expected = exact_uefi_x64_physical_boundary_entry_plan();
        ProgramEntryPhysicalContractPlan::new(
            TargetProfile::UefiX64.program_entry_slot(),
            UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
            ProgramEntryPhysicalContractPackage::UefiX64,
            exact_uefi_x64_physical_contract_package_source_digest(),
            0xfeed,
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

    fn system_table_crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for (index, byte) in bytes.iter().copied().enumerate() {
            let byte = if (16..20).contains(&index) { 0 } else { byte };
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let low_bit_mask = 0_u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xedb8_8320 & low_bit_mask);
            }
        }
        !crc
    }

    fn valid_occurrence(boot_services: u64) -> Vec<u8> {
        let mut bytes = vec![0; 120];
        let header_size = bytes.len() as u32;
        bytes[0..8].copy_from_slice(&UEFI_SYSTEM_TABLE_SIGNATURE.to_le_bytes());
        bytes[8..12].copy_from_slice(&REVISION.to_le_bytes());
        bytes[12..16].copy_from_slice(&header_size.to_le_bytes());
        bytes[96..104].copy_from_slice(&boot_services.to_le_bytes());
        let crc = system_table_crc32(&bytes);
        bytes[16..20].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    fn arrival<'a>(
        ledger: &mut UefiApplicationFirmwareLedger<'a>,
        bytes: &'a [u8],
        base: u64,
    ) -> UefiApplicationBootstrapAdapterInvocationReadiness<'a> {
        let image_handle = ledger
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
        let system_table =
            join_lifecycle_scoped_uefi_system_table(ledger, integrity, provenance, lease).unwrap();
        let arrival = join_uefi_application_physical_arrival(
            ledger,
            image_handle,
            system_table,
            physical_contract(),
        )
        .unwrap();
        prepare_uefi_application_bootstrap_adapter_invocation(ledger, arrival).unwrap()
    }

    fn item_block<'a>(source: &'a str, declaration: &str) -> &'a str {
        let start = source.find(declaration).expect("source declaration");
        let body = &source[start..];
        let mut depth = 0_u32;
        let mut opened = false;
        for (index, character) in body.char_indices() {
            match character {
                '{' => {
                    opened = true;
                    depth += 1;
                }
                '}' if opened => {
                    depth -= 1;
                    if depth == 0 {
                        return &body[..=index];
                    }
                }
                _ => {}
            }
        }
        panic!("unterminated source declaration {declaration}")
    }

    fn public_method_names(block: &str) -> Vec<&str> {
        block
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if !line.starts_with("pub ") {
                    return None;
                }
                let function = line.find("fn ")?;
                line[function + 3..].split('(').next()
            })
            .collect()
    }

    #[test]
    fn exact_boot_services_field_projects_under_live_arrival_custody() {
        let bytes = valid_occurrence(BOOT_SERVICES_TEST_ADDRESS);
        let mut ledger = ledger(10);
        let arrival = arrival(&mut ledger, &bytes, 13);
        let projection = project_uefi_application_boot_services(&ledger, arrival).unwrap();

        assert_eq!(projection.ledger_id(), ledger.ledger_id());
        assert_eq!(
            projection.physical_invocation(),
            ledger.physical_invocation()
        );
        assert_eq!(projection.field_ordinal(), BOOT_SERVICES_FIELD_ORDINAL);
        assert_eq!(projection.field_byte_offset(), BOOT_SERVICES_FIELD_OFFSET);
        assert_eq!(projection.field_byte_size(), BOOT_SERVICES_FIELD_SIZE);
        assert_eq!(projection.field_alignment(), BOOT_SERVICES_FIELD_ALIGNMENT);
        assert_eq!(
            projection.physical_requirement_identity(),
            UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY
        );
        assert_ne!(projection.non_authoritative_layout_report_fingerprint(), 0);

        let released = ledger
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap();
        assert_eq!(released.ledger, ledger.ledger_id());
        ledger.begin_firmware_return().unwrap();
    }

    #[test]
    fn null_boot_services_rejects_and_returns_complete_arrival_for_retry() {
        let null_bytes = valid_occurrence(0);
        let mut ledger = ledger(30);
        let arrival = arrival(&mut ledger, &null_bytes, 33);
        let error = project_uefi_application_boot_services(&ledger, arrival).unwrap_err();
        assert!(error.diagnostic().0.contains("is null"));
        let (readiness, _) = error.into_parts();
        assert_eq!(readiness.ledger_id(), ledger.ledger_id());
        assert_eq!(
            readiness.image_handle_occurrence().normalized_identity(),
            33
        );

        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn foreign_ledger_rejects_without_consuming_projection_inputs() {
        let bytes = valid_occurrence(BOOT_SERVICES_TEST_ADDRESS);
        let mut owner = ledger(50);
        let foreign = ledger(50);
        let arrival = arrival(&mut owner, &bytes, 53);

        let error = project_uefi_application_boot_services(&foreign, arrival).unwrap_err();
        assert!(error.diagnostic().0.contains("different or inactive"));
        let (readiness, _) = error.into_parts();
        let projection = project_uefi_application_boot_services(&owner, readiness).unwrap();
        owner
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap();
    }

    #[test]
    fn adapter_readiness_rejects_foreign_ledger_and_preserves_arrival_for_owner_retry() {
        let bytes = valid_occurrence(BOOT_SERVICES_TEST_ADDRESS);
        let mut owner = ledger(60);
        let foreign = ledger(60);
        let readiness = arrival(&mut owner, &bytes, 63);
        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;

        let error =
            prepare_uefi_application_bootstrap_adapter_invocation(&foreign, arrival).unwrap_err();
        assert!(error.diagnostic().0.contains("different or inactive"));
        let (arrival, _) = error.into_parts();
        let readiness =
            prepare_uefi_application_bootstrap_adapter_invocation(&owner, arrival).unwrap();
        let projection = project_uefi_application_boot_services(&owner, readiness).unwrap();
        owner
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap();
    }

    #[test]
    fn adapter_readiness_commitment_tamper_rejects_and_preserves_retry_custody() {
        let bytes = valid_occurrence(BOOT_SERVICES_TEST_ADDRESS);
        let mut ledger = ledger(65);
        let mut readiness = arrival(&mut ledger, &bytes, 68);
        readiness.physical_calling_plan_commitment = [0x5a; 32];

        let error = project_uefi_application_boot_services(&ledger, readiness).unwrap_err();
        assert!(error.diagnostic().0.contains("commitment drifted"));
        let (mut readiness, _) = error.into_parts();
        readiness.physical_calling_plan_commitment =
            exact_uefi_x64_physical_boundary_entry_plan().contract_commitment_digest();
        let projection = project_uefi_application_boot_services(&ledger, readiness).unwrap();
        ledger
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap();
    }

    #[test]
    fn projection_keeps_firmware_return_blocked_until_consuming_release() {
        let bytes = valid_occurrence(BOOT_SERVICES_TEST_ADDRESS);
        let mut ledger = ledger(70);
        let arrival = arrival(&mut ledger, &bytes, 73);
        let projection = project_uefi_application_boot_services(&ledger, arrival).unwrap();

        assert!(
            ledger
                .begin_firmware_return()
                .unwrap_err()
                .0
                .contains("lease")
        );
        ledger
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap();
        ledger.begin_firmware_return().unwrap();
    }

    #[test]
    fn foreign_release_rejects_and_returns_complete_projection_for_owner_retry() {
        let bytes = valid_occurrence(BOOT_SERVICES_TEST_ADDRESS);
        let mut owner = ledger(90);
        let mut foreign = ledger(90);
        let arrival = arrival(&mut owner, &bytes, 93);
        let projection = project_uefi_application_boot_services(&owner, arrival).unwrap();

        let error = foreign
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap_err();
        assert!(error.diagnostic().0.contains("different firmware ledger"));
        let (projection, _) = error.into_parts();
        assert_eq!(projection.ledger_id(), owner.ledger_id());
        owner
            .release_lifecycle_scoped_boot_services_projection(projection)
            .unwrap();
        owner.begin_firmware_return().unwrap();
    }

    #[test]
    fn projection_public_surface_exposes_no_raw_value_or_provider_authority() {
        let source = include_str!("provider_projection.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production projection source");
        let compact = source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        let projection = item_block(
            source,
            "pub struct LifecycleScopedUefiBootServicesProjection<'occurrence>",
        );
        let compact_projection = projection
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert_eq!(
            compact_projection,
            "pubstructLifecycleScopedUefiBootServicesProjection<'occurrence>{pub(super)readiness:UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,pub(super)field:UefiSystemTableNativeFieldLayout,pub(super)boot_services_table:NonZeroU64,}"
        );
        let public_impl = item_block(source, "impl LifecycleScopedUefiBootServicesProjection<'_>");
        let compact_public_impl = public_impl
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert_eq!(
            public_method_names(public_impl),
            [
                "ledger_id",
                "firmware_session",
                "physical_invocation",
                "image_handle_occurrence",
                "system_table_occurrence",
                "phase_lease_id",
                "field_ordinal",
                "field_byte_offset",
                "field_byte_size",
                "field_alignment",
                "physical_requirement_identity",
                "physical_calling_plan_report_fingerprint",
                "non_authoritative_layout_report_fingerprint",
            ]
        );
        for forbidden in [
            "pubfnraw",
            "pubconstfnraw",
            "pubfnaddress",
            "pubconstfnaddress",
            "pubfnbytes",
            "pubconstfnbytes",
            "psi_extents::Extent",
            "implFrom<LifecycleScopedUefiBootServicesProjection",
            "implInto<LifecycleScopedUefiBootServicesProjection",
        ] {
            assert!(
                !compact_public_impl.contains(forbidden),
                "forbidden projection API appeared: {forbidden}"
            );
        }
        assert!(!compact.contains("implCloneforLifecycleScopedUefiBootServicesProjection"));
    }
}
