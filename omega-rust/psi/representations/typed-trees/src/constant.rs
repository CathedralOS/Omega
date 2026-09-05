use symbols::SymbolHandle;

/// Retained source identity for an independently nameable const declaration.
/// The substituted value remains absent from typed runtime semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstDeclaration {
    pub symbol: SymbolHandle,
    pub is_public: bool,
    pub declared_type: crate::types::TypeReferenceHandle,
    /// Exact authored initializer occurrence retained solely for package
    /// review source custody after const substitution.
    pub initializer_source_span: source::SourceSpan,
    pub canonical_value_encoding: Option<String>,
}
