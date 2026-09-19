//! The report-only layout compatibility fingerprint and checked arithmetic.

use crate::dynamic_executable::load_placement::load_layout::{
    DYNAMIC_LOAD_POLICY_TAG, ElfLoadImageMemoryPlacement, ElfLoadProgramHeader,
    ElfPlacedDynamicSection, ElfResolvedSectionHeaderPlacement, FNV_OFFSET_BASIS, FNV_PRIME,
};
use crate::dynamic_executable::section_headers::relative_section_layout::ValidatedElfRelativeSectionPayloadLayout;
use diagnostics::Diagnostic;
use target::TargetProfile;

pub(crate) fn non_authoritative_layout_compatibility_fingerprint(
    relative: &ValidatedElfRelativeSectionPayloadLayout,
    target: TargetProfile,
    image_base: u64,
    max_page_alignment: u64,
    headers: &[ElfLoadProgramHeader],
    image_memory: &ElfLoadImageMemoryPlacement,
    sections: &[ElfPlacedDynamicSection],
    section_header_table_file_offset: u64,
    resolutions: &[ElfResolvedSectionHeaderPlacement],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.dynamic-load-layout.v1");
    hash.bytes(
        &relative
            .non_authoritative_layout_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.byte(target_tag(target));
    hash.byte(DYNAMIC_LOAD_POLICY_TAG);
    hash.bytes(&image_base.to_le_bytes());
    hash.bytes(&max_page_alignment.to_le_bytes());
    for header in headers {
        hash.byte(header.kind as u8);
        hash.bytes(&header.flags.to_le_bytes());
        hash.bytes(&header.file_offset.to_le_bytes());
        hash.bytes(&header.virtual_address.to_le_bytes());
        hash.bytes(&header.physical_address.to_le_bytes());
        hash.bytes(&header.file_size.to_le_bytes());
        hash.bytes(&header.memory_size.to_le_bytes());
        hash.bytes(&header.alignment.to_le_bytes());
    }
    hash.bytes(&image_memory.text_file_offset.to_le_bytes());
    hash.bytes(&image_memory.text_virtual_address.to_le_bytes());
    hash.bytes(&image_memory.text_size.to_le_bytes());
    hash.bytes(&image_memory.data_file_offset.to_le_bytes());
    hash.bytes(&image_memory.data_virtual_address.to_le_bytes());
    hash.bytes(&image_memory.data_size.to_le_bytes());
    hash.bytes(&image_memory.bss_virtual_address.to_le_bytes());
    hash.bytes(&image_memory.bss_size.to_le_bytes());
    hash.bytes(&image_memory.bss_alignment.to_le_bytes());
    for section in sections {
        hash.bytes(&section.index.to_le_bytes());
        hash.byte(section.kind as u8);
        hash.byte(section.region.map_or(0, |region| region as u8));
        hash.bytes(&section.file_offset.to_le_bytes());
        hash.byte(u8::from(section.virtual_address.is_some()));
        hash.bytes(&section.virtual_address.unwrap_or(0).to_le_bytes());
        hash.bytes(&section.byte_size.to_le_bytes());
        hash.bytes(&section.alignment.to_le_bytes());
    }
    for resolution in resolutions {
        hash.bytes(&resolution.row_index.to_le_bytes());
        hash.byte(resolution.section_kind as u8);
        hash.bytes(&(resolution.byte_offset as u64).to_le_bytes());
        hash.byte(resolution.byte_width);
        hash.byte(resolution.kind as u8);
        hash.bytes(&resolution.value.to_le_bytes());
    }
    hash.bytes(&section_header_table_file_offset.to_le_bytes());
    hash.finish()
}

const fn target_tag(target: TargetProfile) -> u8 {
    match target {
        TargetProfile::LinuxArm64 => 1,
        TargetProfile::LinuxX64 => 2,
        TargetProfile::MacosArm64 => 3,
        TargetProfile::WindowsX64 => 4,
        TargetProfile::UefiX64 => 5,
        TargetProfile::CrossPlatformCli => 6,
        TargetProfile::LocalUnchecked => 7,
        // Recognized but never ELF-realized: the tag reserves the profile
        // identity in the fingerprint vocabulary without an emission path.
        TargetProfile::AlphaBootstrap => 8,
    }
}

pub(crate) fn checked_sum(left: u64, right: u64, context: &str) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
}

pub(crate) fn checked_product(left: u64, right: u64, context: &str) -> Result<u64, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
}

pub(crate) fn checked_align(value: u64, alignment: u64, context: &str) -> Result<u64, Diagnostic> {
    if alignment <= 1 {
        return Ok(value);
    }
    require(
        alignment.is_power_of_two(),
        "ELF placement alignment is not a power of two",
    )?;
    let mask = alignment - 1;
    checked_sum(value, mask, context).map(|sum| sum & !mask)
}

pub(crate) fn require(condition: bool, message: &str) -> Result<(), Diagnostic> {
    if condition {
        Ok(())
    } else {
        Err(Diagnostic::error(message))
    }
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
