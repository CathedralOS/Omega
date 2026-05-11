use omega_core::arena::{Arena, Handle};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::RuntimeStorageRegion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPlan {
    pub target: NativeTarget,
    pub sections: Arena<SectionPlan>,
    pub symbols: Arena<SymbolPlan>,
    pub entry_symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionPlan {
    pub name: String,
    pub kind: SectionKind,
    pub size: usize,
    pub alignment: usize,
}

impl Default for SectionPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: SectionKind::Text,
            size: 0,
            alignment: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Data,
    Bss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPlan {
    pub name: String,
    pub section: Option<String>,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
}

pub type ObjectSymbolHandle = Handle<SymbolPlan>;

impl Default for SymbolPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            section: None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Import,
    Object,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlan {
    pub target: NativeTarget,
    pub records: Arena<RelocationRecord>,
}

impl Default for RelocationPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            records: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecord {
    pub function_symbol: String,
    pub selected_instruction_index: u32,
    pub text_offset: usize,
    pub byte_width: usize,
    pub symbol: String,
    pub symbol_handle: ObjectSymbolHandle,
    pub kind: RelocationKind,
}

impl Default for RelocationRecord {
    fn default() -> Self {
        Self {
            function_symbol: String::new(),
            selected_instruction_index: 0,
            text_offset: 0,
            byte_width: 0,
            symbol: String::new(),
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

pub fn object_symbol_handle_by_name(object: &ObjectPlan, symbol_name: &str) -> ObjectSymbolHandle {
    object
        .symbols
        .iter()
        .find(|(_, symbol)| symbol.name == symbol_name)
        .map(|(handle, _)| handle)
        .unwrap_or_else(Handle::invalid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
    X86_64Absolute64,
    X86_64Relative32,
}

pub fn entry_symbol_name(target: NativeTarget) -> String {
    match target.object_format {
        ObjectFormat::MachO => "_main".to_owned(),
        ObjectFormat::Elf | ObjectFormat::Coff => "main".to_owned(),
    }
}

pub fn section_name(target: NativeTarget, kind: SectionKind) -> String {
    match (target.object_format, kind) {
        (ObjectFormat::MachO, SectionKind::Text) => "__TEXT,__text".to_owned(),
        (ObjectFormat::MachO, SectionKind::Data) => "__DATA,__data".to_owned(),
        (ObjectFormat::MachO, SectionKind::Bss) => "__DATA,__bss".to_owned(),
        (_, SectionKind::Text) => ".text".to_owned(),
        (_, SectionKind::Data) => ".data".to_owned(),
        (_, SectionKind::Bss) => ".bss".to_owned(),
    }
}

pub fn machine_storage_symbol_name(machine_name: &str) -> String {
    format!("omega_machine_{machine_name}_storage")
}

pub fn runtime_frame_storage_symbol_name() -> String {
    "omega_runtime_frame_storage".to_owned()
}

pub fn storage_region_symbol_name(
    region: RuntimeStorageRegion,
    entry_machine_name: &str,
) -> String {
    match region {
        RuntimeStorageRegion::Machine => machine_storage_symbol_name(entry_machine_name),
        RuntimeStorageRegion::RuntimeFrame => runtime_frame_storage_symbol_name(),
    }
}

pub struct ObjectContainerInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectContainerOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
}

pub fn emit_omega_object_container(input: ObjectContainerInput<'_>) -> ObjectContainerOutput {
    let bss_bytes = bss_size(input.object);

    let mut bytes = Vec::new();
    bytes.extend(b"OMGOBJ\0\0");
    write_u32(&mut bytes, 2);
    write_u32(&mut bytes, architecture_id(input.target.architecture));
    write_u32(&mut bytes, object_format_id(input.target.object_format));
    write_u64(
        &mut bytes,
        u64::try_from(input.text_bytes.len()).expect("text size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(input.data_bytes.len()).expect("data size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(bss_bytes).expect("bss size overflow"),
    );

    write_symbols(&mut bytes, input.object);
    write_relocations(&mut bytes, input.relocations);

    bytes.extend(input.text_bytes);
    bytes.extend(input.data_bytes);

    ObjectContainerOutput {
        bytes,
        file_name: "omega-backend.omgobj".to_owned(),
        format: "omega-backend-object-container".to_owned(),
        text_bytes: input.text_bytes.len(),
        data_bytes: input.data_bytes.len(),
        bss_bytes,
        symbols: input.object.symbols.len(),
        relocations: input.relocations.records.len(),
    }
}

fn bss_size(object: &ObjectPlan) -> usize {
    object
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn write_symbols(bytes: &mut Vec<u8>, object: &ObjectPlan) {
    write_u32(
        bytes,
        u32::try_from(object.symbols.len()).expect("symbol count overflow"),
    );

    for (_, symbol) in object.symbols.iter() {
        write_string(bytes, &symbol.name);
        write_string(bytes, symbol.section.as_deref().unwrap_or(""));
        write_u64(
            bytes,
            u64::try_from(symbol.offset).expect("symbol offset overflow"),
        );
        write_u64(
            bytes,
            u64::try_from(symbol.size).expect("symbol size overflow"),
        );
        write_u32(bytes, symbol_kind_id(symbol.kind));
    }
}

fn write_relocations(bytes: &mut Vec<u8>, relocations: &RelocationPlan) {
    write_u32(
        bytes,
        u32::try_from(relocations.records.len()).expect("relocation count overflow"),
    );

    for (_, relocation) in relocations.records.iter() {
        write_string(bytes, &relocation.function_symbol);
        write_u32(bytes, relocation.selected_instruction_index);
        write_u64(
            bytes,
            u64::try_from(relocation.text_offset).expect("relocation text offset overflow"),
        );
        write_u32(
            bytes,
            u32::try_from(relocation.byte_width).expect("relocation byte width overflow"),
        );
        write_string(bytes, &relocation.symbol);
        write_u32(bytes, relocation_kind_id(relocation.kind));
    }
}

fn architecture_id(architecture: Architecture) -> u32 {
    match architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }
}

fn object_format_id(object_format: ObjectFormat) -> u32 {
    match object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }
}

fn symbol_kind_id(symbol_kind: SymbolKind) -> u32 {
    match symbol_kind {
        SymbolKind::Function => 1,
        SymbolKind::Import => 2,
        SymbolKind::Object => 3,
    }
}

fn relocation_kind_id(relocation_kind: RelocationKind) -> u32 {
    match relocation_kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::X86_64Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    write_u32(
        bytes,
        u32::try_from(value.len()).expect("object string length overflow"),
    );
    bytes.extend(value.as_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}
