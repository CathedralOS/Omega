use omega_core::arena::{Arena, Handle};
use omega_object_file::RelocationKind;

use crate::model::FinalImageSymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageRelocationTable {
    pub relocations: Arena<FinalImageRelocation>,
}

impl FinalImageRelocationTable {
    pub fn with_capacity(relocation_capacity: usize) -> Self {
        Self {
            relocations: Arena::with_capacity(relocation_capacity),
        }
    }
}

impl Default for FinalImageRelocationTable {
    fn default() -> Self {
        Self::with_capacity(0)
    }
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
