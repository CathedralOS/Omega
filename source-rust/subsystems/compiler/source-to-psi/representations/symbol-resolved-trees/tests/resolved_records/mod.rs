#[test]
fn intrinsic_finalization_preserves_occurrence_state_checks() {
    use source_declarations::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
    use source_declarations::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;
    use source_declarations::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
    use source_declarations::declaration_selection::AuthoredDeclarationSelectionLateBinding as Binding;
    use source_declarations::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
    let mut trees = super::SymbolResolvedTrees::default();
    let occurrence = trees
        .record_late_bound_authored_declaration_selection(
            Default::default(),
            Exposure::PrivateImplementation,
            Kind::Operator,
            Binding::CheckedOperator,
        )
        .expect("record operator");
    assert!(
        trees
            .finalize_intrinsic_authored_declaration_selection(
                occurrence,
                Binding::CheckedCall,
                Intrinsic::BuiltinOperator
            )
            .is_err()
    );
    assert_eq!(
        trees
            .authored_declaration_selections()
            .get(occurrence)
            .expect("retained occurrence")
            .target(),
        Target::LateBound(Binding::CheckedOperator)
    );
    trees
        .finalize_intrinsic_authored_declaration_selection(
            occurrence,
            Binding::CheckedOperator,
            Intrinsic::BuiltinOperator,
        )
        .expect("finalize checked builtin");
    assert_eq!(
        trees
            .authored_declaration_selections()
            .get(occurrence)
            .expect("retained occurrence")
            .target(),
        Target::Intrinsic(Intrinsic::BuiltinOperator)
    );
    assert!(
        trees
            .finalize_intrinsic_authored_declaration_selection(
                occurrence,
                Binding::CheckedOperator,
                Intrinsic::BuiltinOperator
            )
            .is_err()
    );
}

use crate::source_resolved::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredSelectionExtensionRebaseError,
    AuthoredSelectionOccurrenceStore, SymbolResolvedTrees, domain, expression,
    name::DiagnosticName,
    statement,
    statement::{Statement, TransitionTarget},
};
use arena::HandleSpan;
use source_text::{SourceId, SourceSpan, Span};
use symbols::SymbolHandle;

#[test]
fn symbol_resolved_trees_owns_authored_declaration_selections() {
    let mut trees = SymbolResolvedTrees::default();
    let occurrence_id = trees
        .record_resolved_authored_declaration_selection(
            SourceSpan::new(SourceId(4), Span::new(8, 13)),
            AuthoredDeclarationSelectionExposure::PublicInterface,
            AuthoredDeclarationSelectionKind::TypeReference,
            SymbolHandle::from_arena_index(9),
        )
        .expect("valid selected symbol");
    let selection = *trees
        .authored_declaration_selections()
        .get(occurrence_id)
        .expect("recorded selection");
    let copied_trees = trees.clone();
    trees.rebuild_tables();

    assert_eq!(
        trees.authored_declaration_selections().get(occurrence_id),
        Some(&selection)
    );
    assert_eq!(
        copied_trees.authored_declaration_selections(),
        trees.authored_declaration_selections()
    );
}

#[test]
fn ranking_views_and_signature_invokes_use_independent_arenas() {
    let mut trees = SymbolResolvedTrees::default();
    let mut ranking_view = HandleSpan::empty();
    let mut signature_invokes = HandleSpan::empty();

    trees
        .declarations_mut()
        .ranking_views
        .append_to_span(&mut ranking_view, DiagnosticName::generated("remaining"));
    trees.declarations_mut().signature_invokes.append_to_span(
        &mut signature_invokes,
        crate::signature::InvocationReference::Name(crate::trait_definition::TraitRequirement { name: DiagnosticName::generated("Console.write"), ..Default::default() }),
    );

    assert_eq!(
        trees.machine_ranking_view(ranking_view)[0].as_str(),
        "remaining"
    );
    assert_eq!(
        trees.signature_invokes(signature_invokes)[0].name().as_str(),
        "Console.write"
    );
}

fn record_selection(
    trees: &mut SymbolResolvedTrees,
    start: usize,
    symbol: u32,
) -> crate::source_resolved::AuthoredDeclarationSelectionOccurrenceId {
    trees
        .record_resolved_authored_declaration_selection(
            SourceSpan::new(SourceId(4), Span::new(start, start + 1)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(symbol),
        )
        .expect("valid selected symbol")
}

fn tree_call(occurrence: AuthoredDeclarationSelectionOccurrenceId) -> statement::Statement {
    statement::Statement::Call(statement::Call {
        receiver_symbol: SymbolHandle::invalid(),
        target_symbol: SymbolHandle::from_arena_index(8),
        target: DiagnosticName::generated("invoke"),
        storage: statement::CallStorage {
            authored_call_selection: Some(occurrence),
            ..Default::default()
        },
    })
}

fn named_target(occurrence: AuthoredDeclarationSelectionOccurrenceId) -> TransitionTarget {
    statement::TransitionTarget::Named(statement::NamedTransitionTarget {
        receiver_symbol: Default::default(),
        head_symbol: SymbolHandle::from_arena_index(8),
        symbol: SymbolHandle::from_arena_index(9),
        storage: statement::NamedTransitionTargetStorage {
            authored_call_selection: Some(occurrence),
            ..Default::default()
        },
    })
}

fn tree_transition(occurrence: AuthoredDeclarationSelectionOccurrenceId) -> Statement {
    statement::Statement::Transition(statement::Transition {
        source_guard: None,
        patterns: Default::default(),
        target: named_target(occurrence),
        continuation: Some(named_target(occurrence)),
        guard: statement::TransitionGuard::Always,
        proof_selectors: Box::default(),
        exit: statement::TransitionExit::Ordinary,
        ends_dispatch: true,
        source_span: SourceSpan::default(),
    })
}

#[test]
fn extension_rebase_shifts_every_extension_owned_occurrence_store() {
    let mut trees = SymbolResolvedTrees::default();
    let base_occurrence = record_selection(&mut trees, 1, 3);
    let base_expression = trees
        .expression_table_mut()
        .insert(expression::ExpressionNode::default());
    trees
        .expression_table_mut()
        .attach_authored_selection_occurrences(base_expression, [base_occurrence]);
    trees
        .declarations_mut()
        .state_statements
        .append(tree_call(base_occurrence));
    trees
        .statement_table_mut()
        .insert(statement::StatementNode::Call(statement::TableCall {
            authored_call_selection: Some(base_occurrence),
            ..Default::default()
        }));
    trees
        .declarations_mut()
        .proof_facts
        .append(domain::ProofFact::Membership(domain::ProofMembershipFact {
            authored_domain_selection: Some(base_occurrence),
            ..Default::default()
        }));
    let frontier = trees.authored_selection_extension_frontier();

    let extension_occurrence = record_selection(&mut trees, 3, 4);
    let extension_expression = trees
        .expression_table_mut()
        .insert(expression::ExpressionNode::default());
    trees
        .expression_table_mut()
        .attach_authored_selection_occurrences(extension_expression, [extension_occurrence]);
    let extension_tree_statement = trees
        .declarations_mut()
        .state_statements
        .append(tree_call(extension_occurrence));
    let extension_tree_transition = trees
        .declarations_mut()
        .state_statements
        .append(tree_transition(extension_occurrence));
    let extension_table_statement =
        trees
            .statement_table_mut()
            .insert(statement::StatementNode::Call(statement::TableCall {
                authored_call_selection: Some(extension_occurrence),
                ..Default::default()
            }));
    let extension_fact =
        trees
            .declarations_mut()
            .proof_facts
            .append(domain::ProofFact::Membership(domain::ProofMembershipFact {
                authored_domain_selection: Some(extension_occurrence),
                ..Default::default()
            }));

    let mut destination = crate::source_resolved::AuthoredDeclarationSelections::default();
    destination
        .record_resolved(
            SourceSpan::new(SourceId(4), Span::new(1, 2)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(3),
        )
        .expect("same retained base row");
    destination
        .record_resolved(
            SourceSpan::new(SourceId(4), Span::new(2, 3)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(7),
        )
        .expect("later-phase-only retained row");

    let rebased = trees
        .rebase_authored_selection_extension(frontier, &destination)
        .expect("exact append frontier");
    let shifted = rebased.authored_declaration_selections().rows()[2].occurrence_id();
    assert_eq!(
        rebased
            .expression_table()
            .authored_selection_occurrences(extension_expression)
            .collect::<Vec<_>>(),
        vec![shifted]
    );
    let statement::Statement::Call(call) = rebased
        .declarations()
        .state_statements
        .get(extension_tree_statement)
    else {
        panic!("extension tree call")
    };
    assert_eq!(call.authored_call_selection, Some(shifted));
    let statement::Statement::Transition(transition) = rebased
        .declarations()
        .state_statements
        .get(extension_tree_transition)
    else {
        panic!("extension tree transition")
    };
    let statement::TransitionTarget::Named(target) = &transition.target else {
        panic!("named target")
    };
    assert_eq!(target.authored_call_selection, Some(shifted));
    let Some(statement::TransitionTarget::Named(continuation)) = &transition.continuation
    else {
        panic!("named continuation")
    };
    assert_eq!(continuation.authored_call_selection, Some(shifted));
    let statement::StatementNode::Call(call) = rebased
        .statement_table()
        .statement(extension_table_statement)
    else {
        panic!("extension table call")
    };
    assert_eq!(call.authored_call_selection, Some(shifted));
    let domain::ProofFact::Membership(fact) =
        rebased.declarations().proof_facts.get(extension_fact)
    else {
        panic!("extension proof membership")
    };
    assert_eq!(fact.authored_domain_selection, Some(shifted));
    assert_eq!(
        rebased.authored_declaration_selections().rows().slice(0..2),
        destination.rows()
    );
}

#[test]
fn extension_rebase_failure_returns_the_original_trees_intact() {
    let mut trees = SymbolResolvedTrees::default();
    record_selection(&mut trees, 1, 3);
    let frontier = trees.authored_selection_extension_frontier();
    let mut destination = trees.authored_declaration_selections().clone();
    let destination_only_occurrence = destination
        .record_resolved(
            SourceSpan::new(SourceId(4), Span::new(3, 4)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(4),
        )
        .expect("destination-only selection");
    trees
        .declarations_mut()
        .state_statements
        .append(tree_call(destination_only_occurrence));
    let expected = trees.clone();

    let (returned, error) = trees
        .rebase_authored_selection_extension(frontier, &destination)
        .expect_err("appended store cannot claim a destination-only occurrence");

    assert_eq!(returned, expected);
    assert_eq!(
        error,
        AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
            AuthoredSelectionOccurrenceStore::TreeStatements
        )
    );
}

#[test]
fn extension_rebase_late_failures_preserve_every_earlier_store() {
    for failed_store in [
        AuthoredSelectionOccurrenceStore::Statements,
        AuthoredSelectionOccurrenceStore::TransitionTargets,
        AuthoredSelectionOccurrenceStore::ProofFacts,
    ] {
        let mut trees = SymbolResolvedTrees::default();
        record_selection(&mut trees, 1, 3);
        let frontier = trees.authored_selection_extension_frontier();
        let mut destination = trees.authored_declaration_selections().clone();
        let extension = record_selection(&mut trees, 3, 4);
        for offset in [5, 7] {
            destination
                .record_resolved(
                    SourceSpan::new(SourceId(4), Span::new(offset, offset + 1)),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::MemberAccess,
                    SymbolHandle::from_arena_index(7),
                )
                .expect("destination-only row");
        }
        let outside_source = destination.rows()[2].occurrence_id();
        let expression = trees
            .expression_table_mut()
            .insert(expression::ExpressionNode::default());
        trees
            .expression_table_mut()
            .attach_authored_selection_occurrences(expression, [extension]);
        trees
            .declarations_mut()
            .state_statements
            .append(tree_call(extension));
        trees
            .statement_table_mut()
            .insert(statement::StatementNode::Call(statement::TableCall {
                authored_call_selection: Some(
                    if failed_store == AuthoredSelectionOccurrenceStore::Statements {
                        outside_source
                    } else {
                        extension
                    },
                ),
                ..Default::default()
            }));
        trees.statement_table_mut().insert_transition_target(
            statement::TransitionTargetNode::Named {
                authored_call_selection: Some(
                    if failed_store == AuthoredSelectionOccurrenceStore::TransitionTargets {
                        outside_source
                    } else {
                        extension
                    },
                ),
                operational_acknowledgement: Default::default(),
                path: Default::default(),
                arguments: Default::default(),
                evidence_arguments: Default::default(),
                source_span: Default::default(),
            },
        );
        trees
            .declarations_mut()
            .proof_facts
            .append(domain::ProofFact::Membership(domain::ProofMembershipFact {
                authored_domain_selection: Some(
                    if failed_store == AuthoredSelectionOccurrenceStore::ProofFacts {
                        outside_source
                    } else {
                        extension
                    },
                ),
                ..Default::default()
            }));
        let expected = trees.clone();
        let (returned, error) = trees
            .rebase_authored_selection_extension(frontier, &destination)
            .expect_err("a late destination-only occurrence must reject");
        assert_eq!(
            error,
            AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(failed_store)
        );
        assert_eq!(returned, expected, "late refusal changed an earlier store");
    }
}

#[test]
fn extension_rebase_preserves_appended_clones_of_base_occurrences() {
    let mut trees = SymbolResolvedTrees::default();
    let base_occurrence = record_selection(&mut trees, 1, 3);
    let frontier = trees.authored_selection_extension_frontier();
    let appended_expression = trees
        .expression_table_mut()
        .insert(expression::ExpressionNode::default());
    trees
        .expression_table_mut()
        .attach_authored_selection_occurrences(appended_expression, [base_occurrence]);
    let appended_clone = trees
        .declarations_mut()
        .state_statements
        .append(tree_call(base_occurrence));
    let destination = trees.authored_declaration_selections().clone();

    let rebased = trees
        .rebase_authored_selection_extension(frontier, &destination)
        .expect("compiler-generated clone retains its authored base identity");
    let statement::Statement::Call(call) =
        rebased.declarations().state_statements.get(appended_clone)
    else {
        panic!("appended call clone")
    };
    assert_eq!(call.authored_call_selection, Some(base_occurrence));
    assert_eq!(
        rebased
            .expression_table()
            .authored_selection_occurrences(appended_expression)
            .collect::<Vec<_>>(),
        vec![base_occurrence]
    );
}
