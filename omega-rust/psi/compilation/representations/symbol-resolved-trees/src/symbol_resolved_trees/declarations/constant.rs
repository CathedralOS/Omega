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
    /// Detached original computation for independent semantic fold replay.
    /// Literal-only declarations retain their initializer here too; absence
    /// cannot stand for an erased computation with no operator occurrences.
    pub authored_initializer: crate::expression::ExpressionHandle,
    /// Declaration-side source occurrence, independent of each use-site copy.
    pub initializer_source_span: source::SourceSpan,
    /// The initializer depends on target-selected meaning and cannot be folded
    /// before the selected realization is available.
    pub pending_target_observation: bool,
    /// Completed semantic value. An encoding is available only when its bits
    /// are determined; generic-index eligibility remains a separate judgment.
    pub evaluated_value: Option<language_core::constant_value::EvaluatedConstant>,
}
