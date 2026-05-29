use omega_core::arena::{Arena, Handle};
use omega_object_file::SymbolKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageSymbolTable {
    pub entry_symbol: FinalImageSymbolHandle,
    pub symbols: Arena<FinalImageSymbol>,
    pub imports: Arena<FinalImageImport>,
}

impl FinalImageSymbolTable {
    pub fn with_capacity(
        entry_symbol: FinalImageSymbolHandle,
        symbol_capacity: usize,
        import_capacity: usize,
    ) -> Self {
        Self {
            entry_symbol,
            symbols: Arena::with_capacity(symbol_capacity),
            imports: Arena::with_capacity(import_capacity),
        }
    }
}

impl Default for FinalImageSymbolTable {
    fn default() -> Self {
        Self::with_capacity(Handle::invalid(), 0, 0)
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
