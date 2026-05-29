use omega_core::arena::{Arena, Handle};
use omega_object_file::{RelocationKind, SymbolKind};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImage {
    pub target: NativeTarget,
    pub memory: FinalImageMemory,
    pub symbol_table: FinalImageSymbolTable,
    pub relocation_table: FinalImageRelocationTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageMemory {
    pub text: Vec<u8>,
    pub data: Vec<u8>,
    pub bss_size: usize,
    pub bss_alignment: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageSymbolTable {
    pub entry_symbol: FinalImageSymbolHandle,
    pub symbols: Arena<FinalImageSymbol>,
    pub imports: Arena<FinalImageImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageRelocationTable {
    pub relocations: Arena<FinalImageRelocation>,
}

impl Default for FinalImage {
    fn default() -> Self {
        Self::with_capacity(
            NativeTarget::host(),
            FinalImageMemory::default(),
            Handle::invalid(),
            0,
            0,
            0,
        )
    }
}

impl FinalImage {
    pub fn with_capacity(
        target: NativeTarget,
        memory: FinalImageMemory,
        entry_symbol: FinalImageSymbolHandle,
        symbol_capacity: usize,
        import_capacity: usize,
        relocation_capacity: usize,
    ) -> Self {
        Self {
            target,
            memory,
            symbol_table: FinalImageSymbolTable {
                entry_symbol,
                symbols: Arena::with_capacity(symbol_capacity),
                imports: Arena::with_capacity(import_capacity),
            },
            relocation_table: FinalImageRelocationTable {
                relocations: Arena::with_capacity(relocation_capacity),
            },
        }
    }
}

impl Default for FinalImageMemory {
    fn default() -> Self {
        Self {
            text: Vec::new(),
            data: Vec::new(),
            bss_size: 0,
            bss_alignment: 1,
        }
    }
}

impl Default for FinalImageSymbolTable {
    fn default() -> Self {
        Self {
            entry_symbol: Handle::invalid(),
            symbols: Arena::new(),
            imports: Arena::new(),
        }
    }
}

impl Default for FinalImageRelocationTable {
    fn default() -> Self {
        Self {
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
    pub symbol_handle: FinalImageSymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageRelocation {
    pub text_offset: usize,
    pub byte_width: usize,
    pub symbol_handle: FinalImageSymbolHandle,
    pub kind: RelocationKind,
}

impl Default for FinalImageRelocation {
    fn default() -> Self {
        Self {
            text_offset: 0,
            byte_width: 0,
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
