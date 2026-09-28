//! Replay the supported Mach-O segment mapping, before dyld fixups.
//!
//! This checks file/VM correspondence under the target's ordinary low-VM
//! segment mapping and zero-fill semantics. A common PIE slide preserves the
//! checked section offsets and disjoint intervals; absolute section alignment
//! additionally requires an appropriately aligned slide. Preferred addresses
//! are not claims about an already installed process. This does not validate
//! bind/rebase programs, authenticate providers, or grant installed root authority.
//!
//! Apple's `load_segment` rounds file mappings to pages before adding anonymous
//! zero-fill pages. Thus an S_ZEROFILL label alone cannot justify nonzero bytes
//! in the file-page tail overlapping BSS:
//! https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/mach_loader.c

use diagnostics::Diagnostic;
use image::FinalImageLayout;

use crate::file_layout::constants::MACHO_EXECUTABLE_BASE;
use crate::isa::MachoIsa;

fn invalid() -> Diagnostic {
    Diagnostic::error("Mach-O loader mapping differs from the supported final image")
}

pub(super) fn region(bytes: &[u8], offset: usize, count: usize) -> Result<&[u8], Diagnostic> {
    bytes
        .get(offset..offset.checked_add(count).ok_or_else(invalid)?)
        .ok_or_else(invalid)
}

pub(super) fn word(bytes: &[u8], offset: usize) -> Result<u32, Diagnostic> {
    Ok(u32::from_le_bytes(
        region(bytes, offset, 4)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}

pub(super) fn wide(bytes: &[u8], offset: usize) -> Result<u64, Diagnostic> {
    Ok(u64::from_le_bytes(
        region(bytes, offset, 8)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}

fn end(start: u64, count: u64) -> Result<u64, Diagnostic> {
    start.checked_add(count).ok_or_else(invalid)
}

fn aligned(value: u64, alignment: u64) -> Result<u64, Diagnostic> {
    Ok(end(value, alignment - 1)? & !(alignment - 1))
}

struct Segment<'bytes> {
    sections: &'bytes [u8],
    address: u64,
    memory_size: u64,
    file_offset: u64,
    file_size: u64,
}

impl<'bytes> Segment<'bytes> {
    fn read(
        command: &'bytes [u8],
        protection: u32,
        bytes: &[u8],
        page_size: u64,
    ) -> Result<Self, Diagnostic> {
        let count = usize::try_from(word(command, 64)?).map_err(|_| invalid())?;
        let section_bytes = count.checked_mul(80).ok_or_else(invalid)?;
        if command.len() != 72usize.checked_add(section_bytes).ok_or_else(invalid)?
            || word(command, 56)? != protection
            || word(command, 60)? != protection
            || word(command, 68)? != 0
        {
            return Err(invalid());
        }
        let segment = Self {
            sections: region(command, 72, section_bytes)?,
            address: wide(command, 24)?,
            memory_size: wide(command, 32)?,
            file_offset: wide(command, 40)?,
            file_size: wide(command, 48)?,
        };
        let page = page_size;
        if !segment.address.is_multiple_of(page)
            || !segment.memory_size.is_multiple_of(page)
            || !segment.file_offset.is_multiple_of(page)
            || segment.file_size > segment.memory_size
            || end(segment.file_offset, segment.file_size)? > bytes.len() as u64
        {
            return Err(invalid());
        }
        end(segment.address, segment.memory_size)?;
        Ok(segment)
    }
}

/// Validate the AArch64 image's closed segment roster and exact pre-fixup
/// text/data/BSS mapping. Borrows final bytes and layout; returns no
/// installation or execution authority. Dynamic bind/rebase destinations and
/// provider admission remain separate checks.
pub fn validate_macho_aarch64_loader_mapping(
    bytes: &[u8],
    layout: FinalImageLayout,
    final_text: &[u8],
    final_data: &[u8],
    bss_bytes: usize,
) -> Result<(), Diagnostic> {
    validate_macho_loader_mapping(
        bytes,
        layout,
        final_text,
        final_data,
        bss_bytes,
        MachoIsa::Aarch64,
    )
}

/// Validate the x86-64 image's closed segment roster and exact pre-fixup
/// text/data/BSS mapping under the same contract as the AArch64 check.
pub fn validate_macho_x86_64_loader_mapping(
    bytes: &[u8],
    layout: FinalImageLayout,
    final_text: &[u8],
    final_data: &[u8],
    bss_bytes: usize,
) -> Result<(), Diagnostic> {
    validate_macho_loader_mapping(
        bytes,
        layout,
        final_text,
        final_data,
        bss_bytes,
        MachoIsa::X86_64,
    )
}

pub(crate) fn validate_macho_loader_mapping(
    bytes: &[u8],
    layout: FinalImageLayout,
    final_text: &[u8],
    final_data: &[u8],
    bss_bytes: usize,
    isa: MachoIsa,
) -> Result<(), Diagnostic> {
    let page_size = isa.page_size();
    if word(bytes, 0)? != 0xfeed_facf
        || word(bytes, 4)? != isa.cpu_type()
        || word(bytes, 8)? != isa.cpu_subtype()
        || word(bytes, 12)? != 2
        || word(bytes, 24)? != 0x20_0085
        || word(bytes, 28)? != 0
    {
        return Err(invalid());
    }
    let commands_size = usize::try_from(word(bytes, 20)?).map_err(|_| invalid())?;
    let commands = region(bytes, 32, commands_size)?;
    let command_count = usize::try_from(word(bytes, 16)?).map_err(|_| invalid())?;
    if command_count > commands.len() / 8 {
        return Err(invalid());
    }
    let mut segments: [Option<Segment<'_>>; 4] = [None, None, None, None];
    let mut singletons = 0u16;
    let mut dylib_count = 0usize;
    let mut entry_file_offset = None;
    let mut cursor = 0usize;
    for _ in 0..command_count {
        let size = usize::try_from(word(commands, cursor.checked_add(4).ok_or_else(invalid)?)?)
            .map_err(|_| invalid())?;
        if size < 8 || !size.is_multiple_of(8) {
            return Err(invalid());
        }
        let command = region(commands, cursor, size)?;
        match word(command, 0)? {
            0x19 => {
                let (slot, protection) = match region(command, 8, 16)? {
                    b"__PAGEZERO\0\0\0\0\0\0" => (0, 0),
                    b"__TEXT\0\0\0\0\0\0\0\0\0\0" => (1, 5),
                    b"__DATA\0\0\0\0\0\0\0\0\0\0" => (2, 3),
                    b"__LINKEDIT\0\0\0\0\0\0" => (3, 1),
                    _ => return Err(invalid()),
                };
                if segments[slot]
                    .replace(Segment::read(command, protection, bytes, page_size)?)
                    .is_some()
                {
                    return Err(invalid());
                }
            }
            0xc => {
                if size < 32 || word(command, 8)? != 24 || !command[24..].contains(&0) {
                    return Err(invalid());
                }
                dylib_count += 1;
            }
            kind => {
                // Closed writer-supported commands. Their dynamic payloads are
                // not certified by this segment correspondence check.
                let (bit, expected_size) = match kind {
                    0xe => (0, 32),
                    0x1b => (1, 24),
                    0x32 => (2, 32),
                    0x8000_0028 => (3, 24),
                    0x8000_0022 => (4, 48),
                    0x2 => (5, 24),
                    0xb => (6, 80),
                    0x1d => (7, 16),
                    _ => return Err(invalid()),
                };
                if size != expected_size || singletons & (1 << bit) != 0 {
                    return Err(invalid());
                }
                singletons |= 1 << bit;
                if kind == 0xe
                    && (word(command, 8)? != 12 || region(command, 12, 14)? != b"/usr/lib/dyld\0")
                {
                    return Err(invalid());
                }
                if kind == 0x8000_0028 {
                    if wide(command, 16)? != 0 {
                        return Err(invalid());
                    }
                    entry_file_offset = Some(wide(command, 8)?);
                }
                if matches!(kind, 0x2 | 0xb) && command[8..].iter().any(|byte| *byte != 0) {
                    return Err(invalid());
                }
            }
        }
        cursor = cursor.checked_add(size).ok_or_else(invalid)?;
    }
    if cursor != commands.len() || singletons & 0xef != 0xef || dylib_count == 0 {
        return Err(invalid());
    }
    let [Some(pagezero), Some(text), data, Some(linkedit)] = segments else {
        return Err(invalid());
    };
    if pagezero.address != 0
        || pagezero.memory_size != MACHO_EXECUTABLE_BASE
        || pagezero.file_offset != 0
        || pagezero.file_size != 0
        || !pagezero.sections.is_empty()
        || text.address != MACHO_EXECUTABLE_BASE
        || text.file_offset != 0
        || text.memory_size != text.file_size
        || text.sections.len() != 80
    {
        return Err(invalid());
    }
    let text_file = layout
        .text_address
        .checked_sub(text.address)
        .ok_or_else(invalid)?;
    if text_file != aligned(end(32, commands_size as u64)?, 16)?
        || text.file_size != aligned(end(text_file, final_text.len() as u64)?, page_size)?
    {
        return Err(invalid());
    }
    validate_section(
        text.sections,
        b"__text\0\0\0\0\0\0\0\0\0\0",
        b"__TEXT\0\0\0\0\0\0\0\0\0\0",
        layout.text_address,
        final_text.len() as u64,
        text_file,
        0x8000_0400,
    )?;
    if word(text.sections, 52)? != 2 {
        return Err(invalid());
    }
    compare_file(bytes, text_file, final_text)?;
    let entry = entry_file_offset.ok_or_else(invalid)?;
    if entry < text_file
        || entry >= end(text_file, final_text.len() as u64)?
        || !entry.is_multiple_of(isa.entry_alignment())
    {
        return Err(invalid());
    }
    let text_end = end(text.address, text.memory_size)?;
    let mut mapped_end = text_end;
    let mut file_end = text.file_size;
    if let Some(data) = data {
        if final_data.is_empty() && bss_bytes == 0 {
            return Err(invalid());
        }
        if data.address != text_end
            || data.address != layout.data_address
            || data.file_offset != text.file_size
            || data.file_size != final_data.len() as u64
            || data.sections.len()
                != 80 * (usize::from(!final_data.is_empty()) + usize::from(bss_bytes != 0))
        {
            return Err(invalid());
        }
        let mut sections = data.sections.as_chunks::<80>().0.iter();
        if !final_data.is_empty() {
            let section = sections.next().ok_or_else(invalid)?;
            validate_section(
                section,
                b"__data\0\0\0\0\0\0\0\0\0\0",
                b"__DATA\0\0\0\0\0\0\0\0\0\0",
                data.address,
                data.file_size,
                data.file_offset,
                0,
            )?;
            if word(section, 52)? != 3 {
                return Err(invalid());
            }
        }
        let initialized_end = end(data.address, data.file_size)?;
        let storage_end = if bss_bytes != 0 {
            let section = sections.next().ok_or_else(invalid)?;
            validate_section(
                section,
                b"__bss\0\0\0\0\0\0\0\0\0\0\0",
                b"__DATA\0\0\0\0\0\0\0\0\0\0",
                layout.bss_address,
                bss_bytes as u64,
                0,
                1,
            )?;
            let alignment = 1u64.checked_shl(word(section, 52)?).ok_or_else(invalid)?;
            if layout.bss_address != aligned(initialized_end, alignment)? {
                return Err(invalid());
            }
            let storage_end = end(layout.bss_address, bss_bytes as u64)?;
            // The loader maps the final file page before allocating remaining
            // zero-fill pages. BSS in that file-page tail must itself be zero.
            let mapped_file_end = end(data.address, aligned(data.file_size, page_size)?)?;
            let tail_end = storage_end.min(mapped_file_end);
            if tail_end > layout.bss_address {
                let tail_offset = end(data.file_offset, layout.bss_address - data.address)?;
                let tail = region(
                    bytes,
                    usize::try_from(tail_offset).map_err(|_| invalid())?,
                    usize::try_from(tail_end - layout.bss_address).map_err(|_| invalid())?,
                )?;
                if tail.iter().any(|byte| *byte != 0) {
                    return Err(invalid());
                }
            }
            storage_end
        } else {
            initialized_end
        };
        mapped_end = end(data.address, data.memory_size)?;
        let required_end = aligned(storage_end, page_size)?;
        // An absent BSS section can leave alignment padding in the segment,
        // but does not introduce any semantic storage occurrence there.
        if mapped_end < required_end || (bss_bytes != 0 && mapped_end != required_end) {
            return Err(invalid());
        }
        compare_file(bytes, data.file_offset, final_data)?;
        file_end = end(data.file_offset, data.file_size)?;
    } else if !final_data.is_empty() || bss_bytes != 0 {
        return Err(invalid());
    }
    if linkedit.address != mapped_end
        || !linkedit.sections.is_empty()
        || linkedit.file_offset != aligned(file_end, page_size)?
        || end(linkedit.file_offset, linkedit.file_size)? != bytes.len() as u64
        || linkedit.memory_size != aligned(linkedit.file_size, page_size)?
    {
        return Err(invalid());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_section(
    section: &[u8],
    name: &[u8; 16],
    segment: &[u8; 16],
    address: u64,
    size: u64,
    file_offset: u64,
    flags: u32,
) -> Result<(), Diagnostic> {
    if region(section, 0, 16)? != name
        || region(section, 16, 16)? != segment
        || wide(section, 32)? != address
        || wide(section, 40)? != size
        || u64::from(word(section, 48)?) != file_offset
        || word(section, 64)? != flags
        || word(section, 56)? != 0
        || word(section, 60)? != 0
        || section[68..].iter().any(|byte| *byte != 0)
    {
        return Err(invalid());
    }
    Ok(())
}

fn compare_file(bytes: &[u8], offset: u64, expected: &[u8]) -> Result<(), Diagnostic> {
    if region(
        bytes,
        usize::try_from(offset).map_err(|_| invalid())?,
        expected.len(),
    )? != expected
    {
        return Err(invalid());
    }
    Ok(())
}
