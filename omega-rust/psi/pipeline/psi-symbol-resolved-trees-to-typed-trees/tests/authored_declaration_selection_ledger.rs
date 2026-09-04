use psi_checked_trees::{CheckFacts, CheckedTrees};
use psi_source::{SourceId, SourceSpan, Span};
use psi_symbol_resolved_trees::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, SymbolResolvedTrees,
};
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_owned;
use psi_symbols::SymbolHandle;

#[test]
fn owned_typing_and_checked_wrapping_preserve_authored_selection_occurrences() {
    let mut resolved = SymbolResolvedTrees::default();
    let resolved_id = resolved
        .record_resolved_authored_declaration_selection(
            SourceSpan::new(SourceId(3), Span::new(5, 9)),
            AuthoredDeclarationSelectionExposure::PublicInterface,
            AuthoredDeclarationSelectionKind::TypeReference,
            SymbolHandle::from_arena_index(12),
        )
        .expect("valid resolved target");
    let late_id = resolved
        .record_late_bound_authored_declaration_selection(
            SourceSpan::new(SourceId(3), Span::new(15, 16)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Operator,
            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
        )
        .expect("ledger capacity");
    let expected = resolved.authored_declaration_selections().clone();

    let typed = lower_symbol_resolved_trees_owned(resolved).expect("empty roots type");

    assert_eq!(typed.authored_declaration_selections(), &expected);
    assert_eq!(
        typed
            .authored_declaration_selections()
            .get(resolved_id)
            .map(|selection| selection.occurrence_id()),
        Some(resolved_id)
    );
    assert_eq!(
        typed
            .authored_declaration_selections()
            .get(late_id)
            .map(|selection| selection.occurrence_id()),
        Some(late_id)
    );

    let checked = CheckedTrees::with_roots(typed, CheckFacts::default());

    assert_eq!(checked.typed.authored_declaration_selections(), &expected);
}
