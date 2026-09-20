//! Target-owned native layout of the UEFI x86-64 Loaded Image protocol.
//!
//! UEFI fixes this protocol as a revision followed by pointer-sized handles,
//! pointers, image geometry, memory-type values, and the unload callback. The
//! schema and its byte positions are authored once in the selected target
//! package (`source/library/std/targets/uefi_x86_64/tables.omg`) as the
//! `EfiLoadedImage` carrier record plus the evaluated `EfiLoadedImageLayout`
//! layout policy; this module retains the complete validated 96-byte x86-64
//! layout as descriptive evidence replayed from that evaluated plan.
//! `replayed_uefi_x64_loaded_image_native_layout` binds one retained
//! `LayoutPlanReport` to the recorded source-minted commitments, and
//! `exact_uefi_x64_loaded_image_native_layout` is the fixture materialization
//! for contexts below the build layer -- it routes through the same replay so
//! the residual literal recipe self-checks rather than standing alone. This
//! module grants no permission to dereference a protocol occurrence or treat
//! its image geometry as an `Extent`.
//!
//! [UEFI Loaded Image protocol]: https://uefi.org/specs/UEFI/2.11/09_Protocols_EFI_Loaded_Image.html

pub(crate) mod occurrence;

use crate::{
    Architecture, ObjectFormat, ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntrySlotDeclaration, TargetProfile,
};
use diagnostics::Diagnostic;
use layout_plans::{
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    normalized_layout_plan_report_fingerprint,
};

const FIELD_COUNT: usize = 15;
const SEMANTIC_FIELD_COUNT: usize = 13;
const PROTOCOL_SIZE: u32 = 96;
const PROTOCOL_ALIGNMENT: u32 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Compact FNV coordinate for the `EfiLoadedImage` schema the compiler
/// materializes from `std/targets/uefi_x86_64/tables.omg`, recorded from the
/// evaluated plan. This is evidence: a live evaluation whose schema drifts
/// from this coordinate fails replay instead of silently rebinding.
pub const UEFI_X64_LOADED_IMAGE_SCHEMA_REPORT_FINGERPRINT: u64 = 0x4ae5_84e2_dcce_000b;

/// Compact commitment to the validated `LayoutPlanReport` the authored
/// `EfiLoadedImageLayout::plan` policy produces for the bundled UEFI x86-64
/// target package. Recorded from source evaluation and kept honest by the
/// compiler replay test in `canary_suite/entry_and_abi/uefi_loaded_image_layout.rs`;
/// a report carrying any other schema, member, or placement rejects here.
pub const UEFI_X64_LOADED_IMAGE_LAYOUT_PLAN_COMMITMENT: u64 = 0x59b7_7325_968e_1604;

/// Compact commitment to the validated native field layout the evaluated plan
/// derives (`layout_report_fingerprint` over the complete row catalog). The
/// replayed rows must hash to this identity, so the retained field vocabulary
/// cannot drift silently from the authored plan.
pub const UEFI_X64_LOADED_IMAGE_NATIVE_LAYOUT_COMMITMENT: u64 = 0xc665_f34f_1cec_bf43;

/// Byte offsets the authored policy assigns to each `EfiLoadedImage` member in
/// declaration order. This residual literal recipe exists only for fixture
/// materialization below the build layer; every validated layout is produced
/// by replaying it through the commitments above.
const EXACT_SEMANTIC_OFFSETS: [u64; SEMANTIC_FIELD_COUNT] =
    [0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 84, 88];

/// The semantic vocabulary each authored `EfiLoadedImage` member carries:
/// name, field identity, carrier width, and kind. Names and kinds are the
/// protocol's semantics; byte positions come from the evaluated plan, and the
/// two ABI padding rows (`RevisionPadding`, `LoadOptionsSizePadding`) are the
/// plan's implicit gaps given their physical names.
const SEMANTIC_FIELDS: [(
    &str,
    UefiLoadedImageNativeField,
    u32,
    UefiLoadedImageNativeFieldKind,
); SEMANTIC_FIELD_COUNT] = [
    ("revision", Field::Revision, 4, Kind::UnsignedInteger),
    ("parent_handle", Field::ParentHandle, 8, Kind::Pointer),
    ("system_table", Field::SystemTable, 8, Kind::Pointer),
    ("device_handle", Field::DeviceHandle, 8, Kind::Pointer),
    ("file_path", Field::FilePath, 8, Kind::Pointer),
    ("reserved", Field::Reserved, 8, Kind::Pointer),
    (
        "load_options_size",
        Field::LoadOptionsSize,
        4,
        Kind::UnsignedInteger,
    ),
    ("load_options", Field::LoadOptions, 8, Kind::Pointer),
    ("image_base", Field::ImageBase, 8, Kind::Pointer),
    ("image_size", Field::ImageSize, 8, Kind::UnsignedInteger),
    (
        "image_code_type",
        Field::ImageCodeType,
        4,
        Kind::UnsignedInteger,
    ),
    (
        "image_data_type",
        Field::ImageDataType,
        4,
        Kind::UnsignedInteger,
    ),
    ("unload", Field::Unload, 8, Kind::FunctionPointer),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum UefiLoadedImageNativeField {
    Revision = 1,
    RevisionPadding,
    ParentHandle,
    SystemTable,
    DeviceHandle,
    FilePath,
    Reserved,
    LoadOptionsSize,
    LoadOptionsSizePadding,
    LoadOptions,
    ImageBase,
    ImageSize,
    ImageCodeType,
    ImageDataType,
    Unload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UefiLoadedImageNativeFieldKind {
    UnsignedInteger = 1,
    Pointer = 2,
    FunctionPointer = 3,
    Padding = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiLoadedImageNativeFieldLayout {
    field: UefiLoadedImageNativeField,
    ordinal: u8,
    byte_offset: u32,
    byte_size: u32,
    alignment: u32,
    kind: UefiLoadedImageNativeFieldKind,
}

impl UefiLoadedImageNativeFieldLayout {
    pub const fn field(self) -> UefiLoadedImageNativeField {
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
    pub const fn kind(self) -> UefiLoadedImageNativeFieldKind {
        self.kind
    }
}

#[derive(Debug)]
#[must_use = "validated UEFI Loaded Image layout retains target-owned entry identity"]
pub struct ValidatedUefiLoadedImageNativeLayout {
    profile: TargetProfile,
    entry_slot: ProgramEntrySlotDeclaration,
    fields: Vec<UefiLoadedImageNativeFieldLayout>,
    non_authoritative_layout_report_fingerprint: u64,
}

impl ValidatedUefiLoadedImageNativeLayout {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }
    pub const fn entry_slot(&self) -> ProgramEntrySlotDeclaration {
        self.entry_slot
    }
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
    pub const fn byte_size(&self) -> u32 {
        PROTOCOL_SIZE
    }
    pub const fn alignment(&self) -> u32 {
        PROTOCOL_ALIGNMENT
    }
    pub fn field_layout(
        &self,
        field: UefiLoadedImageNativeField,
    ) -> Option<UefiLoadedImageNativeFieldLayout> {
        self.fields.iter().copied().find(|row| row.field == field)
    }
    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }
    pub fn matches_exact_plan(&self, expected: &Self) -> bool {
        self.profile == expected.profile
            && self.entry_slot == expected.entry_slot
            && self.fields == expected.fields
    }
}

/// Replay one evaluated `EfiLoadedImageLayout::plan` report into the validated
/// native layout. The report must carry the recorded source-minted schema and
/// plan commitments; a foreign report, a drifted plan, or rows that no longer
/// cover the 96-byte aggregate reject. The two ABI padding rows are the
/// evaluated plan's implicit gaps under their physical names -- any other gap
/// is foreign geometry and rejects.
pub fn replayed_uefi_x64_loaded_image_native_layout(
    report: &LayoutPlanReport,
) -> Option<ValidatedUefiLoadedImageNativeLayout> {
    let profile = TargetProfile::UefiX64;
    let entry_slot = profile.program_entry_slot();
    if normalized_layout_plan_report_fingerprint(report)
        != UEFI_X64_LOADED_IMAGE_LAYOUT_PLAN_COMMITMENT
        || report.schema_report_fingerprint != UEFI_X64_LOADED_IMAGE_SCHEMA_REPORT_FINGERPRINT
        || validate_target_owner(profile, entry_slot).is_err()
    {
        return None;
    }
    let fields = fields_from_evaluated_plan(report)?;
    if validate_fields(&fields).is_err() {
        return None;
    }
    let fingerprint = layout_report_fingerprint(entry_slot, &fields);
    if fingerprint != UEFI_X64_LOADED_IMAGE_NATIVE_LAYOUT_COMMITMENT {
        return None;
    }
    Some(ValidatedUefiLoadedImageNativeLayout {
        profile,
        entry_slot,
        fields,
        non_authoritative_layout_report_fingerprint: fingerprint,
    })
}

/// Materialize the native layout for contract fixtures below the build layer,
/// where checked-tree evaluation is unavailable. The report shape mirrors what
/// `EfiLoadedImageLayout::plan` produces for `EfiLoadedImage` and is replayed
/// through the same commitments, so it cannot drift silently from the authored
/// policy.
pub fn exact_uefi_x64_loaded_image_native_layout() -> ValidatedUefiLoadedImageNativeLayout {
    replayed_uefi_x64_loaded_image_native_layout(&exact_uefi_x64_loaded_image_layout_plan_report())
        .expect(
            "the authored UEFI x64 Loaded Image layout must replay its source-minted commitments",
        )
}

/// The report the authored policy produces, in `LayoutPlanReport` form. This
/// is the one residual literal recipe; consumers must route it through
/// `replayed_uefi_x64_loaded_image_native_layout`, never read it directly.
pub fn exact_uefi_x64_loaded_image_layout_plan_report() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: UEFI_X64_LOADED_IMAGE_SCHEMA_REPORT_FINGERPRINT,
        entries: SEMANTIC_FIELDS
            .iter()
            .zip(EXACT_SEMANTIC_OFFSETS)
            .map(|(&(name, _, _, _), offset)| LayoutFieldEntryReport {
                field: name.to_owned(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset },
            })
            .collect(),
        offsets: Some(EXACT_SEMANTIC_OFFSETS.to_vec()),
        size: Some(u64::from(PROTOCOL_SIZE)),
        align: u64::from(PROTOCOL_ALIGNMENT),
    }
}

fn fields_from_evaluated_plan(
    report: &LayoutPlanReport,
) -> Option<Vec<UefiLoadedImageNativeFieldLayout>> {
    if report.entries.len() != SEMANTIC_FIELD_COUNT {
        return None;
    }
    let mut fields = Vec::with_capacity(FIELD_COUNT);
    let mut prior_end = 0_u32;
    for (&(name, field, width, kind), entry) in SEMANTIC_FIELDS.iter().zip(report.entries.iter()) {
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
                (4, 4) => Field::RevisionPadding,
                (52, 4) => Field::LoadOptionsSizePadding,
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
    (prior_end == PROTOCOL_SIZE).then_some(fields)
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
        "EFI_LOADED_IMAGE_PROTOCOL native layout is owned only by the UEFI x86-64 target",
    )?;
    require(
        entry_slot == TargetProfile::UefiX64.program_entry_slot()
            && entry_slot.physical_arrival_requirement == Some("UefiPhysicalEntry::enter")
            && entry_slot.physical_contract_package
                == Some(ProgramEntryPhysicalContractPackage::UefiX64)
            && entry_slot.physical_calling_convention
                == Some(ProgramEntryCallingConvention::MicrosoftX64),
        "EFI_LOADED_IMAGE_PROTOCOL native layout drifted from its target-owned physical entry",
    )
}

fn validate_fields(fields: &[UefiLoadedImageNativeFieldLayout]) -> Result<(), Diagnostic> {
    require(
        fields.len() == FIELD_COUNT,
        "EFI_LOADED_IMAGE_PROTOCOL field catalog is missing, duplicated, reordered, or drifted",
    )?;
    let mut prior_end = 0_u32;
    for (ordinal, row) in fields.iter().enumerate() {
        require(
            usize::from(row.ordinal) == ordinal
                && row.byte_size != 0
                && row.alignment.is_power_of_two()
                && row.byte_offset % row.alignment == 0
                && row.byte_offset >= prior_end,
            "EFI_LOADED_IMAGE_PROTOCOL field order, alignment, or geometry is invalid",
        )?;
        prior_end = row.byte_offset.checked_add(row.byte_size).ok_or_else(|| {
            Diagnostic::error("EFI_LOADED_IMAGE_PROTOCOL field end overflows u32")
        })?;
    }
    require(
        prior_end == PROTOCOL_SIZE,
        "EFI_LOADED_IMAGE_PROTOCOL field catalog does not exactly cover its aggregate",
    )?;
    require(
        fields
            .iter()
            .filter(|row| row.kind == UefiLoadedImageNativeFieldKind::Padding)
            .map(|row| row.field)
            .eq([
                UefiLoadedImageNativeField::RevisionPadding,
                UefiLoadedImageNativeField::LoadOptionsSizePadding,
            ]),
        "EFI_LOADED_IMAGE_PROTOCOL native layout has missing or unexpected ABI padding",
    )
}

const fn row(
    field: UefiLoadedImageNativeField,
    ordinal: u8,
    byte_offset: u32,
    byte_size: u32,
    alignment: u32,
    kind: UefiLoadedImageNativeFieldKind,
) -> UefiLoadedImageNativeFieldLayout {
    UefiLoadedImageNativeFieldLayout {
        field,
        ordinal,
        byte_offset,
        byte_size,
        alignment,
        kind,
    }
}

use UefiLoadedImageNativeField as Field;
use UefiLoadedImageNativeFieldKind as Kind;

fn layout_report_fingerprint(
    entry_slot: ProgramEntrySlotDeclaration,
    fields: &[UefiLoadedImageNativeFieldLayout],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.uefi-x64-loaded-image-native-layout.v1");
    hash.bytes(entry_slot.owner.target_name().as_bytes());
    hash.bytes(entry_slot.slot_name.as_bytes());
    hash.bytes(
        entry_slot
            .physical_arrival_requirement
            .unwrap_or_default()
            .as_bytes(),
    );
    for row in fields {
        hash.byte(row.field as u8);
        hash.byte(row.ordinal);
        hash.bytes(&row.byte_offset.to_le_bytes());
        hash.bytes(&row.byte_size.to_le_bytes());
        hash.bytes(&row.alignment.to_le_bytes());
        hash.byte(row.kind as u8);
    }
    hash.bytes(&PROTOCOL_SIZE.to_le_bytes());
    hash.bytes(&PROTOCOL_ALIGNMENT.to_le_bytes());
    hash.finish()
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
    fn exact_loaded_image_layout_retains_every_x64_field_and_padding_row() {
        let layout = exact_uefi_x64_loaded_image_native_layout();
        assert_eq!(layout.field_count(), 15);
        assert_eq!(layout.byte_size(), 96);
        assert_eq!(layout.alignment(), 8);
        let image_base = layout.field_layout(Field::ImageBase).unwrap();
        let image_size = layout.field_layout(Field::ImageSize).unwrap();
        assert_eq!((image_base.ordinal(), image_base.byte_offset()), (10, 64));
        assert_eq!((image_size.ordinal(), image_size.byte_offset()), (11, 72));
        assert_eq!(
            layout.non_authoritative_layout_report_fingerprint(),
            UEFI_X64_LOADED_IMAGE_NATIVE_LAYOUT_COMMITMENT
        );
    }

    #[test]
    fn replayed_layout_matches_the_exact_materialization() {
        let exact = exact_uefi_x64_loaded_image_native_layout();
        let report = exact_uefi_x64_loaded_image_layout_plan_report();
        let replayed = replayed_uefi_x64_loaded_image_native_layout(&report)
            .expect("the authored plan report replays");
        assert!(exact.matches_exact_plan(&replayed));
    }

    #[test]
    fn drifted_or_foreign_plan_reports_reject() {
        let mut foreign_name = exact_uefi_x64_loaded_image_layout_plan_report();
        foreign_name.entries[0].field = "image_base".to_owned();
        assert!(replayed_uefi_x64_loaded_image_native_layout(&foreign_name).is_none());

        let mut drifted = exact_uefi_x64_loaded_image_layout_plan_report();
        drifted.entries[10].placement = LayoutPlacementReport::At { offset: 56 };
        assert!(replayed_uefi_x64_loaded_image_native_layout(&drifted).is_none());

        let mut stored_width = exact_uefi_x64_loaded_image_layout_plan_report();
        stored_width.entries[6].placement = LayoutPlacementReport::IntegerAt {
            offset: 48,
            stored_width: 32,
            interpretation: layout_plans::IntegerInterpretation::Unsigned,
        };
        assert!(replayed_uefi_x64_loaded_image_native_layout(&stored_width).is_none());

        let mut wrong_size = exact_uefi_x64_loaded_image_layout_plan_report();
        wrong_size.size = Some(104);
        assert!(replayed_uefi_x64_loaded_image_native_layout(&wrong_size).is_none());

        let mut wrong_schema = exact_uefi_x64_loaded_image_layout_plan_report();
        wrong_schema.schema_report_fingerprint ^= 1;
        assert!(replayed_uefi_x64_loaded_image_native_layout(&wrong_schema).is_none());
    }
}
