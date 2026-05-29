use crate::{ObjectSymbolHandle, SectionPlan, SymbolPlan};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFileLayout {
    pub sections: Arena<SectionPlan>,
    pub symbols: Arena<SymbolPlan>,
    pub entry_symbol: ObjectSymbolHandle,
}

impl ObjectFileLayout {
    pub fn with_capacity(section_capacity: usize, symbol_capacity: usize) -> Self {
        Self {
            sections: Arena::with_capacity(section_capacity),
            symbols: Arena::with_capacity(symbol_capacity),
            entry_symbol: ObjectSymbolHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPlan {
    pub target: NativeTarget,
    pub layout: ObjectFileLayout,
}

impl ObjectPlan {
    pub fn with_capacity(
        target: NativeTarget,
        section_capacity: usize,
        symbol_capacity: usize,
    ) -> Self {
        Self {
            target,
            layout: ObjectFileLayout::with_capacity(section_capacity, symbol_capacity),
        }
    }
}
