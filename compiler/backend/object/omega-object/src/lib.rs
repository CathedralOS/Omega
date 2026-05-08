use omega_core::arena::Arena;
use omega_target::{NativeTarget, ObjectFormat};

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
            kind: RelocationKind::Aarch64Branch26,
        }
    }
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
