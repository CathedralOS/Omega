//! Target-owned native layout of the UEFI x86-64 Boot Services table.
//!
//! The UEFI specification fixes one `EFI_TABLE_HEADER` followed by forty-four
//! pointer-sized service slots. This module retains that complete 376-byte
//! x86-64 layout as descriptive evidence. It neither inspects a runtime table
//! nor grants permission to invoke a retained function pointer.
//!
//! [UEFI Boot Services table]: https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html#efi-boot-services-table

use crate::{
    Architecture, ObjectFormat, ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntrySlotDeclaration, TargetProfile,
};
use diagnostics::Diagnostic;

const FIELD_COUNT: usize = 49;
const TABLE_HEADER_SIZE: u32 = 24;
const TABLE_SIZE: u32 = 376;
const TABLE_ALIGNMENT: u32 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Structured UEFI protocol identity in specification field order. It is a
/// name, not a pointer, interface occurrence, or provider admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct UefiProtocolGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

/// Target-owned identity and stable native operand used for the image-handle
/// `HandleProtocol` query.
pub static UEFI_LOADED_IMAGE_PROTOCOL_GUID: UefiProtocolGuid = UefiProtocolGuid {
    data1: 0x5b1b_31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UefiBootServicesNativeFieldKind {
    UnsignedInteger = 1,
    FunctionPointer = 2,
    ReservedZero = 3,
    ReservedPointer = 4,
}

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
    let fields = canonical_fields().to_vec();
    if let Err(diagnostic) = validate_fields(&fields) {
        return Err(Box::new(UefiBootServicesNativeLayoutError {
            profile,
            diagnostic,
        }));
    }
    Ok(ValidatedUefiBootServicesNativeLayout {
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
        "EFI_BOOT_SERVICES native layout is owned only by the UEFI x86-64 target",
    )?;
    require(
        entry_slot == TargetProfile::UefiX64.program_entry_slot()
            && entry_slot.physical_arrival_requirement == Some("UefiPhysicalEntry::enter")
            && entry_slot.physical_contract_package
                == Some(ProgramEntryPhysicalContractPackage::UefiX64)
            && entry_slot.physical_calling_convention
                == Some(ProgramEntryCallingConvention::MicrosoftX64),
        "EFI_BOOT_SERVICES native layout drifted from its target-owned physical entry",
    )
}

fn validate_fields(fields: &[UefiBootServicesNativeFieldLayout]) -> Result<(), Diagnostic> {
    require(
        fields == canonical_fields(),
        "EFI_BOOT_SERVICES field catalog is missing, duplicated, reordered, or drifted",
    )?;
    let mut prior_end = 0_u32;
    for (ordinal, row) in fields.iter().enumerate() {
        require(
            usize::from(row.ordinal) == ordinal
                && row.byte_size != 0
                && row.alignment.is_power_of_two()
                && row.byte_offset % row.alignment == 0
                && row.byte_offset >= prior_end,
            "EFI_BOOT_SERVICES field order, alignment, or geometry is invalid",
        )?;
        prior_end = row
            .byte_offset
            .checked_add(row.byte_size)
            .ok_or_else(|| Diagnostic::error("EFI_BOOT_SERVICES field end overflows u32"))?;
    }
    require(
        prior_end == TABLE_SIZE,
        "EFI_BOOT_SERVICES field catalog does not exactly cover its aggregate",
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

use UefiBootServicesNativeField as Field;
use UefiBootServicesNativeFieldKind as Kind;

const CANONICAL_FIELDS: [UefiBootServicesNativeFieldLayout; FIELD_COUNT] = [
    row(Field::HeaderSignature, 0, 0, 8, 8, Kind::UnsignedInteger),
    row(Field::HeaderRevision, 1, 8, 4, 4, Kind::UnsignedInteger),
    row(Field::HeaderSize, 2, 12, 4, 4, Kind::UnsignedInteger),
    row(Field::HeaderCrc32, 3, 16, 4, 4, Kind::UnsignedInteger),
    row(Field::HeaderReserved, 4, 20, 4, 4, Kind::ReservedZero),
    row(Field::RaiseTpl, 5, 24, 8, 8, Kind::FunctionPointer),
    row(Field::RestoreTpl, 6, 32, 8, 8, Kind::FunctionPointer),
    row(Field::AllocatePages, 7, 40, 8, 8, Kind::FunctionPointer),
    row(Field::FreePages, 8, 48, 8, 8, Kind::FunctionPointer),
    row(Field::GetMemoryMap, 9, 56, 8, 8, Kind::FunctionPointer),
    row(Field::AllocatePool, 10, 64, 8, 8, Kind::FunctionPointer),
    row(Field::FreePool, 11, 72, 8, 8, Kind::FunctionPointer),
    row(Field::CreateEvent, 12, 80, 8, 8, Kind::FunctionPointer),
    row(Field::SetTimer, 13, 88, 8, 8, Kind::FunctionPointer),
    row(Field::WaitForEvent, 14, 96, 8, 8, Kind::FunctionPointer),
    row(Field::SignalEvent, 15, 104, 8, 8, Kind::FunctionPointer),
    row(Field::CloseEvent, 16, 112, 8, 8, Kind::FunctionPointer),
    row(Field::CheckEvent, 17, 120, 8, 8, Kind::FunctionPointer),
    row(
        Field::InstallProtocolInterface,
        18,
        128,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::ReinstallProtocolInterface,
        19,
        136,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::UninstallProtocolInterface,
        20,
        144,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(Field::HandleProtocol, 21, 152, 8, 8, Kind::FunctionPointer),
    row(Field::Reserved, 22, 160, 8, 8, Kind::ReservedPointer),
    row(
        Field::RegisterProtocolNotify,
        23,
        168,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(Field::LocateHandle, 24, 176, 8, 8, Kind::FunctionPointer),
    row(
        Field::LocateDevicePath,
        25,
        184,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::InstallConfigurationTable,
        26,
        192,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(Field::LoadImage, 27, 200, 8, 8, Kind::FunctionPointer),
    row(Field::StartImage, 28, 208, 8, 8, Kind::FunctionPointer),
    row(Field::Exit, 29, 216, 8, 8, Kind::FunctionPointer),
    row(Field::UnloadImage, 30, 224, 8, 8, Kind::FunctionPointer),
    row(
        Field::ExitBootServices,
        31,
        232,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::GetNextMonotonicCount,
        32,
        240,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(Field::Stall, 33, 248, 8, 8, Kind::FunctionPointer),
    row(
        Field::SetWatchdogTimer,
        34,
        256,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::ConnectController,
        35,
        264,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::DisconnectController,
        36,
        272,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(Field::OpenProtocol, 37, 280, 8, 8, Kind::FunctionPointer),
    row(Field::CloseProtocol, 38, 288, 8, 8, Kind::FunctionPointer),
    row(
        Field::OpenProtocolInformation,
        39,
        296,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::ProtocolsPerHandle,
        40,
        304,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::LocateHandleBuffer,
        41,
        312,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(Field::LocateProtocol, 42, 320, 8, 8, Kind::FunctionPointer),
    row(
        Field::InstallMultipleProtocolInterfaces,
        43,
        328,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(
        Field::UninstallMultipleProtocolInterfaces,
        44,
        336,
        8,
        8,
        Kind::FunctionPointer,
    ),
    row(Field::CalculateCrc32, 45, 344, 8, 8, Kind::FunctionPointer),
    row(Field::CopyMem, 46, 352, 8, 8, Kind::FunctionPointer),
    row(Field::SetMem, 47, 360, 8, 8, Kind::FunctionPointer),
    row(Field::CreateEventEx, 48, 368, 8, 8, Kind::FunctionPointer),
];

const fn canonical_fields() -> &'static [UefiBootServicesNativeFieldLayout; FIELD_COUNT] {
    &CANONICAL_FIELDS
}

fn layout_report_fingerprint(
    entry_slot: ProgramEntrySlotDeclaration,
    fields: &[UefiBootServicesNativeFieldLayout],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.uefi-x64-boot-services-native-layout.v1");
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
    hash.bytes(&TABLE_HEADER_SIZE.to_le_bytes());
    hash.bytes(&TABLE_SIZE.to_le_bytes());
    hash.bytes(&TABLE_ALIGNMENT.to_le_bytes());
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
    fn uefi_x64_retains_complete_boot_services_layout() {
        let layout = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap();
        assert_eq!(std::mem::size_of::<UefiProtocolGuid>(), 16);
        assert_eq!(std::mem::align_of::<UefiProtocolGuid>(), 4);
        assert_eq!(layout.field_count(), 49);
        assert_eq!(layout.table_header_size(), 24);
        assert_eq!(layout.known_prefix_byte_size(), 376);
        assert_eq!(layout.alignment(), 8);
        assert_eq!(
            layout
                .field_layout(Field::AllocatePages)
                .unwrap()
                .byte_offset(),
            40
        );
        let handle = layout.field_layout(Field::HandleProtocol).unwrap();
        assert_eq!(
            (
                handle.ordinal(),
                handle.byte_offset(),
                handle.byte_size(),
                handle.alignment(),
                handle.kind()
            ),
            (21, 152, 8, 8, Kind::FunctionPointer)
        );
        assert_eq!(
            layout
                .field_layout(Field::CreateEventEx)
                .unwrap()
                .byte_offset(),
            368
        );
        assert_ne!(layout.non_authoritative_layout_report_fingerprint(), 0);
        assert_eq!(UEFI_LOADED_IMAGE_PROTOCOL_GUID.data1, 0x5b1b_31a1);
        assert!(layout.matches_exact_plan(
            &plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap()
        ));
    }

    #[test]
    fn non_uefi_profiles_reject_boot_services_layout() {
        for profile in [
            TargetProfile::LinuxX64,
            TargetProfile::WindowsX64,
            TargetProfile::LocalUnchecked,
        ] {
            let error = plan_uefi_boot_services_native_layout(profile).unwrap_err();
            assert_eq!(error.profile(), profile);
        }
    }

    #[test]
    fn canonical_catalog_exactly_covers_aggregate() {
        validate_fields(canonical_fields()).unwrap();
        let mut drift = canonical_fields().to_vec();
        drift[21].byte_offset += 8;
        assert!(
            validate_fields(&drift)
                .unwrap_err()
                .message
                .contains("drifted")
        );
    }
}
