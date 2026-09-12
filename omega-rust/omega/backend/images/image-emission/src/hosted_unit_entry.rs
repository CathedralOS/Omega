//! Physical hosted normal-completion adapters; semantic Unit stays value-free.
//! This compatibility mapping follows completed root cleanup; it is not a source
//! ProcessExit abandonment or closure of the canonical two-surface root contract.
//! Each adapter is selected by the exact target: the Darwin shim exits through
//! the hosted syscall path, while the PE shim relies on the Windows entry
//! convention where the loader maps the entry point's return value to the
//! process exit code. Both discard the semantic root's residual status register
//! so a value-free Unit return publishes physical status zero.

use diagnostics::Diagnostic;
use object_file::{
    ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
};

#[derive(Clone, Copy)]
pub(super) enum EntryShim {
    LinuxScalar(crate::LinuxX86ScalarExitShim),
    DarwinUnit {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    WindowsUnit {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
}

pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<Option<(ObjectPlan, Vec<u8>, EntryShim)>, Diagnostic> {
    if !crate::function_fragments::replay::has_free_unit_entry(artifact)? {
        return Ok(None);
    }
    if artifact.target == target::NativeTarget::macos_arm64() {
        return prepare_darwin(artifact).map(Some);
    }
    // `NativeTarget` deliberately collapses hosted Windows and UEFI x86-64 into
    // one PE32+ layout. A free Unit entry carries no parameters or entry claims,
    // so the same physical completion applies on both surfaces: the hosted
    // loader maps the entry return value to the process exit code, and the
    // EFI image entry maps it to EFI_STATUS. Returning zero is success under
    // either contract; parameterized EFI arrivals are not free Unit entries.
    if artifact.target == target::NativeTarget::windows_x64() {
        return prepare_windows(artifact).map(Some);
    }
    Ok(None)
}

fn append_entry_shim(
    artifact: &crate::ObjectArtifact,
    bytes: Vec<u8>,
    name: &str,
) -> Result<(ObjectPlan, Vec<u8>, ObjectSymbolHandle, usize), Diagnostic> {
    let mut object = artifact.object.clone();
    let mut text = artifact.text_bytes.clone();
    let offset = text.len();
    text.extend(bytes);
    let section = object
        .layout
        .sections
        .iter()
        .find(|(_, row)| row.kind == SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(|| Diagnostic::error("hosted Unit entry has no text section"))?;
    object.layout.sections.get_mut(section).size = text.len();
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset,
        size: text.len() - offset,
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    Ok((object, text, symbol, offset))
}

fn prepare_darwin(
    artifact: &crate::ObjectArtifact,
) -> Result<(ObjectPlan, Vec<u8>, EntryShim), Diagnostic> {
    let bytes = encode(
        artifact.text_bytes.len(),
        artifact.entry_function().text_offset,
    )?;
    let (object, text, symbol, offset) =
        append_entry_shim(artifact, bytes, "omega_darwin_unit_entry")?;
    Ok((object, text, EntryShim::DarwinUnit { symbol, offset }))
}

fn prepare_windows(
    artifact: &crate::ObjectArtifact,
) -> Result<(ObjectPlan, Vec<u8>, EntryShim), Diagnostic> {
    let bytes = encode_windows(
        artifact.text_bytes.len(),
        artifact.entry_function().text_offset,
    )?;
    let (object, text, symbol, offset) =
        append_entry_shim(artifact, bytes, "omega_windows_unit_entry")?;
    Ok((object, text, EntryShim::WindowsUnit { symbol, offset }))
}

/// `call entry; xor eax,eax; ret` under the ordinary Windows x64 entry ABI.
/// The loader enters with `rsp % 16 == 8`, so `sub rsp, 8` restores the
/// 16-byte call alignment the semantic entry was compiled against.
fn encode_windows(offset: usize, destination: usize) -> Result<Vec<u8>, Diagnostic> {
    let displacement = i64::try_from(destination)
        .ok()
        .zip(
            i64::try_from(offset)
                .ok()
                .and_then(|origin| origin.checked_add(9)),
        )
        .and_then(|(destination, origin)| destination.checked_sub(origin))
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| Diagnostic::error("Windows Unit entry call is outside rel32 range"))?;
    let mut bytes = vec![0x48, 0x83, 0xec, 0x08, 0xe8];
    bytes.extend(displacement.to_le_bytes());
    bytes.extend([0x31, 0xc0, 0x48, 0x83, 0xc4, 0x08, 0xc3]);
    Ok(bytes)
}

fn encode(offset: usize, destination: usize) -> Result<Vec<u8>, Diagnostic> {
    if !offset.is_multiple_of(4) || !destination.is_multiple_of(4) {
        return Err(Diagnostic::error(
            "Darwin Unit entry requires aligned instructions",
        ));
    }
    let displacement = i64::try_from(destination)
        .ok()
        .zip(i64::try_from(offset).ok())
        .and_then(|(destination, origin)| destination.checked_sub(origin))
        .filter(|value| value % 4 == 0 && (-134_217_728..134_217_728).contains(value))
        .ok_or_else(|| Diagnostic::error("Darwin Unit entry call is outside aligned BL range"))?;
    // The root has settled its normal-return frontier before physical completion.
    // No extra frame or source-level process-abandonment effect is introduced.
    Ok([
        0x9400_0000 | ((displacement / 4) as u32 & 0x03ff_ffff),
        0xd280_0000,
        0xd280_0030,
        0xd400_1001,
        0xd420_0000,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect())
}

fn decode(bytes: &[u8], offset: usize) -> Option<usize> {
    if bytes.len() != 20 || !offset.is_multiple_of(4) {
        return None;
    }
    let words: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect();
    if words[0] & 0xfc00_0000 != 0x9400_0000
        || words[1..] != [0xd280_0000, 0xd280_0030, 0xd400_1001, 0xd420_0000]
    {
        return None;
    }
    let displacement = ((words[0] << 6) as i32 >> 4) as i64;
    usize::try_from(i64::try_from(offset).ok()?.checked_add(displacement)?).ok()
}

fn decode_windows(bytes: &[u8], offset: usize) -> Option<usize> {
    if bytes.len() != 16
        || bytes[..5] != [0x48, 0x83, 0xec, 0x08, 0xe8]
        || bytes[9..] != [0x31, 0xc0, 0x48, 0x83, 0xc4, 0x08, 0xc3]
    {
        return None;
    }
    let displacement = i64::from(i32::from_le_bytes(bytes[5..9].try_into().ok()?));
    usize::try_from(
        i64::try_from(offset)
            .ok()?
            .checked_add(9)?
            .checked_add(displacement)?,
    )
    .ok()
}

fn unique_region(
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

pub(super) fn validate_darwin(
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(20)
        .ok_or_else(|| Diagnostic::error("Darwin entry range overflow"))?;
    if artifact.target != target::NativeTarget::macos_arm64()
        || !crate::function_fragments::replay::has_free_unit_entry(artifact)?
        || offset != artifact.text_bytes.len()
        || object.layout.entry_symbol != symbol
        || text
            .get(offset..end)
            .and_then(|bytes| decode(bytes, offset))
            != Some(artifact.entry_function().text_offset)
        || output.final_text_bytes.get(offset..end) != text.get(offset..end)
    {
        return Err(Diagnostic::error(
            "Darwin Unit entry lost exact source, call, or completion custody",
        ));
    }
    if !unique_region(object, symbol, offset, 20, output) {
        return Err(Diagnostic::error(
            "Darwin Unit entry has no unique executable region",
        ));
    }
    if !main_points_to(&output.bytes, offset) {
        return Err(Diagnostic::error(
            "Mach-O LC_MAIN does not select the exact Unit entry adapter",
        ));
    }
    Ok(())
}

pub(super) fn validate_windows(
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(16)
        .ok_or_else(|| Diagnostic::error("Windows entry range overflow"))?;
    if artifact.target != target::NativeTarget::windows_x64()
        || !crate::function_fragments::replay::has_free_unit_entry(artifact)?
        || offset != artifact.text_bytes.len()
        || object.layout.entry_symbol != symbol
        || text
            .get(offset..end)
            .and_then(|bytes| decode_windows(bytes, offset))
            != Some(artifact.entry_function().text_offset)
        || output.final_text_bytes.get(offset..end) != text.get(offset..end)
    {
        return Err(Diagnostic::error(
            "Windows Unit entry lost exact source, call, or completion custody",
        ));
    }
    if !unique_region(object, symbol, offset, 16, output) {
        return Err(Diagnostic::error(
            "Windows Unit entry has no unique executable region",
        ));
    }
    if !pe_entry_points_to(&output.bytes, offset, &text[offset..end]) {
        return Err(Diagnostic::error(
            "PE AddressOfEntryPoint does not select the exact Unit entry adapter",
        ));
    }
    Ok(())
}

/// Independently parse the emitted PE32+ headers: `AddressOfEntryPoint` must
/// select the shim inside `.text`, and the raw section bytes must carry it.
fn pe_entry_points_to(bytes: &[u8], shim_offset: usize, expected_shim: &[u8]) -> bool {
    fn word16(bytes: &[u8], offset: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
        ))
    }
    fn word32(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
        ))
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

fn main_points_to(bytes: &[u8], shim_offset: usize) -> bool {
    fn word(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
        ))
    }
    fn wide(bytes: &[u8], offset: usize) -> Option<u64> {
        Some(u64::from_le_bytes(
            bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
        ))
    }
    if word(bytes, 0) != Some(0xfeed_facf) || word(bytes, 4) != Some(0x0100_000c) {
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
    use super::*;
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
        bytes[192..200].copy_from_slice(&4160u64.to_le_bytes());
        assert!(main_points_to(&bytes, 64));
        assert!(!main_points_to(&bytes, 68));
        for offset in [0, 4, 16, 20, 36, 96, 104, 152, 184, 188, 192] {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(!main_points_to(&hostile, 64), "offset {offset}");
        }
        assert!(!main_points_to(&bytes[..207], 64));
    }
    #[test]
    fn darwin_unit_entry_overwrites_residual_status_without_an_extra_frame() {
        let bytes = encode(64, 0).unwrap();
        assert_eq!(decode(&bytes, 64), Some(0));
        assert_eq!(
            &bytes[4..],
            isa_aarch64::encode_hosted_exit_process_i32(target::NativeTarget::macos_arm64(), 0)
                .unwrap()
                .as_slice()
        );
        for bit in 0..160 {
            let mut hostile = bytes.clone();
            hostile[bit / 8] ^= 1 << (bit % 8);
            assert_ne!(decode(&hostile, 64), Some(0), "bit {bit}");
        }
        assert_ne!(decode(&bytes, 68), Some(0));
        assert!(encode(1, 0).is_err());
        assert!(encode(134_217_732, 0).is_err());
        assert!(decode(&bytes[..16], 64).is_none());
    }
    #[test]
    fn windows_unit_entry_calls_entry_and_returns_zero_status() {
        let bytes = encode_windows(64, 0).unwrap();
        assert_eq!(decode_windows(&bytes, 64), Some(0));
        // x86 needs no instruction alignment; the shim may start at any offset.
        let unaligned = encode_windows(0x1f, 0).unwrap();
        assert_eq!(decode_windows(&unaligned, 0x1f), Some(0));
        for bit in 0..128 {
            let mut hostile = unaligned.clone();
            hostile[bit / 8] ^= 1 << (bit % 8);
            assert_ne!(decode_windows(&hostile, 0x1f), Some(0), "bit {bit}");
        }
        assert_ne!(decode_windows(&unaligned, 0x20), Some(0));
        assert!(decode_windows(&unaligned[..12], 0x1f).is_none());
        assert!(encode_windows(usize::MAX - 4, 0).is_err());
        assert!(encode_windows(0x1_0000_0000, 0).is_err());
    }
    #[test]
    fn windows_pe_entry_selects_the_exact_adapter_bytes() {
        let shim = encode_windows(0x20, 0).unwrap();
        let mut bytes = vec![0u8; 0x400];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        bytes[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        bytes[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());
        let optional = coff + 20;
        bytes[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        bytes[optional + 16..optional + 20].copy_from_slice(&0x1020u32.to_le_bytes());
        let header = optional + 240;
        bytes[header..header + 8].copy_from_slice(b".text\0\0\0");
        bytes[header + 12..header + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[header + 16..header + 20].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[header + 20..header + 24].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[0x220..0x230].copy_from_slice(&shim);
        assert!(pe_entry_points_to(&bytes, 0x20, &shim));
        assert!(!pe_entry_points_to(&bytes, 0x24, &shim));
        assert!(!pe_entry_points_to(
            &bytes,
            0x20,
            &encode_windows(0x20, 4).unwrap()
        ));
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
}
