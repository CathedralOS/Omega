use omega_core::arena::Handle;
use omega_target::NativeTarget;

use crate::model::{
    FinalImageMemory, FinalImageRelocationTable, FinalImageSymbolHandle, FinalImageSymbolTable,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImage {
    pub target: NativeTarget,
    pub memory: FinalImageMemory,
    pub symbol_table: FinalImageSymbolTable,
    pub relocation_table: FinalImageRelocationTable,
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
            symbol_table: FinalImageSymbolTable::with_capacity(
                entry_symbol,
                symbol_capacity,
                import_capacity,
            ),
            relocation_table: FinalImageRelocationTable::with_capacity(relocation_capacity),
        }
    }
}
