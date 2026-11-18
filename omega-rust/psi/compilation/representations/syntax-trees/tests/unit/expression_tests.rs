use crate::expression::ExpressionNode;

#[test]
fn subtree_copy_rejoins_saved_subject_aliases_without_merging_dispatches() {
    use crate::expression::{
        MatchPattern, TableDestructurePattern, TableMatchArm, TableMatchExpression,
        TableSavedSubject,
    };
    let mut source = crate::SyntaxTrees::default();
    let subject = source.expressions.insert(ExpressionNode::Boolean(true));
    let head = source.expressions.insert(ExpressionNode::Boolean(false));
    let capture = source.expressions.insert_match_subject(subject);
    let alias = source
        .expressions
        .insert(ExpressionNode::SavedSubject(TableSavedSubject {
            component: Default::default(),
            capture,
            pattern_head: head,
        }));
    let arms = source.expressions.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Destructure(TableDestructurePattern {
                head,
                ..Default::default()
            }),
            value: alias,
            ..Default::default()
        },
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: alias,
            ..Default::default()
        },
    ]);
    let root = source
        .expressions
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject,
            capture,
            arms,
        }));
    let mut destination = crate::SyntaxTrees::default();
    let first = destination.copy_expression_from(&source, root);
    let second = destination.copy_expression_from(&source, root);
    let mut captures = Vec::new();
    for copied in [first, second] {
        let ExpressionNode::Match(dispatch) = destination.expressions.expression(copied) else {
            panic!("copied dispatch");
        };
        assert_eq!(
            destination
                .expressions
                .match_subject(dispatch.capture)
                .subject,
            dispatch.subject
        );
        let arms = destination.expressions.match_arms(dispatch.arms);
        assert_eq!(
            arms[0].value, arms[1].value,
            "one source occurrence stays one copied occurrence"
        );
        let MatchPattern::Destructure(pattern) = arms[0].pattern else {
            panic!("copied pattern");
        };
        let ExpressionNode::SavedSubject(saved) = destination.expressions.expression(arms[0].value)
        else {
            panic!("copied saved read");
        };
        assert_eq!(saved.capture, dispatch.capture);
        assert_eq!(saved.pattern_head, pattern.head);
        captures.push(dispatch.capture);
    }
    assert_ne!(
        captures[0], captures[1],
        "independent copy invocations define independent captures"
    );
}

#[test]
fn match_arms_preserve_order_source_and_nonnumeric_values() {
    use crate::expression::{MatchPattern, TableMatchArm, TableMatchExpression};
    let mut syntax = crate::SyntaxTrees::default();
    let table = &mut syntax.tables.expressions;
    let subject = table.insert(ExpressionNode::Boolean(true));
    let pattern = table.insert(ExpressionNode::Boolean(false));
    let first = table.insert(ExpressionNode::String("first".as_bytes().into()));
    let fallback = table.insert(ExpressionNode::String("fallback".as_bytes().into()));
    let source_span =
        source_text::SourceSpan::new(source_text::SourceId(7), source_text::Span::new(11, 29));
    let arms = table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Value(pattern),
            value: first,
            source_span,
        },
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: fallback,
            source_span,
        },
    ]);
    let expression = table.insert(ExpressionNode::Match(TableMatchExpression {
        subject,
        arms,
        ..Default::default()
    }));
    table.set_source_span(expression, source_span);
    assert_eq!(
        table.match_arms(arms)[0].pattern,
        MatchPattern::Value(pattern)
    );
    assert_eq!(table.match_arms(arms)[1].pattern, MatchPattern::Wildcard);
    assert_eq!(table.match_arms(arms)[0].source_span, source_span);
    assert_eq!(
        syntax.display_expression_name(expression),
        "match true { false => \"first\", _ => \"fallback\" }"
    );
}

#[test]
fn call_acknowledgement_preserves_shared_forest_and_call_payload() {
    use crate::expression::{ExpressionHandle, ExpressionTable, TableCallExpression};
    use crate::identifier::Identifier;
    let mut table = ExpressionTable::new();
    let argument = table.insert(ExpressionNode::Boolean(true));
    let arguments = table.insert_expression_handles([argument]);
    let name = table.append_identifier_path_member(Identifier::generated("operation"));
    let callee = table.insert(ExpressionNode::Name {
        members: arena::HandleSpan::from_parts(name, 1),
        application: None,
    });
    let call = table.insert(ExpressionNode::Call(TableCallExpression {
        callee,
        receiver: ExpressionHandle::invalid(),
        target: Identifier::generated("operation"),
        target_is_static: true,
        machine_arguments: Box::default(),
        arguments,
        evidence_arguments: vec![Identifier::generated("evidence")].into_boxed_slice(),
        operational_acknowledgement: Default::default(),
    }));
    let span =
        source_text::SourceSpan::new(source_text::SourceId(7), source_text::Span::new(11, 29));
    table.set_source_span(call, span);
    table.share_pages();
    let snapshot = table.clone();
    let revision = table.expressions.revision();
    let acknowledgement = language_core::CallOperationalAcknowledgement {
        acknowledges_suspend: true,
        acknowledges_block: true,
        ..Default::default()
    };
    assert!(table.set_call_acknowledgement(call, acknowledgement));
    assert!(table.expressions.revision() > revision);
    assert_eq!(snapshot.expressions.revision(), revision);
    let ExpressionNode::Call(current) = table.expression(call) else {
        panic!("call lost")
    };
    let ExpressionNode::Call(retained) = snapshot.expression(call) else {
        panic!("snapshot call lost")
    };
    assert_eq!(current.operational_acknowledgement, acknowledgement);
    assert_eq!(retained.operational_acknowledgement, Default::default());
    let mut restored = current.clone();
    restored.operational_acknowledgement = retained.operational_acknowledgement;
    assert_eq!(format!("{restored:?}"), format!("{retained:?}"));
    assert_eq!(table.expression_handles(current.arguments), &[argument]);
    assert_eq!(current.callee, callee);
    assert_eq!(retained.callee, callee);
    assert_eq!(table.source_span(call), span);
    assert_eq!(snapshot.source_span(call), span);
}

#[test]
fn rejected_call_acknowledgement_leaves_shared_page_untouched() {
    let mut table = crate::expression::ExpressionTable::new();
    let expression = table.insert(ExpressionNode::Boolean(true));
    table.share_pages();
    let snapshot = table.clone();
    let revision = table.expressions.revision();
    assert!(!table.set_call_acknowledgement(expression, Default::default()));
    assert_eq!(table.expressions.revision(), revision);
    assert!(std::ptr::eq(
        table.expression(expression),
        snapshot.expression(expression)
    ));
    assert!(matches!(
        table.expression(expression),
        ExpressionNode::Boolean(true)
    ));
}
