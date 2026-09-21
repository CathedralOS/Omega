//! Target-owned native layout of the UEFI x86-64 system table.
//!
//! The [UEFI specification's table-header chapter][header] and [system-table
//! chapter][system-table] fix the C field order of `EFI_TABLE_HEADER` and the
//! currently defined `EFI_SYSTEM_TABLE` prefix. The authored schema and its
//! evaluated layout policy in `std/targets/uefi_x86_64/tables.omg` own that
//! geometry; this module retains the resulting x86-64 offsets as descriptive
//! evidence replayed from that evaluated plan. It does not inspect a table
//! occurrence, validate a firmware header or CRC, install a provider, or grant
//! authority to dereference any retained pointer field.
//!
//! `replayed_uefi_x64_system_table_native_layout` binds one retained
//! `LayoutPlanReport` to the recorded source-minted commitments, and
//! `exact_uefi_x64_system_table_native_layout` is the fixture materialization
//! below the build layer; every validated layout is produced by replaying it
//! through the commitments below.
//!
//! [header]: https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html#efi-table-header
//! [system-table]: https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html#efi-system-table

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

const FIELD_COUNT: usize = 18;
const SEMANTIC_FIELD_COUNT: usize = 13;
const TABLE_HEADER_SIZE: u32 = 24;
const TABLE_SIZE: u32 = 120;
const TABLE_ALIGNMENT: u32 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Recorded commitment to the authored `EfiSystemTableView` schema report
/// fingerprint. Every replayed plan must come from that source-minted schema.
pub const UEFI_X64_SYSTEM_TABLE_SCHEMA_REPORT_FINGERPRINT: u64 = 0xd007_21f7_9763_5cd7;

/// Compact commitment to the validated `LayoutPlanReport` the authored
/// `EfiSystemTableViewLayout::plan` policy produces for `EfiSystemTableView`.
pub const UEFI_X64_SYSTEM_TABLE_LAYOUT_PLAN_COMMITMENT: u64 = 0xa3aa_ed00_b7a9_c504;

/// Compact commitment to the validated native field layout the evaluated plan
/// replayed rows must hash to this identity, so the retained field vocabulary
/// cannot drift silently from the source-authored geometry.
pub const UEFI_X64_SYSTEM_TABLE_NATIVE_LAYOUT_COMMITMENT: u64 = 0x902a_8803_3004_f8bf;

/// The target byte offsets the authored `EfiSystemTableViewLayout::plan` produces
/// for `EfiSystemTableView`'s thirteen semantic members, in declaration order.
const EXACT_SEMANTIC_OFFSETS: [u64; SEMANTIC_FIELD_COUNT] =
    [0, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112];

/// The semantic vocabulary each authored `EfiSystemTableView` member carries past
/// the shared `header` preamble: name, field identity, carrier width, and
/// kind. Names and kinds are the protocol's semantics; byte positions come
/// from the evaluated plan, and the ABI padding row (`FirmwareRevisionPadding`)
/// is the plan's implicit gap given its physical name.
const SEMANTIC_FIELDS: [(
    &str,
    UefiSystemTableNativeField,
    u32,
    UefiSystemTableNativeFieldKind,
); SEMANTIC_FIELD_COUNT - 1] = [
    ("firmware_vendor", Field::FirmwareVendor, 8, Kind::Pointer),
    (
        "firmware_revision",
        Field::FirmwareRevision,
        4,
        Kind::UnsignedInteger,
    ),
    (
        "console_in_handle",
        Field::ConsoleInHandle,
        8,
        Kind::Pointer,
    ),
    ("console_in", Field::ConsoleIn, 8, Kind::Pointer),
    (
        "console_out_handle",
        Field::ConsoleOutHandle,
        8,
        Kind::Pointer,
    ),
    ("console_out", Field::ConsoleOut, 8, Kind::Pointer),
    (
        "standard_error_handle",
        Field::StandardErrorHandle,
        8,
        Kind::Pointer,
    ),
    ("standard_error", Field::StandardError, 8, Kind::Pointer),
    ("runtime_services", Field::RuntimeServices, 8, Kind::Pointer),
    ("boot_services", Field::BootServices, 8, Kind::Pointer),
    (
        "number_of_table_entries",
        Field::NumberOfTableEntries,
        8,
        Kind::UnsignedInteger,
    ),
    (
        "configuration_table",
        Field::ConfigurationTable,
        8,
        Kind::Pointer,
    ),
];

/// Closed field identity for the UEFI x86-64 `EFI_SYSTEM_TABLE` layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum UefiSystemTableNativeField {
    HeaderSignature = 1,
    HeaderRevision = 2,
    HeaderSize = 3,
    HeaderCrc32 = 4,
    HeaderReserved = 5,
    FirmwareVendor = 6,
    FirmwareRevision = 7,
    FirmwareRevisionPadding = 8,
    ConsoleInHandle = 9,
    ConsoleIn = 10,
    ConsoleOutHandle = 11,
    ConsoleOut = 12,
    StandardErrorHandle = 13,
    StandardError = 14,
    RuntimeServices = 15,
    BootServices = 16,
    NumberOfTableEntries = 17,
    ConfigurationTable = 18,
}

/// Native representation class of one retained field row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UefiSystemTableNativeFieldKind {
    UnsignedInteger = 1,
    Pointer = 2,
    ReservedZero = 3,
    Padding = 4,
}

/// Exact target-relative geometry of one system-table field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiSystemTableNativeFieldLayout {
    field: UefiSystemTableNativeField,
    ordinal: u8,
    byte_offset: u32,
    byte_size: u32,
    alignment: u32,
    kind: UefiSystemTableNativeFieldKind,
}

impl UefiSystemTableNativeFieldLayout {
    pub const fn field(self) -> UefiSystemTableNativeField {
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

    pub const fn kind(self) -> UefiSystemTableNativeFieldKind {
        self.kind
    }
}

/// Independently replayed target-owned layout evidence for one UEFI x86-64
/// system table.
///
/// The exact target entry slot remains bound to the closed field catalog. This
/// non-clone carrier grants no table occurrence, pointer provenance, firmware
/// lifecycle, provider, bootstrap-shell, semantic-root, or native-execution
/// authority.
#[derive(Debug)]
#[must_use = "validated UEFI system-table layout retains target-owned entry identity"]
pub struct ValidatedUefiSystemTableNativeLayout {
    profile: TargetProfile,
    entry_slot: ProgramEntrySlotDeclaration,
    contents: UefiSystemTableNativeLayoutContents,
    non_authoritative_layout_report_fingerprint: u64,
}

impl ValidatedUefiSystemTableNativeLayout {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }

    pub const fn entry_slot(&self) -> ProgramEntrySlotDeclaration {
        self.entry_slot
    }

    pub fn field_count(&self) -> usize {
        self.contents.fields.len()
    }

    pub const fn table_header_size(&self) -> u32 {
        self.contents.table_header_size
    }

    /// Byte extent of the closed prefix defined by this target plan. A runtime
    /// table's validated header may advertise a larger forward-compatible
    /// extent; this descriptive carrier does not inspect that occurrence.
    pub const fn known_prefix_byte_size(&self) -> u32 {
        self.contents.byte_size
    }

    pub const fn alignment(&self) -> u32 {
        self.contents.alignment
    }

    pub fn field_layout(
        &self,
        field: UefiSystemTableNativeField,
    ) -> Option<UefiSystemTableNativeFieldLayout> {
        self.contents
            .fields
            .iter()
            .copied()
            .find(|row| row.field == field)
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
            && self.contents == expected.contents
    }

    #[allow(dead_code)]
    pub(crate) const fn contents(&self) -> &UefiSystemTableNativeLayoutContents {
        &self.contents
    }
}

/// Rejected native-layout planning with the requested target profile retained.
#[derive(Debug)]
#[must_use = "UEFI system-table layout rejection retains the requested profile"]
pub struct UefiSystemTableNativeLayoutError {
    profile: TargetProfile,
    diagnostic: Diagnostic,
}

impl UefiSystemTableNativeLayoutError {
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

impl std::fmt::Display for UefiSystemTableNativeLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiSystemTableNativeLayoutError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UefiSystemTableNativeLayoutContents {
    pub(crate) fields: Vec<UefiSystemTableNativeFieldLayout>,
    pub(crate) table_header_size: u32,
    pub(crate) byte_size: u32,
    pub(crate) alignment: u32,
}

/// Replay one evaluated `EfiSystemTableViewLayout::plan` report into the validated
/// native layout. The report must carry the recorded source-minted schema and
/// plan commitments; a foreign report, a drifted plan, or rows that no longer
/// cover the 120-byte aggregate reject. The `FirmwareRevisionPadding` row is
/// the evaluated plan's implicit gap under its physical name -- any other gap
/// is foreign geometry and rejects.
pub fn replayed_uefi_x64_system_table_native_layout(
    report: &LayoutPlanReport,
) -> Option<ValidatedUefiSystemTableNativeLayout> {
    let profile = TargetProfile::UefiX64;
    let entry_slot = profile.program_entry_slot();
    if normalized_layout_plan_report_fingerprint(report)
        != UEFI_X64_SYSTEM_TABLE_LAYOUT_PLAN_COMMITMENT
        || report.schema_report_fingerprint != UEFI_X64_SYSTEM_TABLE_SCHEMA_REPORT_FINGERPRINT
        || validate_target_owner(profile, entry_slot).is_err()
    {
        return None;
    }
    let fields = fields_from_evaluated_plan(report)?;
    let contents = UefiSystemTableNativeLayoutContents {
        fields,
        table_header_size: TABLE_HEADER_SIZE,
        byte_size: TABLE_SIZE,
        alignment: TABLE_ALIGNMENT,
    };
    if validate_contents(&contents).is_err() {
        return None;
    }
    let fingerprint = non_authoritative_layout_report_fingerprint(entry_slot, &contents);
    if fingerprint != UEFI_X64_SYSTEM_TABLE_NATIVE_LAYOUT_COMMITMENT {
        return None;
    }
    Some(ValidatedUefiSystemTableNativeLayout {
        profile,
        entry_slot,
        contents,
        non_authoritative_layout_report_fingerprint: fingerprint,
    })
}

/// Materialize the native layout for contract fixtures below the build layer,
/// where checked-tree evaluation is unavailable. The report shape mirrors what
/// `EfiSystemTableViewLayout::plan` produces for `EfiSystemTableView` and is replayed
/// through the same commitments, so it cannot drift silently from the authored
/// policy.
pub fn exact_uefi_x64_system_table_native_layout() -> ValidatedUefiSystemTableNativeLayout {
    replayed_uefi_x64_system_table_native_layout(&exact_uefi_x64_system_table_layout_plan_report())
        .expect(
            "the authored UEFI x64 system-table layout must replay its source-minted commitments",
        )
}

/// The report the authored policy produces, in `LayoutPlanReport` form. This
/// is the one residual literal recipe; consumers must route it through
/// `replayed_uefi_x64_system_table_native_layout`, never read it directly.
pub fn exact_uefi_x64_system_table_layout_plan_report() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: UEFI_X64_SYSTEM_TABLE_SCHEMA_REPORT_FINGERPRINT,
        entries: std::iter::once(&"header")
            .copied()
            .chain(SEMANTIC_FIELDS.iter().map(|&(name, _, _, _)| name))
            .zip(EXACT_SEMANTIC_OFFSETS)
            .map(|(name, offset)| LayoutFieldEntryReport {
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
/// `EFI_SYSTEM_TABLE` native layout is owned only by the UEFI x86-64 target;
/// any other profile rejects rather than inheriting firmware structure from
/// its shared x86-64 architecture.
pub fn plan_uefi_system_table_native_layout(
    profile: TargetProfile,
) -> Result<ValidatedUefiSystemTableNativeLayout, Box<UefiSystemTableNativeLayoutError>> {
    let entry_slot = profile.program_entry_slot();
    if let Err(diagnostic) = validate_target_owner(profile, entry_slot) {
        return Err(Box::new(UefiSystemTableNativeLayoutError {
            profile,
            diagnostic,
        }));
    }
    Ok(exact_uefi_x64_system_table_native_layout())
}

fn fields_from_evaluated_plan(
    report: &LayoutPlanReport,
) -> Option<Vec<UefiSystemTableNativeFieldLayout>> {
    if report.entries.len() != SEMANTIC_FIELD_COUNT {
        return None;
    }
    let (header, rest) = report.entries.split_first()?;
    if header.field != "header" {
        return None;
    }
    let header_offset = match header.placement {
        LayoutPlacementReport::At { offset } => u32::try_from(offset).ok()?,
        _ => return None,
    };
    if header_offset != 0 {
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
    let mut prior_end = header_offset.checked_add(TABLE_HEADER_SIZE)?;
    for (&(name, field, width, kind), entry) in SEMANTIC_FIELDS.iter().zip(rest.iter()) {
        if entry.field != name {
            return None;
        }
        let offset = match entry.placement {
            LayoutPlacementReport::At { offset } => u32::try_from(offset).ok()?,
            _ => return None,
        };
        if offset < prior_end || offset % width != 0 {
            return None;
        }
        if offset > prior_end {
            let gap = offset - prior_end;
            let padding = match (prior_end, gap) {
                (36, 4) => Field::FirmwareRevisionPadding,
                _ => return None,
            };
            fields.push(row(
                padding,
                fields.len() as u8,
                prior_end,
                gap,
                gap,
                Kind::Padding,
            ));
        }
        fields.push(row(field, fields.len() as u8, offset, width, width, kind));
        prior_end = offset.checked_add(width)?;
    }
    (prior_end == TABLE_SIZE).then_some(fields)
}

/// The `EfiTableHeader` preamble's retained native rows, replayed from the
/// evaluated `header` member's placement: `(member offset, width, kind)` in
/// declaration order.
const HEADER_ROWS: [(u32, u32, UefiSystemTableNativeFieldKind); 5] = [
    (0, 8, Kind::UnsignedInteger),
    (8, 4, Kind::UnsignedInteger),
    (12, 4, Kind::UnsignedInteger),
    (16, 4, Kind::UnsignedInteger),
    (20, 4, Kind::ReservedZero),
];

const fn header_row_field(offset: u32) -> UefiSystemTableNativeField {
    match offset {
        0 => UefiSystemTableNativeField::HeaderSignature,
        8 => UefiSystemTableNativeField::HeaderRevision,
        12 => UefiSystemTableNativeField::HeaderSize,
        16 => UefiSystemTableNativeField::HeaderCrc32,
        _ => UefiSystemTableNativeField::HeaderReserved,
    }
}

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
        "EFI_SYSTEM_TABLE native layout is owned only by the UEFI x86-64 target",
    )?;
    let expected = TargetProfile::UefiX64.program_entry_slot();
    require(
        entry_slot == expected
            && entry_slot.physical_arrival_requirement == Some("UefiPhysicalEntry::enter")
            && entry_slot.physical_contract_package
                == Some(ProgramEntryPhysicalContractPackage::UefiX64)
            && entry_slot.physical_calling_convention
                == Some(ProgramEntryCallingConvention::MicrosoftX64),
        "EFI_SYSTEM_TABLE native layout drifted from its exact target-owned physical entry",
    )
}

fn validate_contents(contents: &UefiSystemTableNativeLayoutContents) -> Result<(), Diagnostic> {
    require(
        contents.fields.len() == FIELD_COUNT,
        "EFI_SYSTEM_TABLE native field catalog is missing, duplicated, reordered, or drifted",
    )?;
    require(
        contents.table_header_size == TABLE_HEADER_SIZE
            && contents.byte_size == TABLE_SIZE
            && contents.alignment == TABLE_ALIGNMENT,
        "EFI_SYSTEM_TABLE native aggregate geometry drifted",
    )?;
    let mut prior_end = 0;
    for (ordinal, row) in contents.fields.iter().enumerate() {
        require(
            usize::from(row.ordinal) == ordinal
                && row.byte_size != 0
                && row.alignment.is_power_of_two()
                && row.byte_offset % row.alignment == 0
                && row.byte_offset >= prior_end,
            "EFI_SYSTEM_TABLE native field order, alignment, or geometry is invalid",
        )?;
        prior_end = row
            .byte_offset
            .checked_add(row.byte_size)
            .ok_or_else(|| Diagnostic::error("EFI_SYSTEM_TABLE field end overflows u32"))?;
    }
    require(
        prior_end == contents.byte_size,
        "EFI_SYSTEM_TABLE field catalog does not exactly cover the aggregate extent",
    )?;
    require(
        contents
            .fields
            .iter()
            .filter(|row| row.kind == UefiSystemTableNativeFieldKind::Padding)
            .count()
            == 1
            && contents.fields[7].field == UefiSystemTableNativeField::FirmwareRevisionPadding,
        "EFI_SYSTEM_TABLE native layout has missing or unexpected ABI padding",
    )
}

const fn row(
    field: UefiSystemTableNativeField,
    ordinal: u8,
    byte_offset: u32,
    byte_size: u32,
    alignment: u32,
    kind: UefiSystemTableNativeFieldKind,
) -> UefiSystemTableNativeFieldLayout {
    UefiSystemTableNativeFieldLayout {
        field,
        ordinal,
        byte_offset,
        byte_size,
        alignment,
        kind,
    }
}

use UefiSystemTableNativeField as Field;
use UefiSystemTableNativeFieldKind as Kind;

fn non_authoritative_layout_report_fingerprint(
    entry_slot: ProgramEntrySlotDeclaration,
    contents: &UefiSystemTableNativeLayoutContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.uefi-x64-system-table-native-layout.v1");
    hash_entry_slot(&mut hash, entry_slot);
    for row in &contents.fields {
        hash.byte(row.field as u8);
        hash.byte(row.ordinal);
        hash.bytes(&row.byte_offset.to_le_bytes());
        hash.bytes(&row.byte_size.to_le_bytes());
        hash.bytes(&row.alignment.to_le_bytes());
        hash.byte(row.kind as u8);
    }
    hash.bytes(&contents.table_header_size.to_le_bytes());
    hash.bytes(&contents.byte_size.to_le_bytes());
    hash.bytes(&contents.alignment.to_le_bytes());
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
    fn exact_system_table_layout_retains_every_x64_field_and_padding_row() {
        let layout = exact_uefi_x64_system_table_native_layout();
        assert_eq!(layout.profile(), TargetProfile::UefiX64);
        assert_eq!(layout.field_count(), FIELD_COUNT);
        assert_eq!(layout.table_header_size(), 24);
        assert_eq!(layout.known_prefix_byte_size(), 120);
        assert_eq!(layout.alignment(), 8);
        assert_eq!(
            layout.non_authoritative_layout_report_fingerprint(),
            UEFI_X64_SYSTEM_TABLE_NATIVE_LAYOUT_COMMITMENT
        );
        assert_eq!(
            layout
                .field_layout(UefiSystemTableNativeField::ConsoleOut)
                .unwrap()
                .byte_offset(),
            64
        );
        assert_eq!(
            layout
                .field_layout(UefiSystemTableNativeField::BootServices)
                .unwrap()
                .byte_offset(),
            96
        );
        assert_eq!(
            layout
                .field_layout(UefiSystemTableNativeField::ConfigurationTable)
                .unwrap()
                .byte_offset(),
            112
        );
        validate_contents(&layout.contents).unwrap();
    }

    #[test]
    fn replayed_layout_matches_the_exact_materialization() {
        let exact = exact_uefi_x64_system_table_native_layout();
        let report = exact_uefi_x64_system_table_layout_plan_report();
        let replayed = replayed_uefi_x64_system_table_native_layout(&report)
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
            let error = plan_uefi_system_table_native_layout(profile)
                .expect_err("non-UEFI profile must reject EFI_SYSTEM_TABLE layout");
            assert_eq!(error.profile(), profile);
            assert_eq!(error.into_parts().0, profile);
        }
    }

    #[test]
    fn drifted_or_foreign_plan_reports_reject() {
        let mut foreign_name = exact_uefi_x64_system_table_layout_plan_report();
        foreign_name.entries[0].field = "boot_services".to_owned();
        assert!(replayed_uefi_x64_system_table_native_layout(&foreign_name).is_none());

        let mut foreign_header_name = exact_uefi_x64_system_table_layout_plan_report();
        foreign_header_name.entries[3].field = "header".to_owned();
        assert!(replayed_uefi_x64_system_table_native_layout(&foreign_header_name).is_none());

        let mut drifted = exact_uefi_x64_system_table_layout_plan_report();
        drifted.entries[5].placement = LayoutPlacementReport::At { offset: 72 };
        assert!(replayed_uefi_x64_system_table_native_layout(&drifted).is_none());

        let mut stored_width = exact_uefi_x64_system_table_layout_plan_report();
        stored_width.entries[1].placement = LayoutPlacementReport::IntegerAt {
            offset: 24,
            stored_width: 32,
            interpretation: layout_plans::IntegerInterpretation::Unsigned,
        };
        assert!(replayed_uefi_x64_system_table_native_layout(&stored_width).is_none());

        let mut wrong_size = exact_uefi_x64_system_table_layout_plan_report();
        wrong_size.size = Some(128);
        assert!(replayed_uefi_x64_system_table_native_layout(&wrong_size).is_none());

        let mut wrong_schema = exact_uefi_x64_system_table_layout_plan_report();
        wrong_schema.schema_report_fingerprint ^= 1;
        assert!(replayed_uefi_x64_system_table_native_layout(&wrong_schema).is_none());

        let mut foreign_gap = exact_uefi_x64_system_table_layout_plan_report();
        foreign_gap.entries[3].placement = LayoutPlacementReport::At { offset: 44 };
        assert!(replayed_uefi_x64_system_table_native_layout(&foreign_gap).is_none());
    }
}
