use super::ExpressionNode;

#[test]
fn match_arms_preserve_order_source_and_nonnumeric_values() {
    use super::{MatchPattern, TableMatchArm, TableMatchExpression};
    let mut syntax = crate::SyntaxTrees::default();
    let table = &mut syntax.tables.expressions;
    let subject = table.insert(ExpressionNode::Boolean(true));
    let pattern = table.insert(ExpressionNode::Boolean(false));
    let first = table.insert(ExpressionNode::String("first".as_bytes().into()));
    let fallback = table.insert(ExpressionNode::String("fallback".as_bytes().into()));
    let source_span = source::SourceSpan::new(source::SourceId(7), source::Span::new(11, 29));
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
    use super::{ExpressionHandle, ExpressionTable, TableCallExpression};
    use crate::identifier::Identifier;
    let mut table = ExpressionTable::new();
    let argument = table.insert(ExpressionNode::Boolean(true));
    let arguments = table.insert_expression_handles([argument]);
    let call = table.insert(ExpressionNode::Call(TableCallExpression {
        receiver: ExpressionHandle::invalid(),
        target: Identifier::generated("operation"),
        target_is_static: true,
        machine_arguments: Box::default(),
        arguments,
        evidence_arguments: vec![Identifier::generated("evidence")].into_boxed_slice(),
        operational_acknowledgement: Default::default(),
    }));
    let span = source::SourceSpan::new(source::SourceId(7), source::Span::new(11, 29));
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
    // Compare all payload fields after restoring the one intentionally changed field.
    let mut restored = current.clone();
    restored.operational_acknowledgement = retained.operational_acknowledgement;
    assert_eq!(format!("{restored:?}"), format!("{retained:?}"));
    assert_eq!(table.expression_handles(current.arguments), &[argument]);
    assert_eq!(table.source_span(call), span);
    assert_eq!(snapshot.source_span(call), span);
}

#[test]
fn rejected_call_acknowledgement_leaves_shared_page_untouched() {
    let mut table = super::ExpressionTable::new();
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
