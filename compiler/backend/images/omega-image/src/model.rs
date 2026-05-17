use omega_core::arena::{Arena, Handle};
use omega_object::{RelocationKind, SymbolKind};
use omega_target::NativeTarget;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImage {
    pub target: NativeTarget,
    pub entry_symbol: String,
    pub text: Vec<u8>,
    pub data: Vec<u8>,
    pub bss_size: usize,
    pub bss_alignment: usize,
    pub symbols: Arena<FinalImageSymbol>,
    pub imports: Arena<FinalImageImport>,
    pub relocations: Arena<FinalImageRelocation>,
}

impl Default for FinalImage {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            entry_symbol: String::new(),
            text: Vec::new(),
            data: Vec::new(),
            bss_size: 0,
            bss_alignment: 1,
            symbols: Arena::new(),
            imports: Arena::new(),
            relocations: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageSymbol {
    pub name: String,
    pub section: FinalImageSection,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
}

pub type FinalImageSymbolHandle = Handle<FinalImageSymbol>;

impl Default for FinalImageSymbol {
    fn default() -> Self {
        Self {
            name: String::new(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinalImageSection {
    Text,
    Data,
    Bss,
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinalImageImport {
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageRelocation {
    pub text_offset: usize,
    pub byte_width: usize,
    pub symbol: Arc<str>,
    pub symbol_handle: FinalImageSymbolHandle,
    pub kind: RelocationKind,
}

impl Default for FinalImageRelocation {
    fn default() -> Self {
        Self {
            text_offset: 0,
            byte_width: 0,
            symbol: Arc::from(""),
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinalImageLayout {
    pub text_address: u64,
    pub data_address: u64,
    pub bss_address: u64,
}
