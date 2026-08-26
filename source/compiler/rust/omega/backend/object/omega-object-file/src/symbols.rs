use crate::SectionKind;
use omega_function_identity::MachineFunctionIdentity;
use psi_arena::Handle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPlan {
    pub name: String,
    pub section: SymbolSection,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
    /// For `SymbolKind::Import`: the library the binding named (an authored
    /// string-backed bootstrap or a built-in table). Empty means "consult the
    /// per-target import catalog" and is the ZII default for non-imports.
    /// Normalized imports instead retain their atomic coordinates in the
    /// layout's `normalized_imports` side table; this field is never used for
    /// those rows.
    pub import_library: String,
}

pub type ObjectSymbolHandle = Handle<SymbolPlan>;

/// Exact atomic locator attached to one unresolved object import symbol.
///
/// This side table keeps raw target-package bytes out of object-local symbol
/// spellings and preserves the complete normalized value through image
/// construction. It is representation data, not import authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedImportPlan {
    pub symbol: ObjectSymbolHandle,
    pub locator: omega_target::NormalizedForeignLocator,
}

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
