//! Physical hosted normal-completion adapters; semantic Unit stays value-free.
//! Each verified free Unit root receives disjoint image-owned stack backing
//! from its completed native demand. Normal completion restores the original
//! loader continuation before returning status zero or taking the selected
//! target's process-exit path. This is separate from source ProcessExit and
//! from explicitly supplied private-entry RAM recipes.

mod owned_stack;
use diagnostics::Diagnostic;
use object_file::{
    ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
};

/// One installed entry surface: the symbol and text offset every entry
/// surface retains, plus the surface that completes through it.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct EntryShim {
    pub(super) surface: EntrySurface,
    pub(super) symbol: ObjectSymbolHandle,
    pub(super) offset: usize,
    pub(super) stack: Option<owned_stack::HostedUnitStackPlan>,
}

/// The entry surface a shim completes through: physical arrival, one of the
/// hosted receiver shims, or a free Unit adapter. Each names the exact target
/// leg its encoder and header probe publish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntrySurface {
    Physical,
    ProvisionedUnit,
    DarwinReceiver,
    LinuxReceiver,
    LinuxArm64Receiver,
    WindowsReceiver,
    DarwinUnit,
    DarwinX86_64Unit,
    WindowsUnit,
    LinuxUnit,
    LinuxArm64Unit,
}

impl EntrySurface {
    /// The carrier row for one installed entry: the surface plus the exact
    /// symbol and text offset it completes over.
    pub(super) const fn shim(self, symbol: ObjectSymbolHandle, offset: usize) -> EntryShim {
        EntryShim {
            surface: self,
            symbol,
            offset,
            stack: None,
        }
    }
}

impl EntryShim {
    /// The shim's installed symbol and text offset. Every entry surface —
    /// physical arrival, hosted receiver, free Unit — retains these two
    /// coordinates, so replay reads them without re-matching the kind.
    pub(super) const fn symbol_offset(&self) -> (ObjectSymbolHandle, usize) {
        (self.symbol, self.offset)
    }
}

pub(super) struct PreparedEntry {
    pub object: ObjectPlan,
    pub text: Vec<u8>,
    pub relocations: object_file::RelocationPlan,
    pub shim: EntryShim,
}

/// The object, text and relocations the final image is built from: the
/// hosted-entry preparation's when it appended a bridge, the artifact's own
/// otherwise. One settlement serves every writer lane.
pub(super) struct PreparedParts<'a> {
    pub object: &'a ObjectPlan,
    pub text_bytes: &'a [u8],
    pub relocations: &'a object_file::RelocationPlan,
    pub entry_shim: Option<EntryShim>,
}

pub(super) fn prepared_parts<'a>(
    artifact: &'a crate::ObjectArtifact,
    prepared_entry: Option<&'a PreparedEntry>,
) -> PreparedParts<'a> {
    prepared_entry.map_or(
        PreparedParts {
            object: artifact.object(),
            text_bytes: artifact.text_bytes(),
            relocations: artifact.relocations(),
            entry_shim: None,
        },
        |prepared| PreparedParts {
            object: &prepared.object,
            text_bytes: prepared.text.as_slice(),
            relocations: &prepared.relocations,
            entry_shim: Some(prepared.shim.clone()),
        },
    )
}

/// One hosted Unit entry's symbol, retained identity and exact loader-header
/// probe. Stack backing and instruction custody share the ordinary entry owner.
struct UnitEntryForm {
    name: &'static str,
    surface: EntrySurface,
    header_check: fn(&image::EmittedImageOutput, usize, &[u8]) -> bool,
    header_error: &'static str,
}

/// Select the mechanics row for the artifact's target. `NativeTarget`
/// deliberately collapses hosted Windows and UEFI x86-64 into one PE32+
/// layout. A free Unit entry carries no parameters or entry claims, so the
/// same physical completion applies on both surfaces: the hosted loader maps
/// the entry return value to the process exit code, and the EFI image entry
/// maps it to EFI_STATUS. Returning zero is success under either contract;
/// parameterized EFI arrivals are not free Unit entries.
fn unit_entry_form(target: target::NativeTarget) -> Option<UnitEntryForm> {
    Some(if target == target::NativeTarget::macos_arm64() {
        UnitEntryForm {
            name: "omega_darwin_unit_entry",
            surface: EntrySurface::DarwinUnit,
            header_check: |output, offset, _expected| {
                main_points_to(&output.bytes, offset, target::NativeTarget::macos_arm64())
            },
            header_error: "Mach-O LC_MAIN does not select the exact Unit entry adapter",
        }
    } else if target == target::NativeTarget::macos_x64() {
        UnitEntryForm {
            name: "omega_darwin_x86_64_unit_entry",
            surface: EntrySurface::DarwinX86_64Unit,
            header_check: |output, offset, _expected| {
                main_points_to(&output.bytes, offset, target::NativeTarget::macos_x64())
            },
            header_error: "Mach-O LC_MAIN does not select the exact Darwin x86-64 Unit entry adapter",
        }
    } else if target == target::NativeTarget::windows_x64() {
        UnitEntryForm {
            name: "omega_windows_unit_entry",
            surface: EntrySurface::WindowsUnit,
            header_check: |output, offset, expected| {
                pe_entry_points_to(&output.bytes, offset, expected)
            },
            header_error: "PE AddressOfEntryPoint does not select the exact Unit entry adapter",
        }
    } else if target == target::NativeTarget::linux_x64() {
        UnitEntryForm {
            name: "omega_linux_x86_64_unit_entry",
            surface: EntrySurface::LinuxUnit,
            header_check: |output, offset, expected| {
                elf_entry_points_to(
                    &output.bytes,
                    62,
                    output.final_image_layout.text_address,
                    offset,
                    expected,
                )
            },
            header_error: "ELF e_entry does not select the exact Linux x86-64 Unit entry adapter",
        }
    } else if target == target::NativeTarget::linux_arm64() {
        UnitEntryForm {
            name: "omega_linux_arm64_unit_entry",
            surface: EntrySurface::LinuxArm64Unit,
            header_check: |output, offset, expected| {
                elf_entry_points_to(
                    &output.bytes,
                    183,
                    output.final_image_layout.text_address,
                    offset,
                    expected,
                )
            },
            header_error: "ELF e_entry does not select the exact Linux ARM64 Unit entry adapter",
        }
    } else {
        return None;
    })
}

pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<Option<PreparedEntry>, Diagnostic> {
    if artifact.physical_entry_binding().is_some() {
        return crate::physical_entry::prepare(artifact).map(Some);
    }
    if artifact.hosted_receiver_binding().is_some() {
        return crate::hosted_receiver::prepare(artifact).map(Some);
    }
    if !crate::function_fragments::replay::has_free_unit_entry(artifact)? {
        return Ok(None);
    }
    let Some(form) = unit_entry_form(artifact.target) else {
        return Ok(None);
    };
    owned_stack::prepare(artifact, &form).map(Some)
}

/// Append `bytes` to `.text`, grow the section, and install a fresh function
/// symbol as the object entry; returns the symbol and the shim's text offset.
/// Shared by the free-Unit adapters and the hosted-receiver bridges; the
/// caller names how a missing text section reports so each surface keeps its
/// own custody diagnostic.
pub(super) fn install_entry_shim(
    object: &mut ObjectPlan,
    text: &mut Vec<u8>,
    bytes: &[u8],
    name: &str,
    missing_text_section: &dyn Fn() -> Diagnostic,
) -> Result<(ObjectSymbolHandle, usize), Diagnostic> {
    let offset = text.len();
    text.extend_from_slice(bytes);
    let section = object
        .layout
        .sections
        .iter()
        .find(|(_, row)| row.kind == SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(missing_text_section)?;
    object.layout.sections.get_mut(section).size = text.len();
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset,
        size: bytes.len(),
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    Ok((symbol, offset))
}

pub(super) fn unique_region(
    object: &ObjectPlan,
    symbol: ObjectSymbolHandle,
    offset: usize,
    byte_count: usize,
    output: &image::EmittedImageOutput,
) -> bool {
    let name = object_file::object_symbol_name(object, symbol);
    output
        .executable_regions
        .regions
        .iter()
        .filter(|region| {
            region.origin == image::FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == name
                && region.section_offset == offset
                && region.byte_count == byte_count
        })
        .count()
        == 1
}

/// Rejoin exact stack backing, operand-free entry, saved loader continuation
/// and independently decoded physical completion under the selected header.
fn validate_unit_entry(
    form: &UnitEntryForm,
    target: target::NativeTarget,
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    relocations: &object_file::RelocationPlan,
    shim: EntryShim,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    owned_stack::validate(
        form,
        target,
        artifact,
        object,
        text,
        relocations,
        shim,
        output,
    )
}

/// Validate a free-Unit entry adapter from its own retained shim identity: the
/// adapter's variant is its exact target, so the mechanics row is read from
/// the shim rather than named again by a caller arm.
pub(super) fn validate_unit_shim(
    shim: EntryShim,
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    relocations: &object_file::RelocationPlan,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let target = match shim.surface {
        EntrySurface::ProvisionedUnit => artifact.target,
        EntrySurface::DarwinUnit => target::NativeTarget::macos_arm64(),
        EntrySurface::DarwinX86_64Unit => target::NativeTarget::macos_x64(),
        EntrySurface::WindowsUnit => target::NativeTarget::windows_x64(),
        EntrySurface::LinuxUnit => target::NativeTarget::linux_x64(),
        EntrySurface::LinuxArm64Unit => target::NativeTarget::linux_arm64(),
        _ => {
            return Err(Diagnostic::error(
                "Unit entry validation requires a free-Unit adapter shim",
            ));
        }
    };
    let form = unit_entry_form(target)
        .ok_or_else(|| Diagnostic::error("Unit entry has no mechanics row"))?;
    validate_unit_entry(
        &form,
        target,
        artifact,
        object,
        text,
        relocations,
        shim,
        output,
    )
}

/// Independently parse the emitted PE32+ headers: `AddressOfEntryPoint` must
/// select the shim inside `.text`, and the raw section bytes must carry it.
pub(super) fn pe_entry_points_to(bytes: &[u8], shim_offset: usize, expected_shim: &[u8]) -> bool {
    fn word16(bytes: &[u8], offset: usize) -> Option<u16> {
        crate::codec_bytes::u16_at(bytes, offset)
    }
    fn word32(bytes: &[u8], offset: usize) -> Option<u32> {
        crate::codec_bytes::u32_at(bytes, offset)
    }
    if bytes.get(0..2) != Some(b"MZ") {
        return false;
    }
    let Some(pe) = word32(bytes, 0x3c).and_then(|pe| usize::try_from(pe).ok()) else {
        return false;
    };
    let Some(signature_end) = pe.checked_add(4) else {
        return false;
    };
    if bytes.get(pe..signature_end) != Some(b"PE\0\0") {
        return false;
    }
    let Some(coff) = pe.checked_add(4) else {
        return false;
    };
    let Some(sections) = word16(bytes, coff + 2) else {
        return false;
    };
    let Some(optional_size) = word16(bytes, coff + 16) else {
        return false;
    };
    let Some(optional) = coff.checked_add(20) else {
        return false;
    };
    if word16(bytes, optional) != Some(0x20b) {
        return false;
    }
    let Some(entry_rva) = word32(bytes, optional + 16) else {
        return false;
    };
    let Some(first_section) = optional.checked_add(usize::from(optional_size)) else {
        return false;
    };
    let mut text = None;
    for index in 0..usize::from(sections) {
        let Some(header) = first_section
            .checked_add(index.saturating_mul(40))
            .filter(|header| header.checked_add(40).is_some_and(|end| end <= bytes.len()))
        else {
            return false;
        };
        if bytes.get(header..header + 8) != Some(b".text\0\0\0") {
            continue;
        }
        if text.is_some() {
            return false;
        }
        let Some(virtual_address) = word32(bytes, header + 12) else {
            return false;
        };
        let Some(raw_size) = word32(bytes, header + 16) else {
            return false;
        };
        let Some(raw_pointer) = word32(bytes, header + 20) else {
            return false;
        };
        text = Some((virtual_address, raw_size, raw_pointer));
    }
    let Some((virtual_address, raw_size, raw_pointer)) = text else {
        return false;
    };
    if u64::from(entry_rva) != u64::from(virtual_address).saturating_add(shim_offset as u64) {
        return false;
    }
    let Some(raw_start) = (raw_pointer as usize).checked_add(shim_offset) else {
        return false;
    };
    let Some(raw_end) = raw_start.checked_add(expected_shim.len()) else {
        return false;
    };
    if raw_end - (raw_pointer as usize) > raw_size as usize {
        return false;
    }
    bytes.get(raw_start..raw_end) == Some(expected_shim)
}

/// Independently parse the emitted ELF64 headers: `e_entry` must select the
/// shim inside an executable `PT_LOAD`, and the mapped file bytes must carry it.
/// Applies to both the static lane and the section-headered dynamic lane.
pub(super) fn elf_entry_points_to(
    bytes: &[u8],
    machine: u16,
    text_address: u64,
    shim_offset: usize,
    expected_shim: &[u8],
) -> bool {
    fn word16(bytes: &[u8], offset: usize) -> Option<u16> {
        crate::codec_bytes::u16_at(bytes, offset)
    }
    fn word64(bytes: &[u8], offset: usize) -> Option<u64> {
        crate::codec_bytes::u64_at(bytes, offset)
    }
    if bytes.get(0..4) != Some(&[0x7f, b'E', b'L', b'F'])
        || bytes.get(4) != Some(&2)
        || bytes.get(5) != Some(&1)
        || word16(bytes, 16) != Some(2)
        || word16(bytes, 18) != Some(machine)
    {
        return false;
    }
    let Some(entry) = word64(bytes, 24) else {
        return false;
    };
    if entry != text_address.saturating_add(shim_offset as u64) {
        return false;
    }
    let Some(program_headers) = word64(bytes, 32).and_then(|v| usize::try_from(v).ok()) else {
        return false;
    };
    let Some(header_size) = word16(bytes, 54).map(usize::from) else {
        return false;
    };
    let Some(header_count) = word16(bytes, 56).map(usize::from) else {
        return false;
    };
    let shim_file_offset = (0..header_count).find_map(|index| {
        let header = program_headers.checked_add(index.checked_mul(header_size)?)?;
        let kind = bytes
            .get(header..header.checked_add(4)?)
            .and_then(|v| TryInto::<[u8; 4]>::try_into(v).ok())
            .map(u32::from_le_bytes)?;
        let flags = bytes
            .get(header + 4..header.checked_add(8)?)
            .and_then(|v| TryInto::<[u8; 4]>::try_into(v).ok())
            .map(u32::from_le_bytes)?;
        let segment_offset = word64(bytes, header + 8)?;
        let segment_address = word64(bytes, header + 16)?;
        let segment_file_size = word64(bytes, header + 32)?;
        if kind != 1 || flags & 1 == 0 {
            return None;
        }
        if entry < segment_address || entry >= segment_address.checked_add(segment_file_size)? {
            return None;
        }
        let file_offset = segment_offset.checked_add(entry.checked_sub(segment_address)?)?;
        usize::try_from(file_offset).ok()
    });
    let Some(shim_file_offset) = shim_file_offset else {
        return false;
    };
    let Some(shim_file_end) = shim_file_offset.checked_add(expected_shim.len()) else {
        return false;
    };
    bytes.get(shim_file_offset..shim_file_end) == Some(expected_shim)
}

pub(super) fn main_points_to(
    bytes: &[u8],
    shim_offset: usize,
    target: target::NativeTarget,
) -> bool {
    fn word(bytes: &[u8], offset: usize) -> Option<u32> {
        crate::codec_bytes::u32_at(bytes, offset)
    }
    fn wide(bytes: &[u8], offset: usize) -> Option<u64> {
        crate::codec_bytes::u64_at(bytes, offset)
    }
    // CPU_TYPE_ARM64 or CPU_TYPE_X86_64, each with the ABI64 flag: the header
    // must name the exact target the adapter was emitted for.
    let cpu_type = if target == target::NativeTarget::macos_arm64() {
        0x0100_000c
    } else if target == target::NativeTarget::macos_x64() {
        0x0100_0007
    } else {
        return false;
    };
    if word(bytes, 0) != Some(0xfeed_facf) || word(bytes, 4) != Some(cpu_type) {
        return false;
    }
    let Some(count) = word(bytes, 16) else {
        return false;
    };
    let Some(commands_end) = word(bytes, 20)
        .and_then(|size| 32usize.checked_add(size as usize))
        .filter(|end| *end <= bytes.len())
    else {
        return false;
    };
    let mut cursor = 32usize;
    let mut entry = None;
    let mut text = None;
    for _ in 0..count {
        let Some(command) = word(bytes, cursor) else {
            return false;
        };
        let Some(size) = word(bytes, cursor + 4).and_then(|size| usize::try_from(size).ok()) else {
            return false;
        };
        let Some(end) = cursor
            .checked_add(size)
            .filter(|end| size >= 8 && size.is_multiple_of(8) && *end <= commands_end)
        else {
            return false;
        };
        if command == 0x8000_0028 {
            if entry.is_some() || size != 24 {
                return false;
            }
            entry = wide(bytes, cursor + 8);
        }
        if command == 0x19 {
            if size < 72 {
                return false;
            }
            let Some(sections) = word(bytes, cursor + 64) else {
                return false;
            };
            if (sections as usize)
                .checked_mul(80)
                .and_then(|size| size.checked_add(72))
                != Some(size)
            {
                return false;
            }
            for section in 0..sections as usize {
                let Some(section) = section
                    .checked_mul(80)
                    .and_then(|delta| cursor.checked_add(72)?.checked_add(delta))
                    .filter(|section| {
                        section
                            .checked_add(80)
                            .is_some_and(|end_section| end_section <= end)
                    })
                else {
                    return false;
                };
                if bytes.get(section..section + 16) == Some(b"__text\0\0\0\0\0\0\0\0\0\0") {
                    if text.is_some() {
                        return false;
                    }
                    text = word(bytes, section + 48).map(u64::from);
                }
            }
        }
        cursor = end;
    }
    cursor == commands_end
        && entry.is_some()
        && entry == text.and_then(|text| text.checked_add(shim_offset as u64))
}

#[cfg(test)]
mod tests {
    use super::{elf_entry_points_to, main_points_to, pe_entry_points_to};
    #[test]
    fn darwin_main_requires_exact_adapter_offset_and_target_header() {
        let mut bytes = vec![0u8; 208];
        for (offset, word) in [
            (0, 0xfeed_facfu32),
            (4, 0x0100_000c),
            (16, 2),
            (20, 176),
            (32, 0x19),
            (36, 152),
            (96, 1),
            (152, 4096),
            (184, 0x8000_0028),
            (188, 24),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes[104..120].copy_from_slice(b"__text\0\0\0\0\0\0\0\0\0\0");
        bytes[192..192 + 8].copy_from_slice(&4160u64.to_le_bytes());
        let arm64 = target::NativeTarget::macos_arm64();
        assert!(main_points_to(&bytes, 64, arm64));
        assert!(!main_points_to(&bytes, 68, arm64));
        // The same load commands under the other Mach-O ISA's header select
        // nothing for this target, and nothing for a non-Mach-O target.
        assert!(!main_points_to(
            &bytes,
            64,
            target::NativeTarget::macos_x64()
        ));
        assert!(!main_points_to(
            &bytes,
            64,
            target::NativeTarget::linux_x64()
        ));
        let mut intel = bytes.clone();
        intel[4..4 + 4].copy_from_slice(&0x0100_0007u32.to_le_bytes());
        assert!(main_points_to(
            &intel,
            64,
            target::NativeTarget::macos_x64()
        ));
        assert!(!main_points_to(&intel, 64, arm64));
        for offset in [0, 4, 16, 20, 36, 96, 104, 152, 184, 188, 192] {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(!main_points_to(&hostile, 64, arm64), "offset {offset}");
        }
        assert!(!main_points_to(&bytes[..207], 64, arm64));
    }
    #[test]
    fn windows_pe_entry_selects_the_exact_adapter_bytes() {
        let shim = vec![0x90; 16];
        let mut bytes = vec![0u8; 0x400];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x3c + 4].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        bytes[coff + 2..coff + 2 + 2].copy_from_slice(&1u16.to_le_bytes());
        bytes[coff + 16..coff + 16 + 2].copy_from_slice(&240u16.to_le_bytes());
        let optional = coff + 20;
        bytes[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        bytes[optional + 16..optional + 16 + 4].copy_from_slice(&0x1020u32.to_le_bytes());
        let header = optional + 240;
        bytes[header..header + 8].copy_from_slice(b".text\0\0\0");
        bytes[header + 12..header + 12 + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[header + 16..header + 16 + 4].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[header + 20..header + 20 + 4].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[0x220..0x230].copy_from_slice(&shim);
        assert!(pe_entry_points_to(&bytes, 0x20, &shim));
        assert!(!pe_entry_points_to(&bytes, 0x24, &shim));
        assert!(!pe_entry_points_to(&bytes, 0x20, &vec![0x91; 16]));
        for offset in [
            0,
            0x3c,
            0x80,
            coff + 16,
            optional,
            optional + 16,
            header,
            header + 12,
            header + 20,
            0x220,
        ] {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(
                !pe_entry_points_to(&hostile, 0x20, &shim),
                "offset {offset:#x}"
            );
        }
        assert!(!pe_entry_points_to(&bytes[..0x22f], 0x20, &shim));
    }
    #[test]
    fn linux_elf_entry_selects_the_exact_adapter_bytes() {
        // Static-lane geometry: 64-byte ELF header, two 56-byte phdrs, text
        // mapped from file offset 0 at IMAGE_BASE = 0x400000.
        let shim = vec![0x90; 19];
        let text_address = 0x401000u64;
        let mut bytes = vec![0u8; 0x2000];
        bytes[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[16..16 + 2].copy_from_slice(&2u16.to_le_bytes());
        bytes[18..18 + 2].copy_from_slice(&62u16.to_le_bytes());
        bytes[24..32].copy_from_slice(&(text_address + 0x140).to_le_bytes());
        bytes[32..32 + 8].copy_from_slice(&64u64.to_le_bytes());
        bytes[54..54 + 2].copy_from_slice(&56u16.to_le_bytes());
        bytes[56..56 + 2].copy_from_slice(&2u16.to_le_bytes());
        // RX PT_LOAD: whole file mapped at 0x400000.
        bytes[64..64 + 4].copy_from_slice(&1u32.to_le_bytes());
        bytes[68..68 + 4].copy_from_slice(&5u32.to_le_bytes());
        bytes[72..72 + 8].copy_from_slice(&0u64.to_le_bytes());
        bytes[80..80 + 8].copy_from_slice(&0x400000u64.to_le_bytes());
        bytes[96..96 + 8].copy_from_slice(&0x1200u64.to_le_bytes());
        // RW PT_LOAD for .data.
        bytes[120..120 + 4].copy_from_slice(&1u32.to_le_bytes());
        bytes[124..124 + 4].copy_from_slice(&6u32.to_le_bytes());
        bytes[128..128 + 8].copy_from_slice(&0x2000u64.to_le_bytes());
        bytes[136..136 + 8].copy_from_slice(&0x402000u64.to_le_bytes());
        bytes[152..152 + 8].copy_from_slice(&0x100u64.to_le_bytes());
        bytes[0x1140..0x1140 + 19].copy_from_slice(&shim);
        assert!(elf_entry_points_to(&bytes, 62, text_address, 0x140, &shim));
        assert!(!elf_entry_points_to(&bytes, 62, text_address, 0x144, &shim));
        assert!(!elf_entry_points_to(
            &bytes,
            183,
            text_address,
            0x140,
            &shim
        ));
        assert!(!elf_entry_points_to(
            &bytes,
            62,
            text_address,
            0x140,
            &vec![0x91; 19]
        ));
        for offset in [0, 4, 16, 18, 24, 64, 68, 0x1140] {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(
                !elf_entry_points_to(&hostile, 62, text_address, 0x140, &shim),
                "offset {offset:#x}"
            );
        }
        assert!(!elf_entry_points_to(
            &bytes[..0x1140 + 18],
            62,
            text_address,
            0x140,
            &shim
        ));
    }
}
