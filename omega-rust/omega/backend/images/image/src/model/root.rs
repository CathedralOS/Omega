use arena::Handle;
use target::NativeTarget;

use crate::model::{
    FinalExecutableRegion, FinalImageMemory, FinalImageRelocationTable, FinalImageSymbolHandle,
    FinalImageSymbolTable,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImage {
    pub target: NativeTarget,
    pub memory: FinalImageMemory,
    pub symbol_table: FinalImageSymbolTable,
    pub relocation_table: FinalImageRelocationTable,
    pub executable_regions: Vec<FinalExecutableRegion>,
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
    pub fn with_roots(
        target: NativeTarget,
        memory: FinalImageMemory,
        symbol_table: FinalImageSymbolTable,
        relocation_table: FinalImageRelocationTable,
        executable_regions: Vec<FinalExecutableRegion>,
    ) -> Self {
        Self {
            target,
            memory,
            symbol_table,
            relocation_table,
            executable_regions,
        }
    }

    pub fn with_capacity(
        target: NativeTarget,
        memory: FinalImageMemory,
        entry_symbol: FinalImageSymbolHandle,
        symbol_capacity: usize,
        import_capacity: usize,
        relocation_capacity: usize,
    ) -> Self {
        Self::with_roots(
            target,
            memory,
            FinalImageSymbolTable::with_capacity(entry_symbol, symbol_capacity, import_capacity),
            FinalImageRelocationTable::with_capacity(relocation_capacity),
            Vec::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{
        FinalImage, FinalImageMemory, FinalImageRelocationTable, FinalImageSymbolTable,
    };
    use arena::Handle;
    use target::NativeTarget;

    #[test]
    fn final_image_constructor_keeps_artifact_roots_explicit() {
        let target = NativeTarget::linux_arm64();
        let memory = FinalImageMemory {
            text: vec![0xaa],
            data: vec![0xbb],
            bss_size: 3,
            bss_alignment: 4,
        };
        let symbol_table = FinalImageSymbolTable::with_capacity(Handle::invalid(), 1, 2);
        let relocation_table = FinalImageRelocationTable::with_capacity(3);

        let image = FinalImage::with_roots(
            target,
            memory.clone(),
            symbol_table.clone(),
            relocation_table.clone(),
            Vec::new(),
        );

        assert_eq!(image.target, target);
        assert_eq!(image.memory, memory);
        assert_eq!(image.symbol_table, symbol_table);
        assert_eq!(image.relocation_table, relocation_table);
        assert!(image.executable_regions.is_empty());
    }
}
