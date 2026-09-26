use super::lower_expression_handle_from_table;
use arena::HandleSpan;
use source::SourceSpan;
use symbol_resolved_trees as resolved;
use symbols::SymbolHandle;
use typed_trees as typed;

fn authored_selection_occurrences() -> [resolved::AuthoredDeclarationSelectionOccurrenceId; 2] {
    let mut selections = resolved::AuthoredDeclarationSelections::default();
    let first = selections
        .record_resolved(
            SourceSpan::default(),
            resolved::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            resolved::AuthoredDeclarationSelectionKind::StaticPathSegment,
            SymbolHandle::from_arena_index(61),
        )
        .expect("valid selected symbol");
    let second = selections
        .record_late_bound(
            SourceSpan::default(),
            resolved::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            resolved::AuthoredDeclarationSelectionKind::Operator,
            resolved::AuthoredDeclarationSelectionLateBinding::CheckedOperator,
        )
        .expect("ledger capacity");
    [first, second]
}

#[test]
fn lowering_match_preserves_all_arms_nonnumeric_values_and_source_custody() {
    let mut source = resolved::expression::ExpressionTable::new();
    let subject = source.insert(resolved::expression::ExpressionNode::Boolean(true));
    let first = source.insert(resolved::expression::ExpressionNode::Boolean(false));
    let second = source.insert(resolved::expression::ExpressionNode::Boolean(true));
    let span = SourceSpan::new(source::SourceId::default(), source::Span::new(3, 19));
    let arms = source.insert_match_arms([
        resolved::expression::TableMatchArm {
            pattern: resolved::expression::MatchPattern::Value(subject),
            value: first,
            source_span: span,
        },
        resolved::expression::TableMatchArm {
            pattern: resolved::expression::MatchPattern::Wildcard,
            value: second,
            source_span: span,
        },
    ]);
    let expression = source.insert(resolved::expression::ExpressionNode::Match(
        resolved::expression::TableMatchExpression { subject, arms },
    ));
    source.set_source_span(expression, span);
    let mut target = typed::TypedTrees::default();
    let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
        .expect("match lowering");
    let typed::expression::ExpressionNode::Match(dispatch) =
        target.expression_table.expression(lowered)
    else {
        panic!("retained match");
    };
    let arms = target.expression_table.match_arms(dispatch.arms);
    assert_eq!(arms.len(), 2);
    assert!(matches!(
        arms[0].pattern,
        typed::expression::MatchPattern::Value(_)
    ));
    assert!(matches!(
        arms[1].pattern,
        typed::expression::MatchPattern::Wildcard
    ));
    assert!(matches!(
        target.expression_table.expression(arms[0].value),
        typed::expression::ExpressionNode::Boolean(false)
    ));
    assert!(matches!(
        target.expression_table.expression(arms[1].value),
        typed::expression::ExpressionNode::Boolean(true)
    ));
    assert_eq!(arms[0].source_span, span);
    assert_eq!(target.expression_table.source_span(lowered), span);
}

#[test]
fn lowering_copies_expression_occurrence_associations() {
    let occurrences = authored_selection_occurrences();
    let mut source = resolved::expression::ExpressionTable::new();
    let expression = source.insert(resolved::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(9),
    ));
    source.attach_authored_selection_occurrences(expression, occurrences);

    let mut target = typed::TypedTrees::default();
    let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
        .expect("direct lowering should succeed");

    assert_eq!(
        target
            .expression_table
            .authored_selection_occurrences(lowered)
            .collect::<Vec<_>>(),
        occurrences
    );
}

#[test]
fn lowers_binary_expression_directly_into_typed_table() {
    let mut source = resolved::expression::ExpressionTable::new();
    let left = source.insert(resolved::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let right = source.insert(resolved::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(2),
    ));
    let expression = source.insert(resolved::expression::ExpressionNode::Binary(
        resolved::expression::TableBinaryExpression {
            left,
            operator: resolved::expression::BinaryOperator::Add,
            right,
        },
    ));

    let mut target = typed::TypedTrees::default();
    let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
        .expect("direct lowering should succeed");

    assert_eq!(target.expression_table.display_name(lowered), "1 + 2");
    assert_eq!(target.expression_table.expression_count(), 3);
}

#[test]
fn lowers_expression_spans_directly_into_typed_table() {
    let mut source = resolved::expression::ExpressionTable::new();
    let mut values = HandleSpan::empty();
    let one = source.insert(resolved::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let two = source.insert(resolved::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(2),
    ));
    source.push_expression_handle(&mut values, one);
    source.push_expression_handle(&mut values, two);
    let expression = source.insert(resolved::expression::ExpressionNode::ArrayLiteral(values));

    let mut target = typed::TypedTrees::default();
    let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
        .expect("direct lowering should succeed");

    let typed::expression::ExpressionNode::ArrayLiteral(values) =
        target.expression_table.expression(lowered)
    else {
        panic!("root should lower to array literal");
    };

    assert_eq!(values.count(), 2);
    assert_eq!(target.expression_table.display_name(lowered), "[1, 2]");
    assert_eq!(target.expression_table.expression_count(), 3);
}
