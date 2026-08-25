use psi_symbols::SymbolHandle;

/// Source declaration identity retained after const value substitution.
///
/// Const values deliberately do not become runtime or typed value nodes. The
/// declaration itself remains independently nameable, however, so visibility
/// and package custody must survive value erasure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConstDeclaration {
    pub symbol: SymbolHandle,
    pub is_public: bool,
}
