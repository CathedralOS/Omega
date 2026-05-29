use crate::SectionKind;
use omega_core::arena::Handle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPlan {
    pub name: String,
    pub section: SymbolSection,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
}

pub type ObjectSymbolHandle = Handle<SymbolPlan>;

impl Default for SymbolPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolSection {
    #[default]
    None,
    Section(SectionKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Import,
    Object,
}
