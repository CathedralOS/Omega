use psi_source::{SourceId, SourceSpan, Span};
use psi_symbol_resolved_trees::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelections, SymbolResolvedTrees,
};
use psi_symbols::SymbolHandle;

#[test]
fn authored_declaration_selection_ledger_runtime_canary() {
    let resolved = AuthoredDeclarationSelection::resolved(
        SourceSpan::new(SourceId(1), Span::new(2, 8)),
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
        AuthoredDeclarationSelectionKind::MemberAccess,
        SymbolHandle::from_arena_index(3),
    )
    .expect("valid symbol");
    let late = AuthoredDeclarationSelection::late_bound(
        SourceSpan::new(SourceId(1), Span::new(10, 11)),
        AuthoredDeclarationSelectionExposure::PublicInterface,
        AuthoredDeclarationSelectionKind::Operator,
        AuthoredDeclarationSelectionLateBinding::CheckedOperator,
    );
    let mut rows = AuthoredDeclarationSelections::default();
    let resolved_handle = rows.record(resolved);
    rows.record(late);

    assert_eq!(rows.get(resolved_handle), Some(&resolved));
    assert_eq!(rows.iter().copied().collect::<Vec<_>>(), [resolved, late]);
    assert!(
        AuthoredDeclarationSelection::resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::TypeReference,
            SymbolHandle::invalid(),
        )
        .is_none()
    );

    let mut trees = SymbolResolvedTrees::default();
    let tree_handle = trees.record_authored_declaration_selection(resolved);
    trees.rebuild_tables();
    assert_eq!(
        trees.authored_declaration_selections().get(tree_handle),
        Some(&resolved)
    );
}
