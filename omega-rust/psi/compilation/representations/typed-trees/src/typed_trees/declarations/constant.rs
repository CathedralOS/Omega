use symbols::SymbolHandle;

/// Retained source identity for an independently nameable const declaration.
/// The substituted value remains absent from typed runtime semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstDeclaration {
    pub symbol: SymbolHandle,
    pub is_public: bool,
    /// The semantic evaluator normalized this declaration. Its exact authored
    /// and materialized roots must independently replay against the retained
    /// value before the receiving compilation may publish or execute it.
    pub requires_value_replay: bool,
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
    /// Source-owned evaluation entry for a deferred scalar initializer. This
    /// machine is detached from executable roots: its expression still owes
    /// exact scalar formation and invocation checks before it supplies a value.
    /// Target closure installs this entry rather than inventing another owner.
    pub evaluation_entry: arena::Handle<crate::machine::Machine>,
    /// Entry-owned occurrence, distinct from the source root that names the
    /// application. Reading a query in this body must not read the application
    /// whose value this very activation is computing.
    pub evaluation_expression: crate::expression::ExpressionHandle,
}
