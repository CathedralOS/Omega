use psi_symbols::SymbolHandle;

/// Retained source identity for an independently nameable const declaration.
/// The substituted value remains absent from typed runtime semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConstDeclaration {
    pub symbol: SymbolHandle,
    pub is_public: bool,
}
