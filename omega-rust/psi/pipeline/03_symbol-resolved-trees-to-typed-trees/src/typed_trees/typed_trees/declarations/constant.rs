use symbols::SymbolHandle;

/// Retained source identity for an independently nameable const declaration.
/// The substituted value remains absent from typed runtime semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstDeclaration {
    pub symbol: SymbolHandle,
    pub is_public: bool,
    pub declared_type: crate::typed_trees::types::TypeReferenceHandle,
    /// Exact authored initializer occurrence retained solely for package
    /// review source custody after const substitution.
    pub initializer_source_span: source::SourceSpan,
    pub canonical_value_encoding: Option<String>,
    /// Detached authored evidence; absent only when no normalization occurred.
    pub authored_initializer: crate::typed_trees::expression::ExpressionHandle,
    /// Every declaration retains its materialized evidence root, including
    /// literal-only declarations. This is never runtime constant storage.
    pub materialized_initializer: crate::typed_trees::expression::ExpressionHandle,
}
