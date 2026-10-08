use checked_trees::CheckFacts;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;

/// Rederive one exact compiler-owned collection view from the typed call and
/// the checked environments that own it.
pub(crate) fn derive_checked_collection_view_intrinsic(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    let call = program.expression_table.call_node(expression)?;
    validation::expression_owners::checked_collection_view_intrinsic_from_exact_owner(
        program,
        &facts.proof.contract_facts,
        expression,
        call,
        None,
    )
}
