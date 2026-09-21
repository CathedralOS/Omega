//! Target-owned native layout of the UEFI x86-64 Boot Services table.
//!
//! The UEFI specification fixes one `EFI_TABLE_HEADER` followed by forty-four
//! function-pointer slots. The authored schema and its evaluated layout policy
//! in `std/targets/uefi_x86_64/tables.omg` own that geometry; this module
//! retains the resulting x86-64 offsets as descriptive evidence replayed from
//! that evaluated plan. It does not inspect a table occurrence, validate a
//! firmware header or CRC, install a provider, or grant authority to invoke
//! any retained function pointer.
//!
//! `replayed_uefi_x64_boot_services_native_layout` binds one retained
//! `LayoutPlanReport` to the recorded source-minted commitments, and
//! `exact_uefi_x64_boot_services_native_layout` is the fixture materialization
//! below the build layer; every validated layout is produced by replaying it
//! through the commitments below.

pub(crate) mod occurrence;

use crate::{
    Architecture, ObjectFormat, ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntryReceiverProvisioning, ProgramEntrySchema, ProgramEntrySlotDeclaration,
    ProgramEntryVisibleParameters, TargetProfile,
};
use diagnostics::Diagnostic;
use layout_plans::{
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    normalized_layout_plan_report_fingerprint,
};

/// The EFI Loaded Image protocol GUID, retained for provider identity.
pub static UEFI_LOADED_IMAGE_PROTOCOL_GUID: UefiProtocolGuid = UefiProtocolGuid {
    data1: 0x5b1b_31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

/// Byte-encoded UEFI protocol GUID carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiProtocolGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

const FIELD_COUNT: usize = 49;
const SERVICE_FIELD_COUNT: usize = 44;
const TABLE_HEADER_SIZE: u32 = 24;
const TABLE_SIZE: u32 = 376;
const TABLE_ALIGNMENT: u32 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Recorded commitment to the authored `EfiBootServicesView` schema report
/// fingerprint. Every replayed plan must come from that source-minted schema.
pub const UEFI_X64_BOOT_SERVICES_SCHEMA_REPORT_FINGERPRINT: u64 = 0xa61c_19ce_5c21_d68f;

/// Compact commitment to the validated `LayoutPlanReport` the authored
/// `EfiBootServicesViewLayout::plan` policy produces for `EfiBootServicesView`.
pub const UEFI_X64_BOOT_SERVICES_LAYOUT_PLAN_COMMITMENT: u64 = 0x3882_01de_e60d_724d;

/// Compact commitment to the validated native field layout the evaluated plan
/// replayed rows must hash to this identity, so the retained field vocabulary
/// cannot drift silently from the source-authored geometry.
pub const UEFI_X64_BOOT_SERVICES_NATIVE_LAYOUT_COMMITMENT: u64 = 0x5078_c8e1_c605_a12e;

/// The target byte offsets the authored `EfiBootServicesViewLayout::plan` produces
/// for `EfiBootServicesView`'s two semantic members, in declaration order.
const EXACT_SEMANTIC_OFFSETS: [u64; 2] = [0, 24];

/// The semantic member names `EfiBootServicesView` declares: the shared
/// `EfiTableHeader` preamble followed by the fixed `services` pointer block.
const SEMANTIC_FIELDS: [&str; 2] = ["header", "services"];

/// Closed field identity for the UEFI x86-64 `EFI_BOOT_SERVICES` layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum UefiBootServicesNativeField {
    HeaderSignature = 1,
    HeaderRevision,
    HeaderSize,
    HeaderCrc32,
    HeaderReserved,
    RaiseTpl,
    RestoreTpl,
    AllocatePages,
    FreePages,
    GetMemoryMap,
    AllocatePool,
    FreePool,
    CreateEvent,
    SetTimer,
    WaitForEvent,
    SignalEvent,
    CloseEvent,
    CheckEvent,
    InstallProtocolInterface,
    ReinstallProtocolInterface,
    UninstallProtocolInterface,
    HandleProtocol,
    Reserved,
    RegisterProtocolNotify,
    LocateHandle,
    LocateDevicePath,
    InstallConfigurationTable,
    LoadImage,
    StartImage,
    Exit,
    UnloadImage,
    ExitBootServices,
    GetNextMonotonicCount,
    Stall,
    SetWatchdogTimer,
    ConnectController,
    DisconnectController,
    OpenProtocol,
    CloseProtocol,
    OpenProtocolInformation,
    ProtocolsPerHandle,
    LocateHandleBuffer,
    LocateProtocol,
    InstallMultipleProtocolInterfaces,
    UninstallMultipleProtocolInterfaces,
    CalculateCrc32,
    CopyMem,
    SetMem,
    CreateEventEx,
}

/// Native representation class of one retained field row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UefiBootServicesNativeFieldKind {
    UnsignedInteger = 1,
    FunctionPointer = 2,
    ReservedZero = 3,
    ReservedPointer = 4,
}

/// Exact target-relative geometry of one boot-services field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiBootServicesNativeFieldLayout {
    field: UefiBootServicesNativeField,
    ordinal: u8,
    byte_offset: u32,
    byte_size: u32,
    alignment: u32,
    kind: UefiBootServicesNativeFieldKind,
}

impl UefiBootServicesNativeFieldLayout {
    pub const fn field(self) -> UefiBootServicesNativeField {
        self.field
    }
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }
    pub const fn byte_offset(self) -> u32 {
        self.byte_offset
    }
    pub const fn byte_size(self) -> u32 {
        self.byte_size
    }
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
    pub const fn kind(self) -> UefiBootServicesNativeFieldKind {
        self.kind
    }
}

/// Independently replayed target-owned layout evidence for one UEFI x86-64
/// Boot Services table.
///
/// The exact target entry slot remains bound to the closed field catalog. This
/// non-clone carrier grants no table occurrence, pointer provenance, firmware
/// lifecycle, provider, bootstrap-shell, semantic-root, or native-execution
/// authority.
#[derive(Debug)]
#[must_use = "validated UEFI Boot Services layout retains target-owned entry identity"]
pub struct ValidatedUefiBootServicesNativeLayout {
    profile: TargetProfile,
    entry_slot: ProgramEntrySlotDeclaration,
    fields: Vec<UefiBootServicesNativeFieldLayout>,
    non_authoritative_layout_report_fingerprint: u64,
}

impl ValidatedUefiBootServicesNativeLayout {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }
    pub const fn entry_slot(&self) -> ProgramEntrySlotDeclaration {
        self.entry_slot
    }
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
    pub const fn table_header_size(&self) -> u32 {
        TABLE_HEADER_SIZE
    }
    pub const fn known_prefix_byte_size(&self) -> u32 {
        TABLE_SIZE
    }
    pub const fn alignment(&self) -> u32 {
        TABLE_ALIGNMENT
    }
    pub fn field_layout(
        &self,
        field: UefiBootServicesNativeField,
    ) -> Option<UefiBootServicesNativeFieldLayout> {
        self.fields.iter().copied().find(|row| row.field == field)
    }
    /// Compatibility accessor for the compact layout report fingerprint.
    /// Runtime admission must use [`Self::matches_exact_plan`] instead.
    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }
    /// Replay the complete target slot and every exact native-layout row.
    /// The compact report fingerprint is deliberately not consulted.
    pub fn matches_exact_plan(&self, expected: &Self) -> bool {
        self.profile == expected.profile
            && self.entry_slot == expected.entry_slot
            && self.fields == expected.fields
    }
}

/// Rejected native-layout planning with the requested target profile retained.
#[derive(Debug)]
#[must_use = "UEFI Boot Services layout rejection retains the requested profile"]
pub struct UefiBootServicesNativeLayoutError {
    profile: TargetProfile,
    diagnostic: Diagnostic,
}

impl UefiBootServicesNativeLayoutError {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
    pub fn into_parts(self) -> (TargetProfile, Diagnostic) {
        (self.profile, self.diagnostic)
    }
}

impl std::fmt::Display for UefiBootServicesNativeLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiBootServicesNativeLayoutError {}

/// Replay one evaluated `EfiBootServicesViewLayout::plan` report into the
/// validated native layout. The report must carry the recorded source-minted
/// schema and plan commitments and declare the authored `header`/`services`
/// pair; a foreign report, a drifted plan, or a block that no longer covers
/// the 376-byte aggregate rejects. The service-pointer rows are the evaluated
/// `services` extent's named slots -- any other interior geometry is foreign
/// and rejects.
pub fn replayed_uefi_x64_boot_services_native_layout(
    report: &LayoutPlanReport,
) -> Option<ValidatedUefiBootServicesNativeLayout> {
    let profile = TargetProfile::UefiX64;
    let entry_slot = profile.program_entry_slot();
    if normalized_layout_plan_report_fingerprint(report)
        != UEFI_X64_BOOT_SERVICES_LAYOUT_PLAN_COMMITMENT
        || report.schema_report_fingerprint != UEFI_X64_BOOT_SERVICES_SCHEMA_REPORT_FINGERPRINT
        || validate_target_owner(profile, entry_slot).is_err()
    {
        return None;
    }
    let fields = fields_from_evaluated_plan(report)?;
    if validate_fields(&fields).is_err() {
        return None;
    }
    let fingerprint = layout_report_fingerprint(entry_slot, &fields);
    if fingerprint != UEFI_X64_BOOT_SERVICES_NATIVE_LAYOUT_COMMITMENT {
        return None;
    }
    Some(ValidatedUefiBootServicesNativeLayout {
        profile,
        entry_slot,
        non_authoritative_layout_report_fingerprint: fingerprint,
        fields,
    })
}

/// Materialize the native layout for contract fixtures below the build layer,
/// where checked-tree evaluation is unavailable. The report shape mirrors what
/// `EfiBootServicesViewLayout::plan` produces for `EfiBootServicesView` and is
/// replayed through the same commitments, so it cannot drift silently from the
/// authored policy.
pub fn exact_uefi_x64_boot_services_native_layout() -> ValidatedUefiBootServicesNativeLayout {
    replayed_uefi_x64_boot_services_native_layout(&exact_uefi_x64_boot_services_layout_plan_report())
        .expect(
            "the authored UEFI x64 Boot Services layout must replay its source-minted commitments",
        )
}

/// The report the authored policy produces, in `LayoutPlanReport` form. This
/// is the one residual literal recipe; consumers must route it through
/// `replayed_uefi_x64_boot_services_native_layout`, never read it directly.
pub fn exact_uefi_x64_boot_services_layout_plan_report() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: UEFI_X64_BOOT_SERVICES_SCHEMA_REPORT_FINGERPRINT,
        entries: SEMANTIC_FIELDS
            .iter()
            .zip(EXACT_SEMANTIC_OFFSETS)
            .map(|(&name, offset)| LayoutFieldEntryReport {
                field: name.to_owned(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset },
            })
            .collect(),
        offsets: Some(EXACT_SEMANTIC_OFFSETS.to_vec()),
        size: Some(u64::from(TABLE_SIZE)),
        align: u64::from(TABLE_ALIGNMENT),
    }
}

/// Target-profile gate retained for consumers below the build layer.
/// `EFI_BOOT_SERVICES` native layout is owned only by the UEFI x86-64 target;
/// any other profile rejects rather than inheriting firmware structure from
/// its shared x86-64 architecture.
pub fn plan_uefi_boot_services_native_layout(
    profile: TargetProfile,
) -> Result<ValidatedUefiBootServicesNativeLayout, Box<UefiBootServicesNativeLayoutError>> {
    let entry_slot = profile.program_entry_slot();
    if let Err(diagnostic) = validate_target_owner(profile, entry_slot) {
        return Err(Box::new(UefiBootServicesNativeLayoutError {
            profile,
            diagnostic,
        }));
    }
    Ok(exact_uefi_x64_boot_services_native_layout())
}

fn fields_from_evaluated_plan(
    report: &LayoutPlanReport,
) -> Option<Vec<UefiBootServicesNativeFieldLayout>> {
    if report.entries.len() != SEMANTIC_FIELDS.len() {
        return None;
    }
    let (header, rest) = report.entries.split_first()?;
    let [services] = rest else { return None };
    if header.field != "header" || services.field != "services" {
        return None;
    }
    let header_offset = match header.placement {
        LayoutPlacementReport::At { offset } => u32::try_from(offset).ok()?,
        _ => return None,
    };
    if header_offset != 0 {
        return None;
    }
    let services_offset = match services.placement {
        LayoutPlacementReport::At { offset } => u32::try_from(offset).ok()?,
        _ => return None,
    };
    if services_offset != header_offset + TABLE_HEADER_SIZE {
        return None;
    }
    let mut fields = Vec::with_capacity(FIELD_COUNT);
    for &(offset, width, kind) in &HEADER_ROWS {
        fields.push(row(
            header_row_field(offset),
            fields.len() as u8,
            header_offset + offset,
            width,
            width,
            kind,
        ));
    }
    for (index, service) in SERVICE_FIELDS.iter().enumerate() {
        let offset = services_offset.checked_add(index as u32 * 8)?;
        fields.push(row(
            *service,
            fields.len() as u8,
            offset,
            8,
            8,
            if *service == UefiBootServicesNativeField::Reserved {
                Kind::ReservedPointer
            } else {
                Kind::FunctionPointer
            },
        ));
    }
    Some(fields)
}

/// The `EfiTableHeader` preamble's retained native rows, replayed from the
/// evaluated `header` member's placement: `(member offset, width, kind)` in
/// declaration order.
const HEADER_ROWS: [(u32, u32, UefiBootServicesNativeFieldKind); 5] = [
    (0, 8, Kind::UnsignedInteger),
    (8, 4, Kind::UnsignedInteger),
    (12, 4, Kind::UnsignedInteger),
    (16, 4, Kind::UnsignedInteger),
    (20, 4, Kind::ReservedZero),
];

const fn header_row_field(offset: u32) -> UefiBootServicesNativeField {
    match offset {
        0 => UefiBootServicesNativeField::HeaderSignature,
        8 => UefiBootServicesNativeField::HeaderRevision,
        12 => UefiBootServicesNativeField::HeaderSize,
        16 => UefiBootServicesNativeField::HeaderCrc32,
        _ => UefiBootServicesNativeField::HeaderReserved,
    }
}

/// The retained native vocabulary of the forty-four service-pointer slots the
/// authored `services` member covers, in ABI order.
const SERVICE_FIELDS: [UefiBootServicesNativeField; SERVICE_FIELD_COUNT] = [
    UefiBootServicesNativeField::RaiseTpl,
    UefiBootServicesNativeField::RestoreTpl,
    UefiBootServicesNativeField::AllocatePages,
    UefiBootServicesNativeField::FreePages,
    UefiBootServicesNativeField::GetMemoryMap,
    UefiBootServicesNativeField::AllocatePool,
    UefiBootServicesNativeField::FreePool,
    UefiBootServicesNativeField::CreateEvent,
    UefiBootServicesNativeField::SetTimer,
    UefiBootServicesNativeField::WaitForEvent,
    UefiBootServicesNativeField::SignalEvent,
    UefiBootServicesNativeField::CloseEvent,
    UefiBootServicesNativeField::CheckEvent,
    UefiBootServicesNativeField::InstallProtocolInterface,
    UefiBootServicesNativeField::ReinstallProtocolInterface,
    UefiBootServicesNativeField::UninstallProtocolInterface,
    UefiBootServicesNativeField::HandleProtocol,
    UefiBootServicesNativeField::Reserved,
    UefiBootServicesNativeField::RegisterProtocolNotify,
    UefiBootServicesNativeField::LocateHandle,
    UefiBootServicesNativeField::LocateDevicePath,
    UefiBootServicesNativeField::InstallConfigurationTable,
    UefiBootServicesNativeField::LoadImage,
    UefiBootServicesNativeField::StartImage,
    UefiBootServicesNativeField::Exit,
    UefiBootServicesNativeField::UnloadImage,
    UefiBootServicesNativeField::ExitBootServices,
    UefiBootServicesNativeField::GetNextMonotonicCount,
    UefiBootServicesNativeField::Stall,
    UefiBootServicesNativeField::SetWatchdogTimer,
    UefiBootServicesNativeField::ConnectController,
    UefiBootServicesNativeField::DisconnectController,
    UefiBootServicesNativeField::OpenProtocol,
    UefiBootServicesNativeField::CloseProtocol,
    UefiBootServicesNativeField::OpenProtocolInformation,
    UefiBootServicesNativeField::ProtocolsPerHandle,
    UefiBootServicesNativeField::LocateHandleBuffer,
    UefiBootServicesNativeField::LocateProtocol,
    UefiBootServicesNativeField::InstallMultipleProtocolInterfaces,
    UefiBootServicesNativeField::UninstallMultipleProtocolInterfaces,
    UefiBootServicesNativeField::CalculateCrc32,
    UefiBootServicesNativeField::CopyMem,
    UefiBootServicesNativeField::SetMem,
    UefiBootServicesNativeField::CreateEventEx,
];

fn validate_target_owner(
    profile: TargetProfile,
    entry_slot: ProgramEntrySlotDeclaration,
) -> Result<(), Diagnostic> {
    let target = profile.native_target();
    require(
        profile == TargetProfile::UefiX64
            && target.architecture == Architecture::X86_64
            && target.object_format == ObjectFormat::Coff
            && target.pointer_size == 8
            && target.pointer_alignment == 8,
        "EFI_BOOT_SERVICES native layout is owned only by the UEFI x86-64 target",
    )?;
    let expected = TargetProfile::UefiX64.program_entry_slot();
    require(
        entry_slot == expected
            && entry_slot.physical_arrival_requirement == Some("UefiPhysicalEntry::enter")
            && entry_slot.physical_contract_package
                == Some(ProgramEntryPhysicalContractPackage::UefiX64)
            && entry_slot.physical_calling_convention
                == Some(ProgramEntryCallingConvention::MicrosoftX64),
        "EFI_BOOT_SERVICES native layout drifted from its exact target-owned physical entry",
    )
}

fn validate_fields(fields: &[UefiBootServicesNativeFieldLayout]) -> Result<(), Diagnostic> {
    require(
        fields.len() == FIELD_COUNT,
        "EFI_BOOT_SERVICES native field catalog is missing, duplicated, reordered, or drifted",
    )?;
    let mut prior_end = 0;
    for (ordinal, row) in fields.iter().enumerate() {
        require(
            usize::from(row.ordinal) == ordinal
                && row.byte_size != 0
                && row.alignment.is_power_of_two()
                && row.byte_offset % row.alignment == 0
                && row.byte_offset >= prior_end,
            "EFI_BOOT_SERVICES native field order, alignment, or geometry is invalid",
        )?;
        prior_end = row
            .byte_offset
            .checked_add(row.byte_size)
            .ok_or_else(|| Diagnostic::error("EFI_BOOT_SERVICES field end overflows u32"))?;
    }
    require(
        prior_end == TABLE_SIZE,
        "EFI_BOOT_SERVICES field catalog does not exactly cover the aggregate extent",
    )
}

const fn row(
    field: UefiBootServicesNativeField,
    ordinal: u8,
    byte_offset: u32,
    byte_size: u32,
    alignment: u32,
    kind: UefiBootServicesNativeFieldKind,
) -> UefiBootServicesNativeFieldLayout {
    UefiBootServicesNativeFieldLayout {
        field,
        ordinal,
        byte_offset,
        byte_size,
        alignment,
        kind,
    }
}

use UefiBootServicesNativeFieldKind as Kind;

fn layout_report_fingerprint(
    entry_slot: ProgramEntrySlotDeclaration,
    fields: &[UefiBootServicesNativeFieldLayout],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.uefi-x64-boot-services-native-layout.v1");
    hash_entry_slot(&mut hash, entry_slot);
    for row in fields {
        hash.byte(row.field as u8);
        hash.byte(row.ordinal);
        hash.bytes(&row.byte_offset.to_le_bytes());
        hash.bytes(&row.byte_size.to_le_bytes());
        hash.bytes(&row.alignment.to_le_bytes());
        hash.byte(row.kind as u8);
    }
    hash.bytes(&TABLE_HEADER_SIZE.to_le_bytes());
    hash.bytes(&TABLE_SIZE.to_le_bytes());
    hash.bytes(&TABLE_ALIGNMENT.to_le_bytes());
    hash.finish()
}

fn hash_entry_slot(hash: &mut Fnv1a, entry_slot: ProgramEntrySlotDeclaration) {
    hash.bytes(entry_slot.owner.target_name().as_bytes());
    hash.bytes(entry_slot.slot_name.as_bytes());
    hash.byte(match entry_slot.schema {
        ProgramEntrySchema::HostedApplication => 1,
        ProgramEntrySchema::ProgramStorageApplication => 2,
    });
    hash.bytes(entry_slot.semantic_arrival_requirement.as_bytes());
    hash_optional_str(hash, entry_slot.physical_arrival_requirement);
    hash.byte(
        entry_slot
            .physical_contract_package
            .map_or(0, |package| match package {
                ProgramEntryPhysicalContractPackage::UefiX64 => 1,
                ProgramEntryPhysicalContractPackage::MacosArm64 => 2,
                ProgramEntryPhysicalContractPackage::MacosX64 => 6,
                ProgramEntryPhysicalContractPackage::LinuxX86_64 => 3,
                ProgramEntryPhysicalContractPackage::LinuxArm64 => 4,
                ProgramEntryPhysicalContractPackage::WindowsX64 => 5,
            }),
    );
    hash_optional_str(hash, entry_slot.boundary_schema);
    hash.byte(
        entry_slot
            .physical_calling_convention
            .map_or(0, |calling| match calling {
                ProgramEntryCallingConvention::MicrosoftX64 => 1,
                ProgramEntryCallingConvention::Aapcs64 => 2,
                ProgramEntryCallingConvention::SystemVAMD64 => 3,
            }),
    );
    hash.byte(
        entry_slot
            .semantic_calling_convention
            .map_or(0, |calling| match calling {
                ProgramEntryCallingConvention::MicrosoftX64 => 1,
                ProgramEntryCallingConvention::Aapcs64 => 2,
                ProgramEntryCallingConvention::SystemVAMD64 => 3,
            }),
    );
    hash.byte(match entry_slot.visible_parameters {
        ProgramEntryVisibleParameters::None => 1,
        ProgramEntryVisibleParameters::ImageAndInitialStorage => 2,
    });
    hash.byte(match entry_slot.receiver {
        ProgramEntryReceiverProvisioning::NoneOrProvisionedZii => 1,
    });
}

fn hash_optional_str(hash: &mut Fnv1a, value: Option<&str>) {
    match value {
        Some(value) => {
            hash.byte(1);
            hash.bytes(value.as_bytes());
        }
        None => hash.byte(0),
    }
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_boot_services_layout_retains_every_x64_field() {
        let layout = exact_uefi_x64_boot_services_native_layout();
        assert_eq!(layout.profile(), TargetProfile::UefiX64);
        assert_eq!(layout.field_count(), FIELD_COUNT);
        assert_eq!(layout.table_header_size(), 24);
        assert_eq!(layout.known_prefix_byte_size(), 376);
        assert_eq!(layout.alignment(), 8);
        assert_eq!(
            layout.non_authoritative_layout_report_fingerprint(),
            UEFI_X64_BOOT_SERVICES_NATIVE_LAYOUT_COMMITMENT
        );
        assert_eq!(
            layout
                .field_layout(UefiBootServicesNativeField::HandleProtocol)
                .unwrap()
                .byte_offset(),
            152
        );
        assert_eq!(
            layout
                .field_layout(UefiBootServicesNativeField::CreateEventEx)
                .unwrap()
                .byte_offset(),
            368
        );
        validate_fields(&layout.fields).unwrap();
    }

    #[test]
    fn replayed_layout_matches_the_exact_materialization() {
        let exact = exact_uefi_x64_boot_services_native_layout();
        let report = exact_uefi_x64_boot_services_layout_plan_report();
        let replayed = replayed_uefi_x64_boot_services_native_layout(&report)
            .expect("the authored plan report replays");
        assert!(exact.matches_exact_plan(&replayed));
    }

    #[test]
    fn plan_gate_still_rejects_non_uefi_profiles() {
        for profile in [
            TargetProfile::LinuxX64,
            TargetProfile::WindowsX64,
            TargetProfile::LocalUnchecked,
        ] {
            let error = plan_uefi_boot_services_native_layout(profile)
                .expect_err("non-UEFI profile must reject EFI_BOOT_SERVICES layout");
            assert_eq!(error.profile(), profile);
            assert_eq!(error.into_parts().0, profile);
        }
    }

    #[test]
    fn drifted_or_foreign_plan_reports_reject() {
        let mut foreign_name = exact_uefi_x64_boot_services_layout_plan_report();
        foreign_name.entries[0].field = "services".to_owned();
        assert!(replayed_uefi_x64_boot_services_native_layout(&foreign_name).is_none());

        let mut drifted = exact_uefi_x64_boot_services_layout_plan_report();
        drifted.entries[1].placement = LayoutPlacementReport::At { offset: 32 };
        assert!(replayed_uefi_x64_boot_services_native_layout(&drifted).is_none());

        let mut stored_width = exact_uefi_x64_boot_services_layout_plan_report();
        stored_width.entries[1].placement = LayoutPlacementReport::IntegerAt {
            offset: 24,
            stored_width: 32,
            interpretation: layout_plans::IntegerInterpretation::Unsigned,
        };
        assert!(replayed_uefi_x64_boot_services_native_layout(&stored_width).is_none());

        let mut wrong_size = exact_uefi_x64_boot_services_layout_plan_report();
        wrong_size.size = Some(384);
        assert!(replayed_uefi_x64_boot_services_native_layout(&wrong_size).is_none());

        let mut wrong_schema = exact_uefi_x64_boot_services_layout_plan_report();
        wrong_schema.schema_report_fingerprint ^= 1;
        assert!(replayed_uefi_x64_boot_services_native_layout(&wrong_schema).is_none());
    }
}
