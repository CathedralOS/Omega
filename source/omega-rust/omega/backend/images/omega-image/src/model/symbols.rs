use omega_object_file::SymbolKind;
use omega_target::NormalizedForeignLocator;
use psi_arena::{Arena, Handle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageSymbolTable {
    pub entry_symbol: FinalImageSymbolHandle,
    pub symbols: Arena<FinalImageSymbol>,
    pub imports: Arena<FinalImageImport>,
}

impl FinalImageSymbolTable {
    pub fn with_roots(
        entry_symbol: FinalImageSymbolHandle,
        symbols: Arena<FinalImageSymbol>,
        imports: Arena<FinalImageImport>,
    ) -> Self {
        Self {
            entry_symbol,
            symbols,
            imports,
        }
    }

    pub fn with_capacity(
        entry_symbol: FinalImageSymbolHandle,
        symbol_capacity: usize,
        import_capacity: usize,
    ) -> Self {
        Self::with_roots(
            entry_symbol,
            Arena::with_capacity(symbol_capacity),
            Arena::with_capacity(import_capacity),
        )
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
    /// Exact object-plan import coordinates. Normalized coordinates stay
    /// atomic across final-image layout and are interpreted only by their
    /// matching object-format emitter.
    pub import: FinalImageImportPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FinalImageImportPlan {
    #[default]
    None,
    StringBackedBootstrap {
        library: String,
    },
    Normalized(NormalizedForeignLocator),
}

#[cfg(test)]
mod tests {
    use crate::model::{FinalImageImport, FinalImageSymbol, FinalImageSymbolTable};
    use psi_arena::{Arena, Handle};

    #[test]
    fn symbol_table_constructor_keeps_symbol_and_import_roots_explicit() {
        let entry_symbol = Handle::invalid();
        let symbols = Arena::<FinalImageSymbol>::with_capacity(1);
        let imports = Arena::<FinalImageImport>::with_capacity(2);

        let table =
            FinalImageSymbolTable::with_roots(entry_symbol, symbols.clone(), imports.clone());

        assert_eq!(table.entry_symbol, entry_symbol);
        assert_eq!(table.symbols, symbols);
        assert_eq!(table.imports, imports);
    }
}
