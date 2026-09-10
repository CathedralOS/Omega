use symbols::SymbolHandle;

/// Source declaration identity retained after const value substitution.
///
/// Const values deliberately do not become runtime or typed value nodes. The
/// declaration itself remains independently nameable, however, so visibility
/// and package custody must survive value erasure. Resolution continuations
/// retain the detached initializer in this same arena for future substitution;
/// typing does not carry this link into a runtime constant representation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstDeclaration {
    pub symbol: SymbolHandle,
    pub is_public: bool,
    pub declared_type: crate::types::TypeReference,
    /// Already-resolved declaration-owned root, deep-copied at each later use.
    pub initializer: crate::expression::ExpressionHandle,
    /// Declaration-side source occurrence, independent of each use-site copy.
    pub initializer_source_span: source::SourceSpan,
    /// Canonical structural value encoding for public compatibility. Private
    /// const-v0 declarations retain no review value requirement.
    pub canonical_value_encoding: Option<String>,
}
