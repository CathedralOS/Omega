/// Exact checked certificate for the first fact-call projection rung. The
/// expression handles rejoin the retained typed call/member tree; all nominal
/// coordinates are duplicated here so later review cannot accept a merely
/// shape-compatible projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFactCallProjection {
    pub projection_expression:
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    pub call_expression:
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    pub target_machine: symbols::SymbolHandle,
    pub target_state: symbols::SymbolHandle,
    pub machine_arguments: Box<
        [symbol_resolved_trees_to_typed_trees::typed_trees::expression::StaticMachineArgument],
    >,
    pub result_type: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    pub field: symbols::SymbolHandle,
}

/// Exact checked compiler-intrinsic use joined to its retained expression.
/// The expression handle is custody; the closed intrinsic identity is the
/// semantic result of checking, never a later source-text classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedIntrinsicCallFact {
    pub expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    pub intrinsic: language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic,
}

/// Selected execution of an already-checked boundary requirement. These exact
/// semantic symbols steer the source interpreter without rewriting source calls.
/// Provider policy and target realization remain outside checked Psi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryAdapterDispatch {
    pub receiver: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
    pub forward_receiver: bool,
    /// The finite-family value tuple this row realizes, keyed by the canonical
    /// const identities a specialization carries for the same arguments (for
    /// example `named(integer-const(16))`), in the requirement's const/value
    /// binder declaration order. Empty on an exact nongeneric row: it matches
    /// every call to its requirement without consulting static arguments.
    pub family_tuple: Box<[String]>,
}
