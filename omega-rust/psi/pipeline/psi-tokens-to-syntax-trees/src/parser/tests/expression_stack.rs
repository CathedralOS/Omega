use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

#[test]
fn nested_aggregates_and_grouped_unary_prefixes_use_the_default_stack() {
    let value = (0..64).fold("compute(source)".to_owned(), |value, _| {
        format!("~({value})")
    });
    let mut value = format!("Layer0 {{ value: {value}, audit: compute(other) }}");
    for layer in 1..=8 {
        value = format!("Layer{layer} {{ children: [{value}], audit: compute(other) }}");
    }
    let tokens = Lexer::new(&value)
        .tokenize()
        .expect("tokenize nested values");
    let mut parsed = psi_syntax_trees::SyntaxTrees::new(psi_source::SourceId::default());
    let (_, rest) = crate::parser::expression::parse_expression_handle(
        &mut parsed,
        crate::parser::input::Input::new(psi_source::SourceId::default(), &tokens),
    )
    .expect("parse nested values on the default stack");
    assert!(rest.tokens.is_empty());
    let count = |predicate: fn(&ExpressionNode) -> bool| {
        parsed
            .expressions
            .iter_expressions()
            .filter(|(_, expression)| predicate(expression))
            .count()
    };
    assert_eq!(
        count(|expression| matches!(expression, ExpressionNode::Unary(_))),
        64
    );
    assert_eq!(
        count(|expression| matches!(expression, ExpressionNode::StructLiteral(_))),
        9
    );
    assert_eq!(
        count(|expression| matches!(expression, ExpressionNode::ArrayLiteral(_))),
        8
    );
    assert_eq!(
        count(|expression| matches!(expression, ExpressionNode::Call(_))),
        10
    );
}

fn parse_expression(source: &str) -> (SyntaxTrees, ExpressionHandle) {
    let source_id = psi_source::SourceId::default();
    let tokens = Lexer::new(source).tokenize().expect("tokenize expression");
    let mut parsed = SyntaxTrees::new(source_id);
    let (expression, rest) = crate::parser::expression::parse_expression_handle(
        &mut parsed,
        crate::parser::input::Input::new(source_id, &tokens),
    )
    .expect("parse expression");
    assert!(
        rest.tokens.is_empty(),
        "expression must consume all input: {source}"
    );
    (parsed, expression)
}

#[test]
fn operator_stack_retains_precedence_associativity_and_operator_spans() {
    let source = "a || b && c == d < e | f ^ g & h << i + j * k - l / m % n >> o";
    let (parsed, _) = parse_expression(source);
    let actual: Vec<_> = parsed
        .expressions
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Binary(binary) = expression else {
                return None;
            };
            let span = parsed.expressions.source_span(handle).span;
            Some((binary.operator, &source[span.start..span.end]))
        })
        .collect();
    assert_eq!(
        actual,
        [
            (BinaryOperator::Multiply, "*"),
            (BinaryOperator::Add, "+"),
            (BinaryOperator::Divide, "/"),
            (BinaryOperator::Modulo, "%"),
            (BinaryOperator::Subtract, "-"),
            (BinaryOperator::ShiftLeft, "<<"),
            (BinaryOperator::ShiftRight, ">>"),
            (BinaryOperator::BitwiseAnd, "&"),
            (BinaryOperator::BitwiseXor, "^"),
            (BinaryOperator::BitwiseOr, "|"),
            (BinaryOperator::Less, "<"),
            (BinaryOperator::Equal, "=="),
            (BinaryOperator::And, "&&"),
            (BinaryOperator::Or, "||"),
        ]
    );
    let (parsed, root) = parse_expression("a - b - c");
    let ExpressionNode::Binary(root) = parsed.expressions.expression(root) else {
        panic!("binary root");
    };
    assert!(matches!(
        parsed.expressions.expression(root.left),
        ExpressionNode::Binary(_)
    ));
    assert_eq!(parsed.expressions.display_name(root.right), "c");
}

#[test]
fn membership_remains_a_separate_operand_grammar() {
    let (parsed, _) = parse_expression("a + b in First & Second | Third == c || d");
    let subjects: Vec<_> = parsed
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let ExpressionNode::Membership(membership) = expression else {
                return None;
            };
            Some(membership.value)
        })
        .collect();
    assert_eq!(subjects.len(), 3);
    assert!(subjects.iter().all(|subject| *subject == subjects[0]));
    let operators: Vec<_> = parsed
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let ExpressionNode::Binary(binary) = expression else {
                return None;
            };
            Some(binary.operator)
        })
        .collect();
    assert_eq!(
        operators,
        [
            BinaryOperator::Add,
            BinaryOperator::And,
            BinaryOperator::Or,
            BinaryOperator::Equal,
            BinaryOperator::Or,
        ]
    );
    parse_expression("(a in First) + 1");
    let tokens = Lexer::new("machine test() { let value: u64 = a in First + 1; }")
        .tokenize()
        .expect("tokenize ungrouped membership");
    assert!(crate::parser::parse_syntax_trees(&tokens).is_err());
}

#[test]
fn iterative_prefixes_preserve_borrows_literal_folding_and_call_acknowledgements() {
    let (parsed, root) = parse_expression("&write move -7");
    let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(root) else {
        panic!("borrow");
    };
    assert_eq!(borrow.access, psi_language_core::ReferenceAccess::WriteOnly);
    let ExpressionNode::Integer(literal) = parsed.expressions.expression(borrow.target) else {
        panic!("negated integer literal");
    };
    assert_eq!(
        *literal,
        psi_numerics::literals::IntegerLiteral::from_value(-7)
    );
    let (parsed, root) = parse_expression("suspend block perform()");
    let ExpressionNode::Call(call) = parsed.expressions.expression(root) else {
        panic!("call");
    };
    assert!(call.operational_acknowledgement.acknowledges_suspend);
    assert!(call.operational_acknowledgement.acknowledges_block);
    for (source, expected) in [
        ("suspend suspend perform()", "duplicate `suspend`"),
        ("block block perform()", "duplicate `block`"),
        ("block suspend perform()", "canonical order"),
        ("suspend ~perform()", "immediately before a call"),
        ("block 7", "immediately before a call"),
    ] {
        let source = format!("machine test() {{ let value: u64 = {source}; }}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize invalid prefix");
        let error = crate::parser::parse_syntax_trees(&tokens).expect_err("reject invalid prefix");
        assert!(error.message.contains(expected), "{}", error.message);
    }
    parse_expression("suspend()");
    parse_expression("block()");
    parse_expression(&format!("{}value", "~".repeat(4096)));
}
