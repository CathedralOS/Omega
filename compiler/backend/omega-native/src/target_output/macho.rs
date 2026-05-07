use crate::emitter::{EmittedNativeOutput, NativeOutputKind};
use crate::final_image::{FinalImageLayout, apply_aarch64_relocations, build_final_image};
use crate::object::{SectionKind, SymbolKind};
use crate::plan::NativePlan;
use crate::relocations::{RelocationKind, RelocationRecord};
use omega_core::diagnostics::Diagnostic;
use sha2::{Digest, Sha256};

const MACHO_EXECUTABLE_BASE: u64 = 0x1_0000_0000;
const MACHO_ARM64_PAGE_SIZE: usize = 0x4000;
const MACHO_HEADER_SIZE: usize = 32;
const MACHO_SEGMENT_COMMAND_SIZE: usize = 72;
const MACHO_SECTION_SIZE: usize = 80;
const MACHO_LOAD_DYLINKER_COMMAND_SIZE: usize = 32;
const MACHO_MAIN_COMMAND_SIZE: usize = 24;
const MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE: usize = 32;
const MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE: usize = 56;
const MACHO_SYMTAB_COMMAND_SIZE: usize = 24;
const MACHO_DYSYMTAB_COMMAND_SIZE: usize = 80;
const MACHO_CODE_SIGNATURE_COMMAND_SIZE: usize = 16;
const MACHO_HEADER_FLAGS_NOUNDEFS: u32 = 0x1;
const MACHO_HEADER_FLAGS_DYLDLINK: u32 = 0x4;
const MACHO_HEADER_FLAGS_TWOLEVEL: u32 = 0x80;
const MACHO_HEADER_FLAGS_PIE: u32 = 0x20_0000;
const CODE_SIGNATURE_PAGE_SIZE: usize = 4096;
const CODE_SIGNATURE_PAGE_SIZE_POWER: u8 = 12;

pub fn emit_macho_arm64_object(
    native_plan: &NativePlan,
) -> Result<EmittedNativeOutput, Diagnostic> {
    let text_bytes = native_plan.machine_code.bytes.storage_slice();
    let data_bytes = native_plan.data.bytes.storage_slice();
    let bss_bytes = bss_size(native_plan);
    let relocations = macho_text_relocations(native_plan)?;
    let symbols = macho_symbols(native_plan);
    let string_table = macho_string_table(&symbols);
    let section_count = 1 + usize::from(!data_bytes.is_empty()) + usize::from(bss_bytes > 0);
    let segment_command_size = 72 + section_count * 80;
    let sizeofcmds = segment_command_size + 24 + 24 + 80;
    let first_section_offset = 32 + sizeofcmds;
    let text_offset = first_section_offset;
    let data_offset = text_offset + text_bytes.len();
    let relocation_offset = data_offset + data_bytes.len();
    let symbol_offset = relocation_offset + relocations.len() * 8;
    let string_offset = symbol_offset + symbols.len() * 16;

    let mut bytes = Vec::new();
    write_macho_header(&mut bytes, sizeofcmds);
    write_macho_segment_command(
        &mut bytes,
        section_count,
        text_bytes.len() + data_bytes.len() + bss_bytes,
        text_offset,
        text_bytes.len() + data_bytes.len(),
    );
    write_macho_text_section(
        &mut bytes,
        text_bytes.len(),
        text_offset,
        relocation_offset,
        relocations.len(),
    );
    if !data_bytes.is_empty() {
        write_macho_data_section(&mut bytes, text_bytes.len(), data_bytes.len(), data_offset);
    }
    if bss_bytes > 0 {
        write_macho_bss_section(
            &mut bytes,
            text_bytes.len() + data_bytes.len(),
            bss_bytes,
            native_plan,
        );
    }
    write_macho_build_version_command(&mut bytes);
    write_macho_symtab_command(
        &mut bytes,
        symbol_offset,
        symbols.len(),
        string_offset,
        string_table.bytes.len(),
    );
    write_macho_dysymtab_command(&mut bytes, &symbols);

    bytes.extend(text_bytes);
    bytes.extend(data_bytes);
    for relocation in &relocations {
        write_macho_relocation(&mut bytes, relocation);
    }
    for symbol in &symbols {
        write_macho_nlist(&mut bytes, symbol, &string_table);
    }
    bytes.extend(string_table.bytes);

    Ok(EmittedNativeOutput {
        bytes,
        file_name: "omega-native.o".to_owned(),
        format: "mach-o-arm64-object".to_owned(),
        kind: NativeOutputKind::LinkableObject,
        text_bytes: text_bytes.len(),
        data_bytes: data_bytes.len(),
        bss_bytes,
        symbols: symbols.len(),
        relocations: relocations.len(),
        final_image_symbols: 0,
        final_image_imports: 0,
        final_image_relocations: 0,
    })
}

pub fn emit_macho_arm64_executable(
    native_plan: &NativePlan,
) -> Result<EmittedNativeOutput, Diagnostic> {
    let mut image = build_final_image(native_plan);
    if !image.imports.is_empty() {
        return Err(Diagnostic::error(
            "Mach-O direct executable cannot import dynamic symbols yet",
        ));
    }

    let data_section_count = usize::from(!image.data.is_empty()) + usize::from(image.bss_size > 0);
    let has_data_segment = data_section_count > 0;
    let command_count = 10 + usize::from(has_data_segment);
    let sizeofcmds = MACHO_SEGMENT_COMMAND_SIZE
        + (MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
        + usize::from(has_data_segment)
            * (MACHO_SEGMENT_COMMAND_SIZE + data_section_count * MACHO_SECTION_SIZE)
        + MACHO_LOAD_DYLINKER_COMMAND_SIZE
        + MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE
        + MACHO_MAIN_COMMAND_SIZE
        + MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE
        + MACHO_SYMTAB_COMMAND_SIZE
        + MACHO_DYSYMTAB_COMMAND_SIZE
        + MACHO_SEGMENT_COMMAND_SIZE
        + MACHO_CODE_SIGNATURE_COMMAND_SIZE;
    let text_offset = align_to(MACHO_HEADER_SIZE + sizeofcmds, 16);
    let data_offset = align_to(text_offset + image.text.len(), MACHO_ARM64_PAGE_SIZE);
    let text_address = MACHO_EXECUTABLE_BASE + text_offset as u64;
    let data_address = MACHO_EXECUTABLE_BASE + data_offset as u64;
    let bss_address = align_to_u64(
        data_address + image.data.len() as u64,
        image.bss_alignment as u64,
    );
    let layout = FinalImageLayout {
        text_address,
        data_address,
        bss_address,
    };

    apply_aarch64_relocations(&mut image, &layout, "Mach-O direct executable")?;

    let text_file_size = if has_data_segment {
        data_offset
    } else {
        align_to(text_offset + image.text.len(), MACHO_ARM64_PAGE_SIZE)
    };
    let data_memory_size = if has_data_segment {
        (bss_address - data_address)
            .checked_add(image.bss_size as u64)
            .expect("Mach-O data memory size overflow")
    } else {
        0
    };
    let data_vm_size = align_to_u64(data_memory_size, MACHO_ARM64_PAGE_SIZE as u64);

    let unsigned_file_end = if has_data_segment {
        data_offset + image.data.len()
    } else {
        text_offset + image.text.len()
    };
    let code_signature_offset = align_to(unsigned_file_end, MACHO_ARM64_PAGE_SIZE);
    let code_signature_size = code_signature_size(code_signature_offset);
    let linkedit_vmaddr = MACHO_EXECUTABLE_BASE + code_signature_offset as u64;
    let linkedit_vmsize = align_to(code_signature_size, MACHO_ARM64_PAGE_SIZE);

    let mut bytes = Vec::new();
    write_macho_executable_header(&mut bytes, command_count, sizeofcmds);
    write_macho_pagezero_segment(&mut bytes);
    write_macho_executable_text_segment(&mut bytes, text_offset, image.text.len(), text_file_size);
    if has_data_segment {
        write_macho_executable_data_segment(
            &mut bytes,
            data_offset,
            image.data.len(),
            image.bss_size,
            data_vm_size,
            image.bss_alignment,
        );
    }
    write_macho_load_dylinker_command(&mut bytes);
    write_macho_executable_build_version_command(&mut bytes);
    write_macho_main_command(&mut bytes, text_offset);
    write_macho_load_libsystem_command(&mut bytes);
    write_macho_linkedit_segment(
        &mut bytes,
        linkedit_vmaddr,
        code_signature_offset,
        code_signature_size,
        linkedit_vmsize,
    );
    write_empty_macho_symtab_command(&mut bytes);
    write_empty_macho_dysymtab_command(&mut bytes);
    write_macho_code_signature_command(&mut bytes, code_signature_offset, code_signature_size);
    bytes.resize(text_offset, 0);
    bytes.extend(&image.text);
    if has_data_segment {
        bytes.resize(data_offset, 0);
        bytes.extend(&image.data);
    }
    bytes.resize(code_signature_offset, 0);
    let code_signature = macho_ad_hoc_code_signature(&bytes);
    debug_assert_eq!(code_signature.len(), code_signature_size);
    bytes.extend(code_signature);

    Ok(EmittedNativeOutput {
        bytes,
        file_name: "omega-program".to_owned(),
        format: "mach-o-arm64-executable".to_owned(),
        kind: NativeOutputKind::DirectExecutable,
        text_bytes: image.text.len(),
        data_bytes: image.data.len(),
        bss_bytes: image.bss_size,
        symbols: image.symbols.len(),
        relocations: image.relocations.len(),
        final_image_symbols: image.symbols.len(),
        final_image_imports: image.imports.len(),
        final_image_relocations: image.relocations.len(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachOSymbol {
    name: String,
    section_ordinal: u8,
    value: u64,
    kind: MachOSymbolKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MachOSymbolKind {
    LocalSection,
    ExternalSection,
    UndefinedExternal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachOStringTable {
    bytes: Vec<u8>,
    entries: Vec<MachOStringEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachOStringEntry {
    name: String,
    offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachORelocation {
    address: u32,
    symbol_index: u32,
    pcrel: bool,
    length: u8,
    kind: u8,
}

fn macho_symbols(native_plan: &NativePlan) -> Vec<MachOSymbol> {
    let mut symbols = Vec::new();
    let data_section_ordinal = if native_plan.data.bytes.is_empty() {
        0
    } else {
        2
    };
    let bss_section_ordinal = if native_plan.data.bytes.is_empty() {
        2
    } else {
        3
    };

    for (_, symbol) in native_plan.object.symbols.iter() {
        if symbol.kind == SymbolKind::Object
            && symbol.size > 0
            && symbol.section.as_deref() == Some("__DATA,__data")
        {
            symbols.push(MachOSymbol {
                name: symbol.name.clone(),
                section_ordinal: data_section_ordinal,
                value: u64::try_from(native_plan.machine_code.byte_count + symbol.offset)
                    .expect("Mach-O data symbol value overflow"),
                kind: MachOSymbolKind::LocalSection,
            });
        }
    }

    for (_, symbol) in native_plan.object.symbols.iter() {
        if symbol.kind == SymbolKind::Object
            && symbol.size > 0
            && symbol.section.as_deref() == Some("__DATA,__bss")
        {
            symbols.push(MachOSymbol {
                name: symbol.name.clone(),
                section_ordinal: bss_section_ordinal,
                value: u64::try_from(
                    native_plan.machine_code.byte_count
                        + native_plan.data.bytes.len()
                        + symbol.offset,
                )
                .expect("Mach-O bss symbol value overflow"),
                kind: MachOSymbolKind::LocalSection,
            });
        }
    }

    for (_, symbol) in native_plan.object.symbols.iter() {
        if symbol.kind == SymbolKind::Function {
            symbols.push(MachOSymbol {
                name: symbol.name.clone(),
                section_ordinal: 1,
                value: u64::try_from(symbol.offset).expect("Mach-O function symbol value overflow"),
                kind: MachOSymbolKind::ExternalSection,
            });
        }
    }

    for (_, symbol) in native_plan.object.symbols.iter() {
        if symbol.kind == SymbolKind::Import {
            symbols.push(MachOSymbol {
                name: symbol.name.clone(),
                section_ordinal: 0,
                value: 0,
                kind: MachOSymbolKind::UndefinedExternal,
            });
        }
    }

    symbols
}

fn macho_string_table(symbols: &[MachOSymbol]) -> MachOStringTable {
    let mut bytes = vec![0];
    let mut entries = Vec::new();

    for symbol in symbols {
        let offset = u32::try_from(bytes.len()).expect("Mach-O string table offset overflow");
        bytes.extend(symbol.name.as_bytes());
        bytes.push(0);
        entries.push(MachOStringEntry {
            name: symbol.name.clone(),
            offset,
        });
    }

    MachOStringTable { bytes, entries }
}

fn macho_text_relocations(native_plan: &NativePlan) -> Result<Vec<MachORelocation>, Diagnostic> {
    let symbols = macho_symbols(native_plan);
    let mut relocations = native_plan
        .relocations
        .records
        .iter()
        .map(|(_, relocation)| macho_text_relocation(relocation, &symbols))
        .collect::<Result<Vec<_>, _>>()?;

    relocations.sort_by(|left, right| right.address.cmp(&left.address));

    Ok(relocations)
}

fn macho_text_relocation(
    relocation: &RelocationRecord,
    symbols: &[MachOSymbol],
) -> Result<MachORelocation, Diagnostic> {
    let Some(symbol_index) = symbols
        .iter()
        .position(|symbol| symbol.name == relocation.symbol)
    else {
        return Err(Diagnostic::error(format!(
            "cannot emit Mach-O relocation for unknown symbol `{}`",
            relocation.symbol
        )));
    };

    let (pcrel, kind) = match relocation.kind {
        RelocationKind::Aarch64Page21 => (true, 3),
        RelocationKind::Aarch64PageOffset12 => (false, 4),
        RelocationKind::Aarch64Branch26 => (true, 2),
        RelocationKind::X86_64Absolute64 | RelocationKind::X86_64Relative32 => {
            return Err(Diagnostic::error(format!(
                "cannot emit {:?} relocation into Mach-O arm64 object",
                relocation.kind
            )));
        }
    };

    Ok(MachORelocation {
        address: u32::try_from(relocation.text_offset).expect("Mach-O relocation address overflow"),
        symbol_index: u32::try_from(symbol_index).expect("Mach-O symbol index overflow"),
        pcrel,
        length: 2,
        kind,
    })
}

fn write_macho_header(bytes: &mut Vec<u8>, sizeofcmds: usize) {
    write_macho_header_for(bytes, 1, 4, sizeofcmds, 0);
}

fn write_macho_executable_header(bytes: &mut Vec<u8>, command_count: usize, sizeofcmds: usize) {
    write_macho_header_for(
        bytes,
        2,
        command_count,
        sizeofcmds,
        MACHO_HEADER_FLAGS_NOUNDEFS
            | MACHO_HEADER_FLAGS_DYLDLINK
            | MACHO_HEADER_FLAGS_TWOLEVEL
            | MACHO_HEADER_FLAGS_PIE,
    );
}

fn write_macho_header_for(
    bytes: &mut Vec<u8>,
    file_type: u32,
    command_count: usize,
    sizeofcmds: usize,
    flags: u32,
) {
    write_u32(bytes, 0xfeedfacf);
    write_u32(bytes, 0x0100000c);
    write_u32(bytes, 0);
    write_u32(bytes, file_type);
    write_u32(
        bytes,
        u32::try_from(command_count).expect("Mach-O command count overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(sizeofcmds).expect("Mach-O load command size overflow"),
    );
    write_u32(bytes, flags);
    write_u32(bytes, 0);
}

fn write_macho_pagezero_segment(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x19);
    write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
    write_fixed_string_16(bytes, "__PAGEZERO");
    write_u64(bytes, 0);
    write_u64(bytes, MACHO_EXECUTABLE_BASE);
    write_u64(bytes, 0);
    write_u64(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_executable_text_segment(
    bytes: &mut Vec<u8>,
    text_offset: usize,
    text_size: usize,
    text_file_size: usize,
) {
    write_u32(bytes, 0x19);
    write_u32(
        bytes,
        u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
            .expect("Mach-O text segment command size overflow"),
    );
    write_fixed_string_16(bytes, "__TEXT");
    write_u64(bytes, MACHO_EXECUTABLE_BASE);
    write_u64(
        bytes,
        align_to_u64(text_file_size as u64, MACHO_ARM64_PAGE_SIZE as u64),
    );
    write_u64(bytes, 0);
    write_u64(
        bytes,
        u64::try_from(text_file_size).expect("Mach-O text file size overflow"),
    );
    write_u32(bytes, 5);
    write_u32(bytes, 5);
    write_u32(bytes, 1);
    write_u32(bytes, 0);

    write_fixed_string_16(bytes, "__text");
    write_fixed_string_16(bytes, "__TEXT");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + text_offset as u64);
    write_u64(
        bytes,
        u64::try_from(text_size).expect("Mach-O text size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(text_offset).expect("Mach-O text offset overflow"),
    );
    write_u32(bytes, 2);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0x80000400);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_executable_data_segment(
    bytes: &mut Vec<u8>,
    data_offset: usize,
    data_size: usize,
    bss_size: usize,
    data_vm_size: u64,
    bss_alignment: usize,
) {
    let section_count = usize::from(data_size > 0) + usize::from(bss_size > 0);
    write_u32(bytes, 0x19);
    write_u32(
        bytes,
        u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + section_count * MACHO_SECTION_SIZE)
            .expect("Mach-O data segment command size overflow"),
    );
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
    write_u64(bytes, data_vm_size);
    write_u64(
        bytes,
        u64::try_from(data_offset).expect("Mach-O data file offset overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(data_size).expect("Mach-O data file size overflow"),
    );
    write_u32(bytes, 3);
    write_u32(bytes, 3);
    write_u32(
        bytes,
        u32::try_from(section_count).expect("Mach-O data section count overflow"),
    );
    write_u32(bytes, 0);

    if data_size > 0 {
        write_macho_executable_data_section(bytes, data_offset, data_size);
    }
    if bss_size > 0 {
        write_macho_executable_bss_section(bytes, data_offset + data_size, bss_size, bss_alignment);
    }
}

fn write_macho_executable_data_section(bytes: &mut Vec<u8>, data_offset: usize, data_size: usize) {
    write_fixed_string_16(bytes, "__data");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
    write_u64(
        bytes,
        u64::try_from(data_size).expect("Mach-O data section size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(data_offset).expect("Mach-O data section offset overflow"),
    );
    write_u32(bytes, 3);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_executable_bss_section(
    bytes: &mut Vec<u8>,
    bss_address_offset: usize,
    bss_size: usize,
    bss_alignment: usize,
) {
    write_fixed_string_16(bytes, "__bss");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + bss_address_offset as u64);
    write_u64(
        bytes,
        u64::try_from(bss_size).expect("Mach-O bss section size overflow"),
    );
    write_u32(bytes, 0);
    write_u32(bytes, alignment_power(bss_alignment));
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 1);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_load_dylinker_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xe);
    write_u32(bytes, MACHO_LOAD_DYLINKER_COMMAND_SIZE as u32);
    write_u32(bytes, 12);
    bytes.extend(b"/usr/lib/dyld\0");
    bytes.resize(start + MACHO_LOAD_DYLINKER_COMMAND_SIZE, 0);
}

fn write_macho_main_command(bytes: &mut Vec<u8>, entry_offset: usize) {
    write_u32(bytes, 0x80000028);
    write_u32(bytes, MACHO_MAIN_COMMAND_SIZE as u32);
    write_u64(
        bytes,
        u64::try_from(entry_offset).expect("Mach-O entry offset overflow"),
    );
    write_u64(bytes, 0);
}

fn write_macho_executable_build_version_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x32);
    write_u32(bytes, MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE as u32);
    write_u32(bytes, 1);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 1);
    write_u32(bytes, 3);
    write_u32(bytes, 0);
}

fn write_macho_load_libsystem_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xc);
    write_u32(bytes, MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE as u32);
    write_u32(bytes, 24);
    write_u32(bytes, 2);
    write_u32(bytes, 1351 << 16);
    write_u32(bytes, 1 << 16);
    bytes.extend(b"/usr/lib/libSystem.B.dylib\0");
    bytes.resize(start + MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE, 0);
}

fn write_macho_linkedit_segment(
    bytes: &mut Vec<u8>,
    vmaddr: u64,
    file_offset: usize,
    file_size: usize,
    vm_size: usize,
) {
    write_u32(bytes, 0x19);
    write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
    write_fixed_string_16(bytes, "__LINKEDIT");
    write_u64(bytes, vmaddr);
    write_u64(
        bytes,
        u64::try_from(vm_size).expect("Mach-O LINKEDIT vm size overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_offset).expect("Mach-O LINKEDIT file offset overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_size).expect("Mach-O LINKEDIT file size overflow"),
    );
    write_u32(bytes, 1);
    write_u32(bytes, 1);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_code_signature_command(
    bytes: &mut Vec<u8>,
    code_signature_offset: usize,
    code_signature_size: usize,
) {
    write_u32(bytes, 0x1d);
    write_u32(bytes, MACHO_CODE_SIGNATURE_COMMAND_SIZE as u32);
    write_u32(
        bytes,
        u32::try_from(code_signature_offset).expect("Mach-O code signature offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(code_signature_size).expect("Mach-O code signature size overflow"),
    );
}

fn write_empty_macho_symtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x2);
    write_u32(bytes, MACHO_SYMTAB_COMMAND_SIZE as u32);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_empty_macho_dysymtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0xb);
    write_u32(bytes, MACHO_DYSYMTAB_COMMAND_SIZE as u32);
    for _ in 0..18 {
        write_u32(bytes, 0);
    }
}

fn write_macho_segment_command(
    bytes: &mut Vec<u8>,
    section_count: usize,
    virtual_size: usize,
    file_offset: usize,
    file_size: usize,
) {
    write_u32(bytes, 0x19);
    write_u32(
        bytes,
        u32::try_from(72 + section_count * 80).expect("Mach-O segment command size overflow"),
    );
    write_fixed_string_16(bytes, "");
    write_u64(bytes, 0);
    write_u64(
        bytes,
        u64::try_from(virtual_size).expect("Mach-O virtual size overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_offset).expect("Mach-O file offset overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_size).expect("Mach-O file size overflow"),
    );
    write_u32(bytes, 7);
    write_u32(bytes, 7);
    write_u32(
        bytes,
        u32::try_from(section_count).expect("Mach-O section count overflow"),
    );
    write_u32(bytes, 0);
}

fn write_macho_text_section(
    bytes: &mut Vec<u8>,
    text_size: usize,
    text_offset: usize,
    relocation_offset: usize,
    relocation_count: usize,
) {
    write_fixed_string_16(bytes, "__text");
    write_fixed_string_16(bytes, "__TEXT");
    write_u64(bytes, 0);
    write_u64(
        bytes,
        u64::try_from(text_size).expect("Mach-O text size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(text_offset).expect("Mach-O text offset overflow"),
    );
    write_u32(bytes, 2);
    write_u32(
        bytes,
        u32::try_from(relocation_offset).expect("Mach-O relocation offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(relocation_count).expect("Mach-O relocation count overflow"),
    );
    write_u32(bytes, 0x80000400);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_data_section(
    bytes: &mut Vec<u8>,
    data_address: usize,
    data_size: usize,
    data_offset: usize,
) {
    write_fixed_string_16(bytes, "__data");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(
        bytes,
        u64::try_from(data_address).expect("Mach-O data address overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(data_size).expect("Mach-O data size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(data_offset).expect("Mach-O data offset overflow"),
    );
    write_u32(bytes, 3);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_bss_section(
    bytes: &mut Vec<u8>,
    bss_address: usize,
    bss_size: usize,
    native_plan: &NativePlan,
) {
    write_fixed_string_16(bytes, "__bss");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(
        bytes,
        u64::try_from(bss_address).expect("Mach-O bss address overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(bss_size).expect("Mach-O bss size overflow"),
    );
    write_u32(bytes, 0);
    write_u32(
        bytes,
        u32::try_from(bss_alignment_power(native_plan)).expect("Mach-O bss alignment overflow"),
    );
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 1);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_build_version_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x32);
    write_u32(bytes, 24);
    write_u32(bytes, 1);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_symtab_command(
    bytes: &mut Vec<u8>,
    symbol_offset: usize,
    symbol_count: usize,
    string_offset: usize,
    string_size: usize,
) {
    write_u32(bytes, 0x2);
    write_u32(bytes, 24);
    write_u32(
        bytes,
        u32::try_from(symbol_offset).expect("Mach-O symbol offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(symbol_count).expect("Mach-O symbol count overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(string_offset).expect("Mach-O string offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(string_size).expect("Mach-O string size overflow"),
    );
}

fn write_macho_dysymtab_command(bytes: &mut Vec<u8>, symbols: &[MachOSymbol]) {
    let local_count = symbols
        .iter()
        .take_while(|symbol| symbol.kind == MachOSymbolKind::LocalSection)
        .count();
    let external_defined_count = symbols
        .iter()
        .skip(local_count)
        .take_while(|symbol| symbol.kind == MachOSymbolKind::ExternalSection)
        .count();
    let undefined_count = symbols
        .iter()
        .skip(local_count + external_defined_count)
        .filter(|symbol| symbol.kind == MachOSymbolKind::UndefinedExternal)
        .count();

    write_u32(bytes, 0xb);
    write_u32(bytes, 80);
    write_u32(bytes, 0);
    write_u32(
        bytes,
        u32::try_from(local_count).expect("Mach-O local symbol count overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(local_count).expect("Mach-O external defined index overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(external_defined_count).expect("Mach-O external defined count overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(local_count + external_defined_count)
            .expect("Mach-O undefined symbol index overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(undefined_count).expect("Mach-O undefined symbol count overflow"),
    );

    for _ in 0..12 {
        write_u32(bytes, 0);
    }
}

fn write_macho_relocation(bytes: &mut Vec<u8>, relocation: &MachORelocation) {
    write_u32(bytes, relocation.address);
    let packed = relocation.symbol_index
        | (u32::from(relocation.pcrel) << 24)
        | (u32::from(relocation.length) << 25)
        | (1 << 27)
        | (u32::from(relocation.kind) << 28);
    write_u32(bytes, packed);
}

fn write_macho_nlist(bytes: &mut Vec<u8>, symbol: &MachOSymbol, string_table: &MachOStringTable) {
    write_u32(bytes, macho_string_offset(string_table, &symbol.name));
    match symbol.kind {
        MachOSymbolKind::LocalSection => {
            bytes.push(0x0e);
            bytes.push(symbol.section_ordinal);
            write_u16(bytes, 0);
        }
        MachOSymbolKind::ExternalSection => {
            bytes.push(0x0f);
            bytes.push(symbol.section_ordinal);
            write_u16(bytes, 0);
        }
        MachOSymbolKind::UndefinedExternal => {
            bytes.push(0x01);
            bytes.push(0);
            write_u16(bytes, 0);
        }
    }
    write_u64(bytes, symbol.value);
}

fn macho_string_offset(string_table: &MachOStringTable, name: &str) -> u32 {
    string_table
        .entries
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.offset)
        .unwrap_or(0)
}

fn bss_size(native_plan: &NativePlan) -> usize {
    native_plan
        .object
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn bss_alignment_power(native_plan: &NativePlan) -> usize {
    let alignment = native_plan
        .object
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(_, section)| section.alignment.max(1))
        .unwrap_or(1);

    alignment_power(alignment) as usize
}

fn alignment_power(alignment: usize) -> u32 {
    alignment.max(1).trailing_zeros()
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn align_to_u64(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn code_signature_size(code_limit: usize) -> usize {
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let code_directory_length =
        align_to(code_directory_header_size + identifier.len() + 1, 4) + page_count * 32;
    let super_blob_length = 20 + code_directory_length;
    align_to(super_blob_length, 16)
}

fn macho_ad_hoc_code_signature(code_bytes: &[u8]) -> Vec<u8> {
    let code_limit = code_bytes.len();
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let identifier_offset = code_directory_header_size;
    let hash_offset = align_to(identifier_offset + identifier.len() + 1, 4);
    let code_directory_length = hash_offset + page_count * 32;
    let super_blob_length = 20 + code_directory_length;

    let mut bytes = Vec::with_capacity(align_to(super_blob_length, 16));
    write_be_u32(&mut bytes, 0xfade0cc0);
    write_be_u32(
        &mut bytes,
        u32::try_from(super_blob_length).expect("code signature size overflow"),
    );
    write_be_u32(&mut bytes, 1);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 20);

    write_be_u32(&mut bytes, 0xfade0c02);
    write_be_u32(
        &mut bytes,
        u32::try_from(code_directory_length).expect("CodeDirectory size overflow"),
    );
    write_be_u32(&mut bytes, 0x20400);
    write_be_u32(&mut bytes, 0x2);
    write_be_u32(
        &mut bytes,
        u32::try_from(hash_offset).expect("CodeDirectory hash offset overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(identifier_offset).expect("CodeDirectory identifier offset overflow"),
    );
    write_be_u32(&mut bytes, 0);
    write_be_u32(
        &mut bytes,
        u32::try_from(page_count).expect("CodeDirectory page count overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(code_limit).expect("CodeDirectory code limit overflow"),
    );
    bytes.push(32);
    bytes.push(2);
    bytes.push(0);
    bytes.push(CODE_SIGNATURE_PAGE_SIZE_POWER);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u64(
        &mut bytes,
        u64::try_from(code_limit).expect("CodeDirectory code limit overflow"),
    );
    write_be_u64(&mut bytes, MACHO_EXECUTABLE_BASE);
    write_be_u64(&mut bytes, 0);
    write_be_u64(&mut bytes, 0);

    debug_assert_eq!(bytes.len(), 20 + identifier_offset);
    bytes.extend(identifier.as_bytes());
    bytes.push(0);
    bytes.resize(20 + hash_offset, 0);

    for page_index in 0..page_count {
        let start = page_index * CODE_SIGNATURE_PAGE_SIZE;
        let end = (start + CODE_SIGNATURE_PAGE_SIZE).min(code_limit);
        let digest = Sha256::digest(&code_bytes[start..end]);
        bytes.extend(digest);
    }

    bytes.resize(align_to(super_blob_length, 16), 0);
    bytes
}

fn code_slot_count(code_limit: usize) -> usize {
    code_limit.div_ceil(CODE_SIGNATURE_PAGE_SIZE)
}

fn code_signature_identifier() -> &'static str {
    "omega-program"
}

fn write_be_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_be_bytes());
}

fn write_be_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_be_bytes());
}

fn write_fixed_string_16(bytes: &mut Vec<u8>, value: &str) {
    let value_bytes = value.as_bytes();
    assert!(
        value_bytes.len() <= 16,
        "fixed Mach-O string is longer than 16 bytes"
    );
    bytes.extend(value_bytes);
    bytes.resize(bytes.len() + (16 - value_bytes.len()), 0);
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}
