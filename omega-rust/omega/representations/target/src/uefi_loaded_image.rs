//! Target-owned native layout of the UEFI x86-64 Loaded Image protocol.
//!
//! UEFI fixes this protocol as a revision followed by pointer-sized handles,
//! pointers, image geometry, memory-type values, and the unload callback. This
//! module retains the complete 96-byte x86-64 layout as descriptive evidence.
//! It grants no permission to dereference a protocol occurrence or treat its
//! image geometry as an `Extent`.
//!
//! [UEFI Loaded Image protocol]: https://uefi.org/specs/UEFI/2.11/09_Protocols_EFI_Loaded_Image.html

use crate::{
    Architecture, ObjectFormat, ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntrySlotDeclaration, TargetProfile,
};
use diagnostics::Diagnostic;

const FIELD_COUNT: usize = 15;
const PROTOCOL_SIZE: u32 = 96;
const PROTOCOL_ALIGNMENT: u32 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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

#[derive(Debug)]
#[must_use = "UEFI Loaded Image layout rejection retains the requested profile"]
pub struct UefiLoadedImageNativeLayoutError {
    profile: TargetProfile,
    diagnostic: Diagnostic,
}

impl UefiLoadedImageNativeLayoutError {
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

impl std::fmt::Display for UefiLoadedImageNativeLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiLoadedImageNativeLayoutError {}

pub fn plan_uefi_loaded_image_native_layout(
    profile: TargetProfile,
) -> Result<ValidatedUefiLoadedImageNativeLayout, Box<UefiLoadedImageNativeLayoutError>> {
    let entry_slot = profile.program_entry_slot();
    if let Err(diagnostic) = validate_target_owner(profile, entry_slot) {
        return Err(Box::new(UefiLoadedImageNativeLayoutError {
            profile,
            diagnostic,
        }));
    }
    let fields = canonical_fields().to_vec();
    if let Err(diagnostic) = validate_fields(&fields) {
        return Err(Box::new(UefiLoadedImageNativeLayoutError {
            profile,
            diagnostic,
        }));
    }
    Ok(ValidatedUefiLoadedImageNativeLayout {
        profile,
        entry_slot,
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint(entry_slot, &fields),
        fields,
    })
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
        fields == canonical_fields(),
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

const CANONICAL_FIELDS: [UefiLoadedImageNativeFieldLayout; FIELD_COUNT] = [
    row(Field::Revision, 0, 0, 4, 4, Kind::UnsignedInteger),
    row(Field::RevisionPadding, 1, 4, 4, 4, Kind::Padding),
    row(Field::ParentHandle, 2, 8, 8, 8, Kind::Pointer),
    row(Field::SystemTable, 3, 16, 8, 8, Kind::Pointer),
    row(Field::DeviceHandle, 4, 24, 8, 8, Kind::Pointer),
    row(Field::FilePath, 5, 32, 8, 8, Kind::Pointer),
    row(Field::Reserved, 6, 40, 8, 8, Kind::Pointer),
    row(Field::LoadOptionsSize, 7, 48, 4, 4, Kind::UnsignedInteger),
    row(Field::LoadOptionsSizePadding, 8, 52, 4, 4, Kind::Padding),
    row(Field::LoadOptions, 9, 56, 8, 8, Kind::Pointer),
    row(Field::ImageBase, 10, 64, 8, 8, Kind::Pointer),
    row(Field::ImageSize, 11, 72, 8, 8, Kind::UnsignedInteger),
    row(Field::ImageCodeType, 12, 80, 4, 4, Kind::UnsignedInteger),
    row(Field::ImageDataType, 13, 84, 4, 4, Kind::UnsignedInteger),
    row(Field::Unload, 14, 88, 8, 8, Kind::FunctionPointer),
];

const fn canonical_fields() -> &'static [UefiLoadedImageNativeFieldLayout; FIELD_COUNT] {
    &CANONICAL_FIELDS
}

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
        let layout = plan_uefi_loaded_image_native_layout(TargetProfile::UefiX64).unwrap();
        assert_eq!(layout.field_count(), 15);
        assert_eq!(layout.byte_size(), 96);
        assert_eq!(layout.alignment(), 8);
        let image_base = layout.field_layout(Field::ImageBase).unwrap();
        let image_size = layout.field_layout(Field::ImageSize).unwrap();
        assert_eq!((image_base.ordinal(), image_base.byte_offset()), (10, 64));
        assert_eq!((image_size.ordinal(), image_size.byte_offset()), (11, 72));
        assert_ne!(layout.non_authoritative_layout_report_fingerprint(), 0);
    }

    #[test]
    fn non_uefi_profiles_and_layout_drift_reject() {
        assert!(plan_uefi_loaded_image_native_layout(TargetProfile::WindowsX64).is_err());
        let exact = plan_uefi_loaded_image_native_layout(TargetProfile::UefiX64).unwrap();
        assert!(exact.matches_exact_plan(
            &plan_uefi_loaded_image_native_layout(TargetProfile::UefiX64).unwrap()
        ));

        let mut reordered = canonical_fields().to_vec();
        reordered.swap(10, 11);
        assert!(validate_fields(&reordered).is_err());

        let mut drifted = canonical_fields().to_vec();
        drifted[10].byte_offset = 56;
        assert!(validate_fields(&drifted).is_err());
    }
}
