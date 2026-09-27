//! Loader-declared entry addresses and loadable file ranges from container bytes.
//! These readers interpret the published ELF, PE, or Mach-O container, not evidence.

/// Little-endian readers over the published container — the container's own
/// declared fields, not evidence wire bytes. Reads are bounds-checked
/// arithmetic-free: a hostile header can only end the leg, never panic it.
fn container_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn container_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn container_u64(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

/// ELF64 little-endian: `e_entry` at header offset 24 is the loader-visible
/// entry virtual address. A null entry declares none.
fn elf64_entry_address(bytes: &[u8]) -> Option<u64> {
    let identification = bytes.get(..16)?;
    if identification[..4] != [0x7f, b'E', b'L', b'F']
        || identification[4] != 2
        || identification[5] != 1
    {
        return None;
    }
    let entry = container_u64(bytes, 24)?;
    (entry != 0).then_some(entry)
}

/// PE32+: the MZ stub's `e_lfanew` names the PE signature, the optional
/// header carries `AddressOfEntryPoint` as an RVA the loader relocates by
/// `ImageBase`. A null RVA declares no entry.
fn pe32_plus_entry_address(bytes: &[u8]) -> Option<u64> {
    if bytes.get(..2)? != *b"MZ" {
        return None;
    }
    let pe_offset = usize::try_from(container_u32(bytes, 0x3c)?).ok()?;
    if bytes.get(pe_offset..pe_offset.checked_add(4)?)? != *b"PE\0\0" {
        return None;
    }
    let optional = pe_offset.checked_add(24)?;
    if container_u16(bytes, optional)? != 0x20b {
        return None;
    }
    let entry_rva = container_u32(bytes, optional.checked_add(16)?)?;
    if entry_rva == 0 {
        return None;
    }
    let image_base = container_u64(bytes, optional.checked_add(24)?)?;
    image_base.checked_add(u64::from(entry_rva))
}

/// Mach-O 64 little-endian: `LC_MAIN` carries the entry's file offset, which
/// the `__TEXT` segment's `vmaddr`/`fileoff` pair maps to a virtual address.
/// A container with no `LC_MAIN` or no `__TEXT` segment declares no entry
/// claim this leg can check.
fn macho64_entry_address(bytes: &[u8]) -> Option<u64> {
    const LC_SEGMENT_64: u32 = 0x19;
    const LC_MAIN: u32 = 0x8000_0028;
    if container_u32(bytes, 0)? != 0xfeed_facf {
        return None;
    }
    let command_count = usize::try_from(container_u32(bytes, 16)?).ok()?;
    let mut command_offset = 32usize;
    let mut text_segment = None;
    let mut entry_file_offset = None;
    for _ in 0..command_count {
        let command = container_u32(bytes, command_offset)?;
        let command_size =
            usize::try_from(container_u32(bytes, command_offset.checked_add(4)?)?).ok()?;
        if command_size < 8 {
            return None;
        }
        match command {
            LC_SEGMENT_64 => {
                if bytes.get(command_offset.checked_add(8)?..command_offset.checked_add(24)?)?
                    == b"__TEXT\0\0\0\0\0\0\0\0\0\0"
                {
                    let vmaddr = container_u64(bytes, command_offset.checked_add(24)?)?;
                    let file_offset = container_u64(bytes, command_offset.checked_add(40)?)?;
                    text_segment = Some((vmaddr, file_offset));
                }
            }
            LC_MAIN => {
                entry_file_offset = Some(container_u64(bytes, command_offset.checked_add(8)?)?);
            }
            _ => {}
        }
        command_offset = command_offset.checked_add(command_size)?;
    }
    let (vmaddr, file_offset) = text_segment?;
    let in_segment = entry_file_offset?.checked_sub(file_offset)?;
    vmaddr.checked_add(in_segment)
}

/// The entry point the published container declares to its loader,
/// re-derived from the artifact bytes alone for the declared object format.
/// `None` means the bytes carry no checkable entry claim — a container that
/// does not parse as the declared format fails nothing new here, since every
/// other leg still binds whatever bytes sit under the declared extents.
pub(super) fn declared_entry_address(
    target: target::NativeTarget,
    executable_bytes: &[u8],
) -> Option<u64> {
    match target.object_format {
        target::ObjectFormat::Elf => elf64_entry_address(executable_bytes),
        target::ObjectFormat::Coff => pe32_plus_entry_address(executable_bytes),
        target::ObjectFormat::MachO => macho64_entry_address(executable_bytes),
    }
}

/// A loadable file range the published container itself declares to its
/// loader, with the access roles that range grants. The roles are the
/// container's own claims — the evidence only checks that its declared
/// extents sit inside ranges carrying the role the extent asserts.
pub(super) struct ContainerLoadableRange {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) executable: bool,
    pub(super) writable: bool,
}

/// ELF64 little-endian: every `PT_LOAD` program header maps
/// `[p_offset, p_offset + p_filesz)` with `p_flags` access bits (PF_X = 1,
/// PF_W = 2). A container with no `PT_LOAD` entries declares no loadable
/// claim this leg can check.
fn elf64_loadable_ranges(bytes: &[u8]) -> Option<Vec<ContainerLoadableRange>> {
    let identification = bytes.get(..16)?;
    if identification[..4] != [0x7f, b'E', b'L', b'F']
        || identification[4] != 2
        || identification[5] != 1
    {
        return None;
    }
    let header_offset = usize::try_from(container_u64(bytes, 32)?).ok()?;
    let entry_size = usize::from(container_u16(bytes, 54)?);
    if entry_size < 56 {
        return None;
    }
    let count = usize::from(container_u16(bytes, 56)?);
    let mut ranges = Vec::new();
    for index in 0..count {
        let entry = header_offset.checked_add(index.checked_mul(entry_size)?)?;
        if container_u32(bytes, entry)? != 1 {
            continue;
        }
        let flags = container_u32(bytes, entry.checked_add(4)?)?;
        let start = container_u64(bytes, entry.checked_add(8)?)?;
        let end = start.checked_add(container_u64(bytes, entry.checked_add(32)?)?)?;
        if end == start {
            continue;
        }
        ranges.push(ContainerLoadableRange {
            start,
            end,
            executable: flags & 1 != 0,
            writable: flags & 2 != 0,
        });
    }
    Some(ranges)
}

/// PE32+: every section header declares the raw file range it maps
/// (`PointerToRawData`, `SizeOfRawData`) under COFF characteristics
/// (`IMAGE_SCN_MEM_EXECUTE` = 0x2000_0000, `IMAGE_SCN_MEM_WRITE` =
/// 0x8000_0000). A container with no sections carrying raw bytes declares
/// no loadable claim this leg can check.
fn pe32_plus_loadable_ranges(bytes: &[u8]) -> Option<Vec<ContainerLoadableRange>> {
    if bytes.get(..2)? != *b"MZ" {
        return None;
    }
    let pe_offset = usize::try_from(container_u32(bytes, 0x3c)?).ok()?;
    if bytes.get(pe_offset..pe_offset.checked_add(4)?)? != *b"PE\0\0" {
        return None;
    }
    let coff = pe_offset.checked_add(4)?;
    let section_count = usize::from(container_u16(bytes, coff.checked_add(2)?)?);
    let optional_size = usize::from(container_u16(bytes, coff.checked_add(16)?)?);
    let mut section = coff.checked_add(20)?.checked_add(optional_size)?;
    let mut ranges = Vec::new();
    for _ in 0..section_count {
        let raw_size = container_u32(bytes, section.checked_add(16)?)?;
        let start = u64::from(container_u32(bytes, section.checked_add(20)?)?);
        let characteristics = container_u32(bytes, section.checked_add(36)?)?;
        section = section.checked_add(40)?;
        if raw_size == 0 {
            continue;
        }
        let end = start.checked_add(u64::from(raw_size))?;
        ranges.push(ContainerLoadableRange {
            start,
            end,
            executable: characteristics & 0x2000_0000 != 0,
            writable: characteristics & 0x8000_0000 != 0,
        });
    }
    Some(ranges)
}

/// Mach-O 64 little-endian: every `LC_SEGMENT_64` maps `[fileoff, fileoff +
/// filesize)` under its `initprot` bits (VM_PROT_EXECUTE = 4,
/// VM_PROT_WRITE = 2). A container declaring no mapped segment file range
/// carries no loadable claim this leg can check.
fn macho64_loadable_ranges(bytes: &[u8]) -> Option<Vec<ContainerLoadableRange>> {
    const LC_SEGMENT_64: u32 = 0x19;
    if container_u32(bytes, 0)? != 0xfeed_facf {
        return None;
    }
    let command_count = usize::try_from(container_u32(bytes, 16)?).ok()?;
    let mut command_offset = 32usize;
    let mut ranges = Vec::new();
    for _ in 0..command_count {
        let command = container_u32(bytes, command_offset)?;
        let command_size =
            usize::try_from(container_u32(bytes, command_offset.checked_add(4)?)?).ok()?;
        if command_size < 8 {
            return None;
        }
        if command == LC_SEGMENT_64 {
            let start = container_u64(bytes, command_offset.checked_add(40)?)?;
            let end = start.checked_add(container_u64(bytes, command_offset.checked_add(48)?)?)?;
            let initial_protection = container_u32(bytes, command_offset.checked_add(60)?)?;
            if end != start {
                ranges.push(ContainerLoadableRange {
                    start,
                    end,
                    executable: initial_protection & 4 != 0,
                    writable: initial_protection & 2 != 0,
                });
            }
        }
        command_offset = command_offset.checked_add(command_size)?;
    }
    Some(ranges)
}

/// The loadable file ranges the published container declares to its loader,
/// re-derived from the artifact bytes alone for the declared object format.
/// `None` means the bytes carry no checkable loadable claim — a container
/// that does not parse as the declared format fails nothing new here, since
/// every other leg still binds whatever bytes sit under the declared
/// extents.
pub(super) fn declared_loadable_ranges(
    target: target::NativeTarget,
    executable_bytes: &[u8],
) -> Option<Vec<ContainerLoadableRange>> {
    match target.object_format {
        target::ObjectFormat::Elf => elf64_loadable_ranges(executable_bytes),
        target::ObjectFormat::Coff => pe32_plus_loadable_ranges(executable_bytes),
        target::ObjectFormat::MachO => macho64_loadable_ranges(executable_bytes),
    }
}
