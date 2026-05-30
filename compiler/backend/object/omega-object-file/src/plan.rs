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
    pub fn with_roots(
        sections: Arena<SectionPlan>,
        symbols: Arena<SymbolPlan>,
        entry_symbol: ObjectSymbolHandle,
    ) -> Self {
        Self {
            sections,
            symbols,
            entry_symbol,
        }
    }

    pub fn with_capacity(section_capacity: usize, symbol_capacity: usize) -> Self {
        Self::with_roots(
            Arena::with_capacity(section_capacity),
            Arena::with_capacity(symbol_capacity),
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
}

#[cfg(test)]
mod tests {
    use crate::{ObjectFileLayout, ObjectPlan, SectionPlan, SymbolPlan};
    use omega_core::arena::{Arena, Handle};
    use omega_target::NativeTarget;

    #[test]
    fn layout_constructor_keeps_section_symbol_and_entry_roots_explicit() {
        let sections = Arena::<SectionPlan>::with_capacity(1);
        let symbols = Arena::<SymbolPlan>::with_capacity(2);
        let entry_symbol = Handle::invalid();

        let layout = ObjectFileLayout::with_roots(sections.clone(), symbols.clone(), entry_symbol);

        assert_eq!(layout.sections, sections);
        assert_eq!(layout.symbols, symbols);
        assert_eq!(layout.entry_symbol, entry_symbol);
    }

    #[test]
    fn plan_constructor_keeps_target_and_layout_roots_explicit() {
        let target = NativeTarget::host();
        let layout = ObjectFileLayout::with_capacity(1, 2);

        let object = ObjectPlan::with_layout(target, layout.clone());

        assert_eq!(object.target, target);
        assert_eq!(object.layout, layout);
    }
}
