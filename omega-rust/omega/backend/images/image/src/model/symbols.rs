use crate::model::FinalImage;
use arena::{Arena, Handle};
use object_file::SymbolKind;
use sha2::{Digest, Sha256};
use target::NormalizedForeignLocator;

/// Domain-separated commitment to the exact object-owned symbol table copied
/// into one final image. Object-local handles remain compact coordinates; this
/// digest binds each coordinate to its complete symbol row and the selected
/// entry handle before native-artifact replay or publication can rely on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FinalImageSymbolDigest([u8; 32]);

impl FinalImageSymbolDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub fn final_image_symbol_digest(image: &FinalImage) -> FinalImageSymbolDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.final-image-symbol-table.sha256.v1\0");
    digest.update([match image.target.architecture {
        target::Architecture::Aarch64 => 1,
        target::Architecture::X86_64 => 2,
    }]);
    digest.update([match image.target.object_format {
        target::ObjectFormat::Elf => 1,
        target::ObjectFormat::MachO => 2,
        target::ObjectFormat::Coff => 3,
    }]);
    digest.update((image.target.pointer_size as u64).to_le_bytes());
    digest.update((image.target.pointer_alignment as u64).to_le_bytes());
    digest_handle(&mut digest, image.symbol_table.entry_symbol);
    digest.update((image.symbol_table.symbols.len() as u64).to_le_bytes());
    for (handle, symbol) in image.symbol_table.symbols.iter() {
        digest_handle(&mut digest, handle);
        digest.update((symbol.name.len() as u64).to_le_bytes());
        digest.update(symbol.name.as_bytes());
        digest.update([match symbol.section {
            FinalImageSection::Text => 1,
            FinalImageSection::Data => 2,
            FinalImageSection::Bss => 3,
            FinalImageSection::None => 0,
        }]);
        digest.update((symbol.offset as u64).to_le_bytes());
        digest.update((symbol.size as u64).to_le_bytes());
        digest.update([match symbol.kind {
            SymbolKind::Function => 1,
            SymbolKind::Import => 2,
            SymbolKind::Object => 3,
        }]);
    }
    FinalImageSymbolDigest(digest.finalize().into())
}

fn digest_handle(digest: &mut Sha256, handle: FinalImageSymbolHandle) {
    digest.update([u8::from(handle.is_valid())]);
    digest.update(u64::from(handle.arena_index()).to_le_bytes());
    digest.update(u64::from(handle.generation()).to_le_bytes());
}

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
    use crate::model::{
        FinalImage, FinalImageImport, FinalImageMemory, FinalImageSymbol, FinalImageSymbolTable,
        final_image_symbol_digest,
    };
    use arena::{Arena, Handle};
    use object_file::SymbolKind;
    use target::NativeTarget;

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

    #[test]
    fn strong_symbol_digest_rejects_same_handle_entry_and_data_substitution() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::linux_x64(),
            FinalImageMemory::default(),
            Handle::invalid(),
            2,
            0,
            0,
        );
        let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "main".into(),
            section: super::FinalImageSection::Text,
            offset: 0,
            size: 8,
            kind: SymbolKind::Function,
        });
        let data = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "state".into(),
            section: super::FinalImageSection::Data,
            offset: 0,
            size: 8,
            kind: SymbolKind::Object,
        });
        image.symbol_table.entry_symbol = entry;
        let expected = final_image_symbol_digest(&image);

        let mut entry_substitution = image.clone();
        entry_substitution.symbol_table.symbols.get_mut(entry).name = "other_entry".into();
        assert_eq!(entry_substitution.symbol_table.entry_symbol, entry);
        assert_ne!(final_image_symbol_digest(&entry_substitution), expected);

        let mut data_substitution = image;
        data_substitution.symbol_table.symbols.get_mut(data).offset = 8;
        assert_eq!(
            data_substitution.symbol_table.symbols.get(data).kind,
            SymbolKind::Object
        );
        assert_ne!(final_image_symbol_digest(&data_substitution), expected);
    }
}
