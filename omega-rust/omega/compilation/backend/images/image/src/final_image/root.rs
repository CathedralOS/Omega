use arena::Handle;
use target::NativeTarget;

use crate::final_image::{
    FinalDataRegion, FinalExecutableRegion, FinalImageMemory, FinalImageRelocationTable,
    FinalImageSymbolHandle, FinalImageSymbolTable,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImage {
    pub target: NativeTarget,
    pub memory: FinalImageMemory,
    pub symbol_table: FinalImageSymbolTable,
    pub relocation_table: FinalImageRelocationTable,
    pub executable_regions: Vec<FinalExecutableRegion>,
    /// Classified `.data` rows: the compiler's initialized-data image plus any
    /// writer-installed binding slots and alignment padding.
    pub data_regions: Vec<FinalDataRegion>,
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
            executable_regions: Vec::new(),
            data_regions: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {}
