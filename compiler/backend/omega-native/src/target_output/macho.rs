use crate::emitter::{EmittedNativeOutput, NativeOutputKind};
use crate::object::{SectionKind, SymbolKind};
use crate::plan::NativePlan;
use crate::relocations::{RelocationKind, RelocationRecord};
use omega_core::diagnostics::Diagnostic;

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
    write_u32(bytes, 0xfeedfacf);
    write_u32(bytes, 0x0100000c);
    write_u32(bytes, 0);
    write_u32(bytes, 1);
    write_u32(bytes, 4);
    write_u32(
        bytes,
        u32::try_from(sizeofcmds).expect("Mach-O load command size overflow"),
    );
    write_u32(bytes, 0);
    write_u32(bytes, 0);
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

    alignment.trailing_zeros() as usize
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
