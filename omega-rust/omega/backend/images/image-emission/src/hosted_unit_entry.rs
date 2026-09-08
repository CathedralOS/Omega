//! Physical Darwin normal-completion adapter; semantic Unit stays value-free.
//! This compatibility mapping follows completed root cleanup; it is not a source
//! ProcessExit abandonment or closure of the canonical two-surface root contract.

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
}

pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<Option<(ObjectPlan, Vec<u8>, EntryShim)>, Diagnostic> {
    if artifact.target != target::NativeTarget::macos_arm64()
        || !crate::function_fragments::replay::has_free_unit_entry(artifact)?
    {
        return Ok(None);
    }
    let mut object = artifact.object.clone();
    let mut text = artifact.text_bytes.clone();
    let offset = text.len();
    text.extend(encode(offset, artifact.entry_function().text_offset)?);
    let section = object
        .layout
        .sections
        .iter()
        .find(|(_, row)| row.kind == SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(|| Diagnostic::error("Darwin Unit entry has no text section"))?;
    object.layout.sections.get_mut(section).size = text.len();
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: "omega_darwin_unit_entry".into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset,
        size: 20,
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    Ok(Some((
        object,
        text,
        EntryShim::DarwinUnit { symbol, offset },
    )))
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

pub(super) fn validate(
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
    let name = object_file::object_symbol_name(object, symbol);
    if output
        .executable_regions
        .regions
        .iter()
        .filter(|region| {
            region.origin == image::FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == name
                && region.section_offset == offset
                && region.byte_count == 20
        })
        .count()
        != 1
    {
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
}
