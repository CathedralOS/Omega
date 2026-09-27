//! Start here. The mutable arena-backed object plan the image builders
//! consume: `ObjectPlan` over its arena layout, with `symbols`, `sections`
//! and `relocations` describing the records it holds and `container`
//! serializing the whole plan as the OMGOBJ container.

pub(crate) mod container;
pub(crate) mod relocations;
pub(crate) mod sections;
pub(crate) mod symbols;

use crate::{
    FunctionSymbolPlan, NormalizedImportPlan, ObjectSymbolHandle, SectionPlan, SymbolPlan,
};
use arena::Arena;
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFileLayout {
    pub sections: Arena<SectionPlan>,
    pub symbols: Arena<SymbolPlan>,
    pub function_symbols: Arena<FunctionSymbolPlan>,
    pub normalized_imports: Vec<NormalizedImportPlan>,
    pub entry_symbol: ObjectSymbolHandle,
}

impl ObjectFileLayout {
    pub fn with_roots(
        sections: Arena<SectionPlan>,
        symbols: Arena<SymbolPlan>,
        function_symbols: Arena<FunctionSymbolPlan>,
        normalized_imports: Vec<NormalizedImportPlan>,
        entry_symbol: ObjectSymbolHandle,
    ) -> Self {
        Self {
            sections,
            symbols,
            function_symbols,
            normalized_imports,
            entry_symbol,
        }
    }

    pub fn with_capacity(section_capacity: usize, symbol_capacity: usize) -> Self {
        Self::with_roots(
            Arena::with_capacity(section_capacity),
            Arena::with_capacity(symbol_capacity),
            Arena::new(),
            Vec::new(),
            ObjectSymbolHandle::invalid(),
        )
    }

    pub fn with_capacities(
        section_capacity: usize,
        symbol_capacity: usize,
        function_symbol_capacity: usize,
    ) -> Self {
        Self::with_roots(
            Arena::with_capacity(section_capacity),
            Arena::with_capacity(symbol_capacity),
            Arena::with_capacity(function_symbol_capacity),
            Vec::new(),
            ObjectSymbolHandle::invalid(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPlan {
    pub target: NativeTarget,
    pub layout: ObjectFileLayout,
}

impl ObjectPlan {
    pub fn with_layout(target: NativeTarget, layout: ObjectFileLayout) -> Self {
        Self { target, layout }
    }

    pub fn with_capacity(
        target: NativeTarget,
        section_capacity: usize,
        symbol_capacity: usize,
    ) -> Self {
        Self::with_layout(
            target,
            ObjectFileLayout::with_capacity(section_capacity, symbol_capacity),
        )
    }

    pub fn with_capacities(
        target: NativeTarget,
        section_capacity: usize,
        symbol_capacity: usize,
        function_symbol_capacity: usize,
    ) -> Self {
        Self::with_layout(
            target,
            ObjectFileLayout::with_capacities(
                section_capacity,
                symbol_capacity,
                function_symbol_capacity,
            ),
        )
    }
}

#[cfg(test)]
mod tests {}
