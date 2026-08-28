use psi_symbols::SymbolHandle;

/// Source declaration identity retained after const value substitution.
///
/// Const values deliberately do not become runtime or typed value nodes. The
/// declaration itself remains independently nameable, however, so visibility
/// and package custody must survive value erasure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstDeclaration {
    pub symbol: SymbolHandle,
    pub is_public: bool,
    pub declared_type: crate::types::TypeReference,
    /// Exact authored initializer occurrence retained after value
    /// substitution erases the expression from semantic trees.
    pub initializer_source_span: psi_source::SourceSpan,
    /// Canonical structural value encoding for public compatibility. Private
    /// const-v0 declarations retain no review value requirement.
    pub canonical_value_encoding: Option<String>,
}
