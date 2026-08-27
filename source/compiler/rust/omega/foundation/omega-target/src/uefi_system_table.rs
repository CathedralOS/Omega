//! Target-owned native layout of the UEFI x86-64 system table.
//!
//! The [UEFI specification's table-header chapter][header] and [system-table
//! chapter][system-table] fix the C field order of `EFI_TABLE_HEADER` and the
//! currently defined `EFI_SYSTEM_TABLE` prefix. This module retains the
//! resulting x86-64 offsets as descriptive target evidence. It does not
//! inspect a table occurrence, validate a firmware header or CRC, install a
//! provider, or grant authority to dereference any retained pointer field.
//!
//! [header]: https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html#efi-table-header
//! [system-table]: https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html#efi-system-table

use crate::{
    Architecture, ObjectFormat, ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntryReceiverProvisioning, ProgramEntrySchema, ProgramEntrySlotDeclaration,
    ProgramEntryVisibleParameters, TargetProfile,
};
use psi_diagnostics::Diagnostic;

const FIELD_COUNT: usize = 18;
const TABLE_HEADER_SIZE: u32 = 24;
const TABLE_SIZE: u32 = 120;
const TABLE_ALIGNMENT: u32 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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
    layout_identity: u64,
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

    /// Compatibility fingerprint over the exact target/package/calling-plan
    /// selection and every field row. This is layout identity, not firmware
    /// occurrence or provider authority.
    pub const fn layout_identity(&self) -> u64 {
        self.layout_identity
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

struct Candidate {
    profile: TargetProfile,
    entry_slot: ProgramEntrySlotDeclaration,
    contents: UefiSystemTableNativeLayoutContents,
    layout_identity: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Construct the exact target-owned x86-64 native layout of
/// `EFI_SYSTEM_TABLE` for the UEFI application profile.
///
/// Other target profiles reject rather than inheriting firmware structure from
/// their shared x86-64 architecture. The returned plan is descriptive layout
/// evidence only and cannot validate or dereference a runtime table.
pub fn plan_uefi_system_table_native_layout(
    profile: TargetProfile,
) -> Result<ValidatedUefiSystemTableNativeLayout, Box<UefiSystemTableNativeLayoutError>> {
    let entry_slot = profile.program_entry_slot();
    let contents = match derive_contents(profile, entry_slot) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(UefiSystemTableNativeLayoutError {
                profile,
                diagnostic,
            }));
        }
    };
    let layout_identity = layout_identity(entry_slot, &contents);
    let candidate = Candidate {
        profile,
        entry_slot,
        contents,
        layout_identity,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(UefiSystemTableNativeLayoutError {
            profile: error.candidate.profile,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    profile: TargetProfile,
    entry_slot: ProgramEntrySlotDeclaration,
) -> Result<UefiSystemTableNativeLayoutContents, Diagnostic> {
    validate_target_owner(profile, entry_slot)?;
    Ok(UefiSystemTableNativeLayoutContents {
        fields: canonical_fields().to_vec(),
        table_header_size: TABLE_HEADER_SIZE,
        byte_size: TABLE_SIZE,
        alignment: TABLE_ALIGNMENT,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedUefiSystemTableNativeLayout, CandidateValidationError> {
    if let Err(diagnostic) = validate_target_owner(candidate.profile, candidate.entry_slot)
        .and_then(|()| validate_contents(&candidate.contents))
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.layout_identity != layout_identity(candidate.entry_slot, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error("UEFI system-table layout identity does not replay"),
        });
    }
    Ok(ValidatedUefiSystemTableNativeLayout {
        profile: candidate.profile,
        entry_slot: candidate.entry_slot,
        contents: candidate.contents,
        layout_identity: candidate.layout_identity,
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
    let canonical = canonical_fields();
    require(
        contents.fields.len() == FIELD_COUNT && contents.fields.as_slice() == canonical,
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

const fn canonical_fields() -> &'static [UefiSystemTableNativeFieldLayout; FIELD_COUNT] {
    &CANONICAL_FIELDS
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

const CANONICAL_FIELDS: [UefiSystemTableNativeFieldLayout; FIELD_COUNT] = [
    row(Field::HeaderSignature, 0, 0, 8, 8, Kind::UnsignedInteger),
    row(Field::HeaderRevision, 1, 8, 4, 4, Kind::UnsignedInteger),
    row(Field::HeaderSize, 2, 12, 4, 4, Kind::UnsignedInteger),
    row(Field::HeaderCrc32, 3, 16, 4, 4, Kind::UnsignedInteger),
    row(Field::HeaderReserved, 4, 20, 4, 4, Kind::ReservedZero),
    row(Field::FirmwareVendor, 5, 24, 8, 8, Kind::Pointer),
    row(Field::FirmwareRevision, 6, 32, 4, 4, Kind::UnsignedInteger),
    row(Field::FirmwareRevisionPadding, 7, 36, 4, 4, Kind::Padding),
    row(Field::ConsoleInHandle, 8, 40, 8, 8, Kind::Pointer),
    row(Field::ConsoleIn, 9, 48, 8, 8, Kind::Pointer),
    row(Field::ConsoleOutHandle, 10, 56, 8, 8, Kind::Pointer),
    row(Field::ConsoleOut, 11, 64, 8, 8, Kind::Pointer),
    row(Field::StandardErrorHandle, 12, 72, 8, 8, Kind::Pointer),
    row(Field::StandardError, 13, 80, 8, 8, Kind::Pointer),
    row(Field::RuntimeServices, 14, 88, 8, 8, Kind::Pointer),
    row(Field::BootServices, 15, 96, 8, 8, Kind::Pointer),
    row(
        Field::NumberOfTableEntries,
        16,
        104,
        8,
        8,
        Kind::UnsignedInteger,
    ),
    row(Field::ConfigurationTable, 17, 112, 8, 8, Kind::Pointer),
];

fn layout_identity(
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
            }),
    );
    hash_optional_str(hash, entry_slot.boundary_schema);
    hash.byte(
        entry_slot
            .physical_calling_convention
            .map_or(0, |calling| match calling {
                ProgramEntryCallingConvention::MicrosoftX64 => 1,
            }),
    );
    hash.byte(
        entry_slot
            .semantic_calling_convention
            .map_or(0, |calling| match calling {
                ProgramEntryCallingConvention::MicrosoftX64 => 1,
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

    fn candidate() -> Candidate {
        let profile = TargetProfile::UefiX64;
        let entry_slot = profile.program_entry_slot();
        let contents = derive_contents(profile, entry_slot).unwrap();
        let layout_identity = layout_identity(entry_slot, &contents);
        Candidate {
            profile,
            entry_slot,
            contents,
            layout_identity,
        }
    }

    #[test]
    fn uefi_x64_retains_the_complete_exact_system_table_layout() {
        let layout = plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap();
        assert_eq!(layout.profile(), TargetProfile::UefiX64);
        assert_eq!(layout.field_count(), FIELD_COUNT);
        assert_eq!(layout.table_header_size(), 24);
        assert_eq!(layout.known_prefix_byte_size(), 120);
        assert_eq!(layout.alignment(), 8);
        assert_ne!(layout.layout_identity(), 0);
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
        assert_eq!(layout.contents.fields.as_slice(), canonical_fields());
        validate_contents(&layout.contents).unwrap();
    }

    #[test]
    fn shared_architecture_profiles_cannot_claim_uefi_layout() {
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
    fn field_slot_aggregate_and_identity_drift_reject_recoverably() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|c| {
                c.contents.fields.pop();
            }),
            Box::new(|c| c.contents.fields.push(c.contents.fields[0])),
            Box::new(|c| c.contents.fields.swap(1, 2)),
            Box::new(|c| c.contents.fields[11].field = UefiSystemTableNativeField::ConsoleIn),
            Box::new(|c| c.contents.fields[11].ordinal ^= 1),
            Box::new(|c| c.contents.fields[11].byte_offset += 8),
            Box::new(|c| c.contents.fields[11].byte_size = 4),
            Box::new(|c| c.contents.fields[11].alignment = 4),
            Box::new(|c| {
                c.contents.fields[11].kind = UefiSystemTableNativeFieldKind::UnsignedInteger
            }),
            Box::new(|c| c.contents.table_header_size += 1),
            Box::new(|c| c.contents.byte_size += 8),
            Box::new(|c| c.contents.alignment = 4),
            Box::new(|c| c.entry_slot.owner = TargetProfile::WindowsX64),
            Box::new(|c| c.entry_slot.slot_name = "other_root"),
            Box::new(|c| c.entry_slot.schema = ProgramEntrySchema::HostedApplication),
            Box::new(|c| c.entry_slot.semantic_arrival_requirement = "OtherRoot::install"),
            Box::new(|c| c.entry_slot.physical_arrival_requirement = Some("Other::enter")),
            Box::new(|c| c.entry_slot.physical_contract_package = None),
            Box::new(|c| c.entry_slot.boundary_schema = Some("OtherBoundary")),
            Box::new(|c| c.entry_slot.physical_calling_convention = None),
            Box::new(|c| c.entry_slot.semantic_calling_convention = None),
            Box::new(|c| c.entry_slot.visible_parameters = ProgramEntryVisibleParameters::None),
            Box::new(|c| c.layout_identity ^= 1),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt UEFI native layout must reject fail closed");
            assert_eq!(error.candidate.profile, TargetProfile::UefiX64);
        }
    }

    #[test]
    fn field_lookup_is_exact_and_padding_remains_nonsemantic() {
        let layout = plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap();
        let padding = layout
            .field_layout(UefiSystemTableNativeField::FirmwareRevisionPadding)
            .unwrap();
        assert_eq!(padding.ordinal(), 7);
        assert_eq!(padding.byte_offset(), 36);
        assert_eq!(padding.byte_size(), 4);
        assert_eq!(padding.alignment(), 4);
        assert_eq!(padding.kind(), UefiSystemTableNativeFieldKind::Padding);
        assert_eq!(
            padding.field(),
            UefiSystemTableNativeField::FirmwareRevisionPadding
        );
    }
}
