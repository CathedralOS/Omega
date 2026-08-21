use crate::SectionKind;
use omega_control_flow::MachineFunctionIdentity;
use psi_arena::Handle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPlan {
    pub name: String,
    pub section: SymbolSection,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
    /// For `SymbolKind::Import`: the library the binding named (an authored
    /// `Binding::DllImport(module, symbol)` leaf or a built-in table).
    /// Empty means "consult the per-target import catalog" -- the historical
    /// lookup path -- and is the ZII default for every non-import symbol.
    pub import_library: String,
}

pub type ObjectSymbolHandle = Handle<SymbolPlan>;

/// Canonical linkage from one lowered function identity to its object symbol.
///
/// This side table keeps compiler-private identity out of the serialized
/// symbol vocabulary while still giving relocation planning an exact target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSymbolPlan {
    pub identity: MachineFunctionIdentity,
    pub symbol: ObjectSymbolHandle,
}

impl Default for FunctionSymbolPlan {
    fn default() -> Self {
        Self {
            identity: MachineFunctionIdentity::default(),
            symbol: ObjectSymbolHandle::invalid(),
        }
    }
}

impl Default for SymbolPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
            import_library: String::new(),
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
