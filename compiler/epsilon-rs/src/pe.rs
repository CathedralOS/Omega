// Windows PE32+ image emission (one of the per-format image writers; a future
// elf.rs / macho.rs would be siblings). Deterministic: TimeDateStamp = 0, no
// timestamps, fixed field order.
//
// Two paths:
//  - no imports (slices 1-2): a single .text section (byte-identical to before).
//  - with imports (slice 3+): .text + .rdata (strings + a kernel32 import table),
//    with RIP-relative relocations in .text patched once RVAs are known.

use crate::util::align_up;
use crate::x64::{ImportFunction, LoweredProgram, RelocationTarget};

const FILE_ALIGN: u32 = 0x200;
const SECT_ALIGN: u32 = 0x1000;
const IMAGE_BASE: u64 = 0x1_4000_0000;
const TEXT_CHARS: u32 = 0x6000_0020; // CNT_CODE | MEM_EXECUTE | MEM_READ
const RDATA_CHARS: u32 = 0x4000_0040; // CNT_INITIALIZED_DATA | MEM_READ
const DATA_CHARS: u32 = 0xC000_0040; // CNT_INITIALIZED_DATA | MEM_READ | MEM_WRITE

struct Section {
    name: [u8; 8],
    rva: u32,
    data: Vec<u8>,
    characteristics: u32,
}

fn section_name(name: &[u8]) -> [u8; 8] {
    let mut padded = [0u8; 8];
    padded[..name.len()].copy_from_slice(name);
    padded
}
fn write_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn write_u64_le(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn align_to_2(value: usize) -> usize {
    (value + 1) & !1
}
fn align_to_8(value: usize) -> usize {
    (value + 7) & !7
}

pub fn build_pe(lowered: &LoweredProgram) -> Vec<u8> {
    let text_rva = SECT_ALIGN; // 0x1000
    let mut code = lowered.code.clone();
    let mut next_rva = text_rva + align_up(code.len() as u32, SECT_ALIGN);

    // Optional .rdata (string literals + the kernel32 import table).
    let mut rdata_layout: Option<RdataLayout> = None;
    let mut rdata_rva = 0;
    if lowered.uses_imports {
        rdata_rva = next_rva;
        let rdata = build_rdata_section(&lowered.rodata, rdata_rva);
        next_rva += align_up(rdata.bytes.len() as u32, SECT_ALIGN);
        rdata_layout = Some(rdata);
    }

    // Optional .data (the entry machine's zero-initialized `self` instance).
    let mut data_rva = 0;
    if lowered.data_size > 0 {
        data_rva = next_rva;
        next_rva += align_up(lowered.data_size, SECT_ALIGN);
    }
    let _ = next_rva;

    // Patch RIP-relative disp32s now that all RVAs are fixed.
    for relocation in &lowered.relocations {
        let target_rva = match &relocation.target {
            RelocationTarget::Rodata(offset) => rdata_rva + offset,
            RelocationTarget::Import(import_function) => {
                rdata_layout.as_ref().unwrap().address_table_rva
                    + match import_function {
                        ImportFunction::GetStdHandle => 0,
                        ImportFunction::WriteFile => 8,
                        ImportFunction::ReadFile => 16,
                    }
            }
            RelocationTarget::Data(offset) => data_rva + offset,
        };
        let rip = text_rva + relocation.patch_offset + 4; // RVA of the byte after the disp32
        let displacement = target_rva as i64 - rip as i64;
        let offset = relocation.patch_offset as usize;
        code[offset..offset + 4].copy_from_slice(&(displacement as i32).to_le_bytes());
    }

    let mut sections = vec![Section {
        name: section_name(b".text"),
        rva: text_rva,
        data: code,
        characteristics: TEXT_CHARS,
    }];
    let mut data_dirs = Vec::new();
    if let Some(rdata) = rdata_layout {
        data_dirs.push((1, rdata.import_directory_rva, rdata.import_directory_size));
        data_dirs.push((12, rdata.address_table_rva, rdata.address_table_size));
        sections.push(Section {
            name: section_name(b".rdata"),
            rva: rdata_rva,
            data: rdata.bytes,
            characteristics: RDATA_CHARS,
        });
    }
    if lowered.data_size > 0 {
        sections.push(Section {
            name: section_name(b".data"),
            rva: data_rva,
            data: vec![0u8; lowered.data_size as usize], // zero-initialized
            characteristics: DATA_CHARS,
        });
    }
    assemble_pe(text_rva, &sections, &data_dirs)
}

struct RdataLayout {
    bytes: Vec<u8>,
    import_directory_rva: u32,
    import_directory_size: u32,
    address_table_rva: u32,
    address_table_size: u32,
}

// Layout of .rdata: [strings][IDT][ILT][IAT][hint/name entries][dll name].
// The kernel32 imports, in IAT-slot order (must match ImportFunction in x64.rs).
const IMPORT_NAMES: [&[u8]; 3] = [b"GetStdHandle", b"WriteFile", b"ReadFile"];

fn build_rdata_section(rodata: &[u8], rdata_rva: u32) -> RdataLayout {
    let dll_name = b"kernel32.dll";
    let count = IMPORT_NAMES.len();
    let thunk_table_bytes = 8 * (count + 1); // n thunks + a null terminator

    let string_len = rodata.len();
    let mut cursor = align_to_8(string_len);
    let import_directory_offset = cursor;
    cursor += 40; // IMAGE_IMPORT_DESCRIPTOR (kernel32) + null terminator (2 * 20)
    let lookup_table_offset = cursor;
    cursor += thunk_table_bytes;
    let address_table_offset = cursor;
    cursor += thunk_table_bytes;
    let mut name_offsets = Vec::with_capacity(count);
    for name in IMPORT_NAMES {
        name_offsets.push(cursor);
        cursor += align_to_2(2 + name.len() + 1); // u16 hint + name + NUL
    }
    let dll_name_offset = cursor;
    cursor += align_to_2(dll_name.len() + 1);
    let total = cursor;

    let lookup_table_rva = rdata_rva + lookup_table_offset as u32;
    let address_table_rva = rdata_rva + address_table_offset as u32;
    let dll_name_rva = rdata_rva + dll_name_offset as u32;
    let import_directory_rva = rdata_rva + import_directory_offset as u32;

    let mut bytes = vec![0u8; total];
    bytes[..string_len].copy_from_slice(rodata);

    // IDT[0] = kernel32 (then a zero null-descriptor follows automatically)
    write_u32_le(&mut bytes, import_directory_offset, lookup_table_rva); // OriginalFirstThunk
    write_u32_le(&mut bytes, import_directory_offset + 12, dll_name_rva); // Name
    write_u32_le(&mut bytes, import_directory_offset + 16, address_table_rva); // FirstThunk

    // ILT and IAT both hold RVAs of the hint/name entries (import-by-name).
    for (index, &name_offset) in name_offsets.iter().enumerate() {
        let name_rva = (rdata_rva + name_offset as u32) as u64;
        write_u64_le(&mut bytes, lookup_table_offset + 8 * index, name_rva);
        write_u64_le(&mut bytes, address_table_offset + 8 * index, name_rva);
        let name = IMPORT_NAMES[index];
        bytes[name_offset + 2..name_offset + 2 + name.len()].copy_from_slice(name); // hint=0, then name
    }
    bytes[dll_name_offset..dll_name_offset + dll_name.len()].copy_from_slice(dll_name);

    RdataLayout {
        bytes,
        import_directory_rva,
        import_directory_size: 40,
        address_table_rva,
        address_table_size: thunk_table_bytes as u32,
    }
}

fn assemble_pe(entry_rva: u32, sections: &[Section], data_dirs: &[(usize, u32, u32)]) -> Vec<u8> {
    let section_count = sections.len() as u32;
    let headers_unpadded = 0x40 + 4 + 20 + 240 + 40 * section_count;
    let size_of_headers = align_up(headers_unpadded, FILE_ALIGN);

    let mut raw_offsets = Vec::with_capacity(sections.len());
    let mut raw_sizes = Vec::with_capacity(sections.len());
    let mut file_offset = size_of_headers;
    for section in sections {
        let raw_size = align_up(section.data.len() as u32, FILE_ALIGN);
        raw_offsets.push(file_offset);
        raw_sizes.push(raw_size);
        file_offset += raw_size;
    }
    let total_file = file_offset;

    let size_of_image = sections
        .iter()
        .map(|section| section.rva + align_up(section.data.len() as u32, SECT_ALIGN))
        .max()
        .unwrap();
    let size_of_code: u32 = sections
        .iter()
        .zip(&raw_sizes)
        .filter(|(section, _)| section.characteristics & 0x20 != 0)
        .map(|(_, &raw_size)| raw_size)
        .sum();
    let size_of_init: u32 = sections
        .iter()
        .zip(&raw_sizes)
        .filter(|(section, _)| section.characteristics & 0x40 != 0)
        .map(|(_, &raw_size)| raw_size)
        .sum();

    let mut output: Vec<u8> = Vec::new();

    // DOS header
    output.extend_from_slice(b"MZ");
    output.resize(0x3C, 0);
    output.extend_from_slice(&0x40u32.to_le_bytes());

    // PE signature + COFF header
    output.extend_from_slice(b"PE\0\0");
    output.extend_from_slice(&0x8664u16.to_le_bytes());
    output.extend_from_slice(&(section_count as u16).to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp (deterministic)
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&240u16.to_le_bytes());
    output.extend_from_slice(&0x0022u16.to_le_bytes());

    // Optional header (PE32+, 240 bytes)
    let optional_header_start = output.len();
    output.extend_from_slice(&0x20Bu16.to_le_bytes());
    output.push(0);
    output.push(0);
    output.extend_from_slice(&size_of_code.to_le_bytes());
    output.extend_from_slice(&size_of_init.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&entry_rva.to_le_bytes());
    output.extend_from_slice(&entry_rva.to_le_bytes());
    output.extend_from_slice(&IMAGE_BASE.to_le_bytes());
    output.extend_from_slice(&SECT_ALIGN.to_le_bytes());
    output.extend_from_slice(&FILE_ALIGN.to_le_bytes());
    output.extend_from_slice(&6u16.to_le_bytes());
    output.extend_from_slice(&0u16.to_le_bytes());
    output.extend_from_slice(&0u16.to_le_bytes());
    output.extend_from_slice(&0u16.to_le_bytes());
    output.extend_from_slice(&6u16.to_le_bytes());
    output.extend_from_slice(&0u16.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&size_of_image.to_le_bytes());
    output.extend_from_slice(&size_of_headers.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&3u16.to_le_bytes()); // Subsystem = console
    output.extend_from_slice(&0u16.to_le_bytes());
    output.extend_from_slice(&0x100000u64.to_le_bytes());
    output.extend_from_slice(&0x1000u64.to_le_bytes());
    output.extend_from_slice(&0x100000u64.to_le_bytes());
    output.extend_from_slice(&0x1000u64.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&16u32.to_le_bytes());
    let mut dirs = [(0u32, 0u32); 16];
    for &(index, rva, size) in data_dirs {
        dirs[index] = (rva, size);
    }
    for (rva, size) in dirs {
        output.extend_from_slice(&rva.to_le_bytes());
        output.extend_from_slice(&size.to_le_bytes());
    }
    assert_eq!(output.len() - optional_header_start, 240);

    // Section headers
    for (section_index, section) in sections.iter().enumerate() {
        output.extend_from_slice(&section.name);
        output.extend_from_slice(&(section.data.len() as u32).to_le_bytes()); // VirtualSize
        output.extend_from_slice(&section.rva.to_le_bytes());
        output.extend_from_slice(&raw_sizes[section_index].to_le_bytes());
        output.extend_from_slice(&raw_offsets[section_index].to_le_bytes());
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&0u16.to_le_bytes());
        output.extend_from_slice(&0u16.to_le_bytes());
        output.extend_from_slice(&section.characteristics.to_le_bytes());
    }

    output.resize(size_of_headers as usize, 0);
    for (section_index, section) in sections.iter().enumerate() {
        debug_assert_eq!(output.len(), raw_offsets[section_index] as usize);
        output.extend_from_slice(&section.data);
        output.resize((raw_offsets[section_index] + raw_sizes[section_index]) as usize, 0);
    }
    debug_assert_eq!(output.len(), total_file as usize);
    output
}
