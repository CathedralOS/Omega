/// Exact checked certificate for the first fact-call projection rung. The
/// expression handles rejoin the retained typed call/member tree; all nominal
/// coordinates are duplicated here so later review cannot accept a merely
/// shape-compatible projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFactCallProjection {
    pub projection_expression: psi_typed_trees::expression::ExpressionHandle,
    pub call_expression: psi_typed_trees::expression::ExpressionHandle,
    pub target_machine: psi_symbols::SymbolHandle,
    pub target_state: psi_symbols::SymbolHandle,
    pub machine_arguments: Box<[psi_typed_trees::expression::StaticMachineArgument]>,
    pub result_type: psi_typed_trees::types::TypeReferenceHandle,
    pub field: psi_symbols::SymbolHandle,
}

/// Exact checked compiler-intrinsic use joined to its retained expression.
/// The expression handle is custody; the closed intrinsic identity is the
/// semantic result of checking, never a later source-text classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedIntrinsicCallFact {
    pub expression: psi_typed_trees::expression::ExpressionHandle,
    pub intrinsic:
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic,
}
