/// Exact checked certificate for the first fact-call projection rung. The
/// expression handles rejoin the retained typed call/member tree; all nominal
/// coordinates are duplicated here so later review cannot accept a merely
/// shape-compatible projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFactCallProjection {
    pub projection_expression: typed_trees::expression::ExpressionHandle,
    pub call_expression: typed_trees::expression::ExpressionHandle,
    pub target_machine: symbols::SymbolHandle,
    pub target_state: symbols::SymbolHandle,
    pub machine_arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
    pub result_type: typed_trees::types::TypeReferenceHandle,
    pub field: symbols::SymbolHandle,
}

/// Exact checked compiler-intrinsic use joined to its retained expression.
/// The expression handle is custody; the closed intrinsic identity is the
/// semantic result of checking, never a later source-text classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedIntrinsicCallFact {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub intrinsic: language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic,
}
