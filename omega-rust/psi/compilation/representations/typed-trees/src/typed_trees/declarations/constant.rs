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
    /// The source initializer awaits target-selected meaning.
    pub pending_target_observation: bool,
    /// Completed declaration meaning, separately from runtime materialization.
    pub evaluated_value: Option<language_core::constant_value::EvaluatedConstant>,
    /// Detached authored evidence; absent only when no normalization occurred.
    pub authored_initializer: crate::expression::ExpressionHandle,
    /// Every declaration retains its evaluated evidence root, including
    /// semantic-only values. This is never runtime constant storage.
    pub materialized_initializer: crate::expression::ExpressionHandle,
}
