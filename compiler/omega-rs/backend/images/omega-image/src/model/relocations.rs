use omega_object_file::RelocationKind;
use psi_arena::{Arena, Handle};

use crate::model::{FinalImageSection, FinalImageSymbolHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageRelocationTable {
    pub relocations: Arena<FinalImageRelocation>,
}

impl FinalImageRelocationTable {
    pub fn with_roots(relocations: Arena<FinalImageRelocation>) -> Self {
        Self { relocations }
    }

    pub fn with_capacity(relocation_capacity: usize) -> Self {
        Self::with_roots(Arena::with_capacity(relocation_capacity))
    }
}

impl Default for FinalImageRelocationTable {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageRelocation {
    pub section: FinalImageSection,
    pub offset: usize,
    pub byte_width: usize,
    pub symbol_handle: FinalImageSymbolHandle,
    pub addend: i64,
    pub kind: RelocationKind,
}

impl Default for FinalImageRelocation {
    fn default() -> Self {
        Self {
            section: FinalImageSection::Text,
            offset: 0,
            byte_width: 0,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{FinalImageRelocation, FinalImageRelocationTable};
    use psi_arena::Arena;

    #[test]
    fn relocation_table_constructor_keeps_relocation_root_explicit() {
        let relocations = Arena::<FinalImageRelocation>::with_capacity(3);

        let table = FinalImageRelocationTable::with_roots(relocations.clone());

        assert_eq!(table.relocations, relocations);
    }
}
