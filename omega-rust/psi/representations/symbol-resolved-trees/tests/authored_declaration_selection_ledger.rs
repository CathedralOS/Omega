use source::{SourceId, SourceSpan, Span};
use symbol_resolved_trees::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionRecordError,
    AuthoredDeclarationSelections, SymbolResolvedTrees,
};
use symbols::SymbolHandle;

#[test]
fn authored_declaration_selection_ledger_runtime_canary() {
    let mut rows = AuthoredDeclarationSelections::default();
    let resolved_id = rows
        .record_resolved(
            SourceSpan::new(SourceId(1), Span::new(2, 8)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(3),
        )
        .expect("valid symbol");
    let late_id = rows
        .record_late_bound(
            SourceSpan::new(SourceId(1), Span::new(10, 11)),
            AuthoredDeclarationSelectionExposure::PublicInterface,
            AuthoredDeclarationSelectionKind::Operator,
            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
        )
        .expect("ledger capacity");

    assert_eq!(resolved_id.ordinal(), 0);
    assert_eq!(late_id.ordinal(), 1);
    assert_eq!(
        rows.get(resolved_id)
            .map(|selection| selection.occurrence_id()),
        Some(resolved_id)
    );
    assert_eq!(
        rows.get(late_id).map(|selection| selection.occurrence_id()),
        Some(late_id)
    );

    let before_rejection = rows.clone();
    assert_eq!(
        rows.record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::TypeReference,
            SymbolHandle::invalid(),
        ),
        Err(AuthoredDeclarationSelectionRecordError::InvalidSelectedSymbol)
    );
    assert_eq!(rows, before_rejection);

    let mut trees = SymbolResolvedTrees::default();
    let tree_id = trees
        .record_resolved_authored_declaration_selection(
            SourceSpan::new(SourceId(1), Span::new(2, 8)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(3),
        )
        .expect("valid symbol");
    trees.rebuild_tables();
    assert_eq!(
        trees
            .authored_declaration_selections()
            .get(tree_id)
            .map(|selection| selection.occurrence_id()),
        Some(tree_id)
    );
}
