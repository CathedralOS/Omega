//! PIC shared-object layout, dynamic tables and symbol/relocation application.

use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageLayout, FinalImageSection, FinalImageSymbol};
use object_file::{RelocationKind, SymbolKind};
use target::{Architecture, NativeTarget};

use super::ElfComponentExport;
use crate::dynamic_executable::import_sections::dynamic_sections::{
    ElfDynamicSymbol, build_sysv_hash, derive_shared_import_contents,
};
use crate::imports::canonical_referenced_imports;

const PAGE: usize = 0x1_0000;
const HEADER: usize = 64 + 5 * 56;
const SECTION_COUNT: usize = 13;

#[derive(Clone, Default)]
struct Section {
    name: u32,
    kind: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
    link: u32,
    information: u32,
    alignment: u64,
    entry_size: u64,
}

pub(super) fn emit(
    image: &FinalImage,
    exports: &[ElfComponentExport],
) -> Result<Vec<u8>, Diagnostic> {
    let machine = if image.target == NativeTarget::linux_x64() {
        62
    } else if image.target == NativeTarget::linux_arm64() {
        183
    } else {
        return Err(invalid(
            "shared components require an exact Linux x86-64 or AArch64 target",
        ));
    };
    if exports.is_empty() {
        return Err(invalid("shared component has no selected exports"));
    }
    for (position, export) in exports.iter().enumerate() {
        if export.name.is_empty()
            || export.name.contains(&0)
            || export.slot_commitment == [0; 32]
            || exports[..position].iter().any(|prior| {
                prior.name == export.name || prior.slot_commitment == export.slot_commitment
            })
            || !image.symbol_table.symbols.is_valid(export.symbol)
        {
            return Err(invalid(
                "shared component has absent, duplicate or invalid export custody",
            ));
        }
        let symbol = image.symbol_table.symbols.get(export.symbol);
        if symbol.kind != SymbolKind::Function
            || symbol.section != FinalImageSection::Text
            || symbol.size == 0
            || symbol
                .offset
                .checked_add(symbol.size)
                .is_none_or(|end| end > image.memory.text.len())
        {
            return Err(invalid(
                "component export is not a complete emitted text function",
            ));
        }
    }
    if image
        .relocation_table
        .relocations
        .iter()
        .any(|(_, row)| row.kind == RelocationKind::Absolute64)
    {
        return Err(invalid(
            "component absolute pointers require loader-relative relocation custody",
        ));
    }
    if !image.memory.bss_alignment.is_power_of_two() {
        return Err(invalid("component BSS alignment is invalid"));
    }
    let imports = canonical_referenced_imports(image)?;
    let mut tables = derive_shared_import_contents(&imports)?;
    let mut exported_symbols = Vec::with_capacity(exports.len());
    for export in exports {
        let name = u32::try_from(tables.dynstr.len())
            .map_err(|_| invalid("component symbol strings overflow"))?;
        tables.dynstr.extend_from_slice(&export.name);
        tables.dynstr.push(0);
        let symbol = image.symbol_table.symbols.get(export.symbol);
        exported_symbols.push(tables.dynsym.len());
        tables.dynsym.push(ElfDynamicSymbol {
            name,
            info: 0x12,
            other: 3,
            section_index: 1,
            value: symbol.offset as u64,
            size: symbol.size as u64,
        });
        tables.versym.push(1);
    }
    let names = tables
        .dynsym
        .iter()
        .skip(1)
        .map(|symbol| string_at(&tables.dynstr, symbol.name))
        .collect::<Result<Vec<_>, _>>()?;
    tables.sysv_hash = build_sysv_hash(&names)?;
    let mut sections = vec![Section::default(); SECTION_COUNT];
    let mut output = vec![0; HEADER];
    let dynstr = append(&mut output, 1, &tables.dynstr)?;
    set_section(&mut sections, 4, 3, 2, dynstr, tables.dynstr.len(), 1);
    let dynsym = append(
        &mut output,
        8,
        &vec![
            0;
            tables
                .dynsym
                .len()
                .checked_mul(24)
                .ok_or_else(|| invalid("symbol table overflow"))?
        ],
    )?;
    set_section(&mut sections, 5, 11, 2, dynsym, tables.dynsym.len() * 24, 8);
    sections[5].link = 4;
    sections[5].information = 1;
    sections[5].entry_size = 24;
    let mut hash = Vec::new();
    for value in [tables.sysv_hash.bucket_count, tables.sysv_hash.chain_count]
        .into_iter()
        .chain(tables.sysv_hash.buckets.iter().copied())
        .chain(tables.sysv_hash.chains.iter().copied())
    {
        hash.extend(value.to_le_bytes());
    }
    let hash_offset = append(&mut output, 8, &hash)?;
    set_section(&mut sections, 6, 5, 2, hash_offset, hash.len(), 8);
    sections[6].link = 5;
    sections[6].entry_size = 4;
    let versions = tables
        .versym
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let version_offset = append(&mut output, 2, &versions)?;
    set_section(
        &mut sections,
        7,
        0x6fff_ffff,
        2,
        version_offset,
        versions.len(),
        2,
    );
    sections[7].link = 5;
    sections[7].entry_size = 2;
    let mut needs = Vec::new();
    for need in &tables.verneed {
        needs.extend(need.version.to_le_bytes());
        needs.extend(need.count.to_le_bytes());
        needs.extend(need.file.to_le_bytes());
        needs.extend(need.auxiliary_offset.to_le_bytes());
        needs.extend(need.next_offset.to_le_bytes());
        for auxiliary in &need.auxiliaries {
            needs.extend(auxiliary.hash.to_le_bytes());
            needs.extend(auxiliary.flags.to_le_bytes());
            needs.extend(auxiliary.other.to_le_bytes());
            needs.extend(auxiliary.name.to_le_bytes());
            needs.extend(auxiliary.next_offset.to_le_bytes());
        }
    }
    let need_offset = append(&mut output, 8, &needs)?;
    set_section(
        &mut sections,
        8,
        0x6fff_fffe,
        2,
        need_offset,
        needs.len(),
        8,
    );
    sections[8].link = 4;
    sections[8].information = tables.verneed.len() as u32;
    let relocation_size = imports
        .len()
        .checked_mul(24)
        .ok_or_else(|| invalid("component relocation table overflow"))?;
    let relocation_offset = append(&mut output, 8, &vec![0; relocation_size])?;
    set_section(
        &mut sections,
        9,
        4,
        2,
        relocation_offset,
        relocation_size,
        8,
    );
    sections[9].link = 5;
    sections[9].information = 10;
    sections[9].entry_size = 24;
    let read_only_end = output.len();
    let text_offset = align(output.len(), PAGE)?;
    let procedure_offset = align(image.memory.text.len(), 16)?;
    let text_size = procedure_offset
        .checked_add(
            imports
                .len()
                .checked_mul(16)
                .ok_or_else(|| invalid("component linkage size overflow"))?,
        )
        .ok_or_else(|| invalid("component text size overflow"))?;
    let data_offset = align(
        text_offset
            .checked_add(text_size)
            .ok_or_else(|| invalid("component load extent overflow"))?,
        PAGE,
    )?;
    let got_offset = align(
        data_offset
            .checked_add(image.memory.data.len())
            .ok_or_else(|| invalid("component data extent overflow"))?,
        8,
    )?;
    let got_size = imports
        .len()
        .checked_add(3)
        .and_then(|count| count.checked_mul(8))
        .ok_or_else(|| invalid("component GOT size overflow"))?;
    let dynamic_offset = got_offset
        .checked_add(got_size)
        .ok_or_else(|| invalid("component dynamic extent overflow"))?;
    let mut dynamic = tables
        .needed
        .iter()
        .map(|offset| (1u64, u64::from(*offset)))
        .collect::<Vec<_>>();
    dynamic.extend([
        (4, hash_offset as u64),
        (5, dynstr as u64),
        (6, dynsym as u64),
        (10, tables.dynstr.len() as u64),
        (11, 24),
        (3, got_offset as u64),
        (30, 8),
        (0x6fff_fffb, 1),
    ]);
    if !imports.is_empty() {
        dynamic.extend([
            (23, relocation_offset as u64),
            (2, relocation_size as u64),
            (20, 7),
        ]);
    }
    dynamic.push((0x6fff_fff0, version_offset as u64));
    if !tables.verneed.is_empty() {
        dynamic.extend([
            (0x6fff_fffe, need_offset as u64),
            (0x6fff_ffff, tables.verneed.len() as u64),
        ]);
    }
    dynamic.push((0, 0));
    let dynamic_size = dynamic
        .len()
        .checked_mul(16)
        .ok_or_else(|| invalid("component dynamic table overflow"))?;
    let initialized_end = dynamic_offset
        .checked_add(dynamic_size)
        .ok_or_else(|| invalid("component initialized extent overflow"))?;
    let bss_offset = align(initialized_end, image.memory.bss_alignment)?;
    let memory_end = bss_offset
        .checked_add(image.memory.bss_size)
        .ok_or_else(|| invalid("component BSS extent overflow"))?;
    let layout = FinalImageLayout {
        text_address: text_offset as u64,
        data_address: data_offset as u64,
        bss_address: bss_offset as u64,
    };
    let mut relocated = image.clone();
    relocated.memory.text.resize(text_size, 0);
    for (index, import) in imports.iter().enumerate() {
        let procedure = procedure_offset + index * 16;
        let got = got_offset + (index + 3) * 8;
        emit_procedure(
            &mut relocated.memory.text[procedure..procedure + 16],
            image.target.architecture,
            text_offset + procedure,
            got,
        )?;
        let symbol = relocated.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("__omega_component_import_{index}"),
            section: FinalImageSection::Text,
            offset: procedure,
            size: 16,
            kind: SymbolKind::Function,
        });
        let uses = relocated
            .relocation_table
            .relocations
            .iter()
            .filter(|(_, row)| row.symbol_handle == import.symbol_handle)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in uses {
            relocated
                .relocation_table
                .relocations
                .get_mut(handle)
                .symbol_handle = symbol;
        }
    }
    image::apply_relocations(&mut relocated, &layout, "shared ELF component")?;
    output.resize(initialized_end, 0);
    put64(&mut output, got_offset, dynamic_offset as u64);
    output[text_offset..text_offset + text_size].copy_from_slice(&relocated.memory.text);
    output[data_offset..data_offset + relocated.memory.data.len()]
        .copy_from_slice(&relocated.memory.data);
    for symbol in &mut tables.dynsym {
        if symbol.section_index == 1 {
            symbol.value = symbol
                .value
                .checked_add(text_offset as u64)
                .ok_or_else(|| invalid("component export address overflow"))?;
        }
    }
    for (index, symbol) in tables.dynsym.iter().enumerate() {
        let offset = dynsym + index * 24;
        put32(&mut output, offset, symbol.name);
        output[offset + 4] = symbol.info;
        output[offset + 5] = symbol.other;
        put16(&mut output, offset + 6, symbol.section_index);
        put64(&mut output, offset + 8, symbol.value);
        put64(&mut output, offset + 16, symbol.size);
    }
    for (index, binding) in tables.bindings.iter().enumerate() {
        let offset = relocation_offset + index * 24;
        put64(
            &mut output,
            offset,
            (got_offset + (binding.request_index + 3) * 8) as u64,
        );
        let kind = match image.target.architecture {
            Architecture::X86_64 => 7,
            Architecture::Aarch64 => 1026,
        };
        put64(
            &mut output,
            offset + 8,
            (u64::from(binding.dynamic_symbol_index) << 32) | kind,
        );
        put64(&mut output, offset + 16, 0);
    }
    for (index, (tag, value)) in dynamic.iter().enumerate() {
        put64(&mut output, dynamic_offset + index * 16, *tag);
        put64(&mut output, dynamic_offset + index * 16 + 8, *value);
    }
    set_section(&mut sections, 1, 1, 6, text_offset, text_size, 16);
    set_section(
        &mut sections,
        2,
        1,
        3,
        data_offset,
        image.memory.data.len(),
        16,
    );
    set_section(
        &mut sections,
        3,
        8,
        3,
        bss_offset,
        image.memory.bss_size,
        image.memory.bss_alignment,
    );
    set_section(&mut sections, 10, 1, 3, got_offset, got_size, 8);
    sections[10].entry_size = 8;
    set_section(&mut sections, 11, 6, 3, dynamic_offset, dynamic_size, 8);
    sections[11].link = 4;
    sections[11].entry_size = 16;
    let mut section_names = vec![0];
    for (index, name) in [
        ".text",
        ".data",
        ".bss",
        ".dynstr",
        ".dynsym",
        ".hash",
        ".gnu.version",
        ".gnu.version_r",
        ".rela.plt",
        ".got.plt",
        ".dynamic",
        ".shstrtab",
    ]
    .iter()
    .enumerate()
    {
        sections[index + 1].name =
            u32::try_from(section_names.len()).map_err(|_| invalid("section strings overflow"))?;
        section_names.extend(name.as_bytes());
        section_names.push(0);
    }
    let names_offset = append(&mut output, 1, &section_names)?;
    set_section(
        &mut sections,
        12,
        3,
        0,
        names_offset,
        section_names.len(),
        1,
    );
    sections[12].address = 0;
    let section_offset = align(output.len(), 8)?;
    output.resize(section_offset + SECTION_COUNT * 64, 0);
    for (index, section) in sections.iter().enumerate() {
        encode_section(
            &mut output[section_offset + index * 64..section_offset + (index + 1) * 64],
            section,
        );
    }
    encode_header(&mut output, machine, section_offset as u64);
    encode_program(
        &mut output[64..120],
        1,
        4,
        0,
        read_only_end as u64,
        read_only_end as u64,
        PAGE as u64,
    );
    encode_program(
        &mut output[120..176],
        1,
        5,
        text_offset as u64,
        text_size as u64,
        text_size as u64,
        PAGE as u64,
    );
    encode_program(
        &mut output[176..232],
        1,
        6,
        data_offset as u64,
        (initialized_end - data_offset) as u64,
        (memory_end - data_offset) as u64,
        PAGE as u64,
    );
    encode_program(
        &mut output[232..288],
        2,
        6,
        dynamic_offset as u64,
        dynamic_size as u64,
        dynamic_size as u64,
        8,
    );
    encode_program(&mut output[288..344], 0x6474_e551, 6, 0, 0, 0, 16);
    validate_header(&output, machine, section_offset)?;
    for index in exported_symbols {
        let symbol = tables.dynsym[index];
        if symbol.value < text_offset as u64
            || symbol.value + symbol.size > (text_offset + text_size) as u64
        {
            return Err(invalid("component export escaped its executable load"));
        }
    }
    Ok(output)
}

fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("ELF shared component: {reason}"))
}
fn align(value: usize, alignment: usize) -> Result<usize, Diagnostic> {
    if !alignment.is_power_of_two() {
        return Err(invalid("invalid section alignment"));
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(|| invalid("section alignment overflow"))
}
fn append(output: &mut Vec<u8>, alignment: usize, bytes: &[u8]) -> Result<usize, Diagnostic> {
    let offset = align(output.len(), alignment)?;
    offset
        .checked_add(bytes.len())
        .ok_or_else(|| invalid("section extent overflow"))?;
    output.resize(offset, 0);
    output.extend_from_slice(bytes);
    Ok(offset)
}
fn string_at(strings: &[u8], offset: u32) -> Result<&[u8], Diagnostic> {
    let remaining = strings
        .get(offset as usize..)
        .ok_or_else(|| invalid("dynamic symbol string offset is absent"))?;
    let end = remaining
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| invalid("dynamic symbol string is unterminated"))?;
    Ok(&remaining[..end])
}
fn set_section(
    sections: &mut [Section],
    index: usize,
    kind: u32,
    flags: u64,
    offset: usize,
    size: usize,
    alignment: usize,
) {
    sections[index].kind = kind;
    sections[index].flags = flags;
    sections[index].address = offset as u64;
    sections[index].offset = offset as u64;
    sections[index].size = size as u64;
    sections[index].alignment = alignment as u64;
}
fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn encode_header(bytes: &mut [u8], machine: u16, section_offset: u64) {
    bytes[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    put16(bytes, 16, 3);
    put16(bytes, 18, machine);
    put32(bytes, 20, 1);
    put64(bytes, 24, 0);
    put64(bytes, 32, 64);
    put64(bytes, 40, section_offset);
    put32(bytes, 48, 0);
    put16(bytes, 52, 64);
    put16(bytes, 54, 56);
    put16(bytes, 56, 5);
    put16(bytes, 58, 64);
    put16(bytes, 60, SECTION_COUNT as u16);
    put16(bytes, 62, 12);
}
fn encode_program(
    bytes: &mut [u8],
    kind: u32,
    flags: u32,
    offset: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
) {
    put32(bytes, 0, kind);
    put32(bytes, 4, flags);
    put64(bytes, 8, offset);
    put64(bytes, 16, offset);
    put64(bytes, 24, offset);
    put64(bytes, 32, file_size);
    put64(bytes, 40, memory_size);
    put64(bytes, 48, alignment);
}
fn encode_section(bytes: &mut [u8], section: &Section) {
    put32(bytes, 0, section.name);
    put32(bytes, 4, section.kind);
    put64(bytes, 8, section.flags);
    put64(bytes, 16, section.address);
    put64(bytes, 24, section.offset);
    put64(bytes, 32, section.size);
    put32(bytes, 40, section.link);
    put32(bytes, 44, section.information);
    put64(bytes, 48, section.alignment);
    put64(bytes, 56, section.entry_size);
}
fn emit_procedure(
    bytes: &mut [u8],
    architecture: Architecture,
    address: usize,
    got: usize,
) -> Result<(), Diagnostic> {
    match architecture {
        Architecture::X86_64 => {
            let displacement = i64::try_from(got)
                .ok()
                .and_then(|got| {
                    i64::try_from(address)
                        .ok()
                        .and_then(|address| got.checked_sub(address + 6))
                })
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| invalid("component procedure/GOT displacement exceeds rel32"))?;
            bytes.fill(0x90);
            bytes[..2].copy_from_slice(&[0xff, 0x25]);
            bytes[2..6].copy_from_slice(&displacement.to_le_bytes());
        }
        Architecture::Aarch64 => {
            let pages = i64::try_from(got >> 12)
                .ok()
                .zip(i64::try_from(address >> 12).ok())
                .and_then(|(got, address)| got.checked_sub(address))
                .filter(|value| (-(1 << 20)..(1 << 20)).contains(value))
                .ok_or_else(|| invalid("component procedure/GOT page displacement exceeds ADRP"))?;
            if got & 7 != 0 {
                return Err(invalid("component GOT slot is unaligned"));
            }
            let immediate = (pages as u32) & 0x1f_ffff;
            let adrp = 0x9000_0010 | ((immediate & 3) << 29) | ((immediate >> 2) << 5);
            let load = 0xf940_0211 | (((got as u32 & 0xfff) >> 3) << 10);
            for (index, instruction) in [adrp, load, 0xd61f_0220, 0xd503_201f]
                .into_iter()
                .enumerate()
            {
                put32(bytes, index * 4, instruction);
            }
        }
    }
    Ok(())
}
fn validate_header(bytes: &[u8], machine: u16, section_offset: usize) -> Result<(), Diagnostic> {
    let read16 = |offset| {
        u16::from_le_bytes(
            bytes[offset..offset + 2]
                .try_into()
                .expect("bounded ELF16 field"),
        )
    };
    let read64 = |offset| {
        u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("bounded ELF64 field"),
        )
    };
    if bytes.get(..7) != Some(&[0x7f, b'E', b'L', b'F', 2, 1, 1])
        || read16(16) != 3
        || read16(18) != machine
        || read64(24) != 0
        || read64(32) != 64
        || read64(40) != section_offset as u64
        || read16(56) != 5
        || read16(60) != SECTION_COUNT as u16
        || read16(62) != 12
    {
        return Err(invalid(
            "shared ELF header does not retain its exact container role",
        ));
    }
    let mut last_load_end = 0;
    for index in 0..5 {
        let offset = 64 + index * 56;
        let kind = u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("bounded program kind"),
        );
        let flags = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("bounded program flags"),
        );
        let file_offset = read64(offset + 8);
        let address = read64(offset + 16);
        let file_size = read64(offset + 32);
        let memory_size = read64(offset + 40);
        let alignment = read64(offset + 48);
        if kind == 3
            || file_offset != address
            || file_size > memory_size
            || flags & 3 == 3
            || file_offset
                .checked_add(file_size)
                .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err(invalid("component load permissions or extent are invalid"));
        }
        if kind == 1 {
            if alignment != PAGE as u64 || address < last_load_end {
                return Err(invalid("component load geometry overlaps"));
            }
            last_load_end = address
                .checked_add(memory_size)
                .ok_or_else(|| invalid("component load memory extent overflows"))?;
        }
    }
    Ok(())
}
