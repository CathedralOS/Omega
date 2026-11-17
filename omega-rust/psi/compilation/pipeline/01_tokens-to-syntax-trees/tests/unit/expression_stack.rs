use crate::expressions::parse::parse_expression_handle;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

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
    let mut parsed = syntax_trees::SyntaxTrees::new(source::SourceId::default());
    let retained_source = crate::token_input::cursor::Input::retain_source(&tokens);
    let input = crate::token_input::cursor::Input::new(
        source::SourceId::default(),
        &tokens,
        &retained_source,
        &mut parsed,
    );
    let (_, rest) = parse_expression_handle(&mut parsed, input)
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
    let source_id = source::SourceId::default();
    let tokens = Lexer::new(source).tokenize().expect("tokenize expression");
    let mut parsed = SyntaxTrees::new(source_id);
    let retained_source = crate::token_input::cursor::Input::retain_source(&tokens);
    let input =
        crate::token_input::cursor::Input::new(source_id, &tokens, &retained_source, &mut parsed);
    let (expression, rest) = parse_expression_handle(&mut parsed, input).expect("parse expression");
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
    assert_eq!(parsed.display_expression_name(root.right), "c");
}

/// A suffix chooses the policy of one operation and nothing else: every
/// suffixed form parses as its base operator with that operator's
/// precedence. A suffixed token's span covers its suffix; `>>` reads its
/// suffix from the next token, so its span stays the two `>`.
#[test]
fn policy_suffixes_keep_the_base_operator_and_record_its_policy() {
    use numerics::arithmetic::ArithmeticDomain::{Exact, Saturating, Trapping, Wrapping};
    let source = "a +% b *| c -! d /% e %| f <<! g >>% h >>| i >>! j >> k + l";
    let (parsed, _) = parse_expression(source);
    let actual: Vec<_> = parsed
        .expressions
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Binary(binary) = expression else {
                return None;
            };
            let span = parsed.expressions.source_span(handle).span;
            Some((
                binary.operator,
                binary.policy,
                &source[span.start..span.end],
            ))
        })
        .collect();
    assert_eq!(
        actual,
        [
            (BinaryOperator::Multiply, Saturating, "*|"),
            (BinaryOperator::Add, Wrapping, "+%"),
            (BinaryOperator::Divide, Wrapping, "/%"),
            (BinaryOperator::Modulo, Saturating, "%|"),
            (BinaryOperator::Subtract, Trapping, "-!"),
            (BinaryOperator::ShiftLeft, Trapping, "<<!"),
            (BinaryOperator::ShiftRight, Wrapping, ">>"),
            (BinaryOperator::ShiftRight, Saturating, ">>"),
            (BinaryOperator::ShiftRight, Trapping, ">>"),
            (BinaryOperator::Add, Exact, "+"),
            (BinaryOperator::ShiftRight, Exact, ">>"),
        ]
    );
}

/// Lexing takes the longest operator, so only a space keeps a logical
/// negation apart from a trapping suffix; `>>` reads its suffix from the
/// adjacent token for the same reason.
#[test]
fn a_space_separates_a_negated_operand_from_a_trapping_suffix() {
    use numerics::arithmetic::ArithmeticDomain::{Exact, Trapping};
    for (source, policy, negated) in [
        ("a +!b", Trapping, false),
        ("a + !b", Exact, true),
        ("a >>!b", Trapping, false),
        ("a >> !b", Exact, true),
    ] {
        let (parsed, root) = parse_expression(source);
        let ExpressionNode::Binary(binary) = parsed.expressions.expression(root) else {
            panic!("binary root in `{source}`");
        };
        assert_eq!(binary.policy, policy, "{source}");
        assert_eq!(
            matches!(
                parsed.expressions.expression(binary.right),
                ExpressionNode::Unary(_)
            ),
            negated,
            "{source}"
        );
    }
}

/// A constant is folded before any operand is typed, so a suffixed
/// operator rejects at its own token wherever the initializer nests it.
#[test]
fn a_constant_initializer_rejects_a_nested_policy_suffix() {
    for source in [
        "const WRAPPED: u32 = BASE +% BASE;",
        "const TABLE: [u32; 2] = [1 +% 2, 3];",
        "const CALLED: u32 = scale(BASE *| 2);",
        "const SHIFTED: u32 = (BASE >>! 1);",
        "data Buffer { bytes: [u8; SIZE +% 1]; }",
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize constant");
        let error = crate::parse_syntax_trees(&tokens).expect_err("constant must reject");
        assert!(
            error
                .message
                .contains("is not available in a constant expression yet"),
            "{source}: {}",
            error.message
        );
        let span = error.source_span.span;
        assert!(
            matches!(&source[span.start..span.end], "+%" | "*|" | ">>"),
            "{source}: error points at `{}`",
            &source[span.start..span.end]
        );
    }
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
    assert!(crate::parse_syntax_trees(&tokens).is_err());
}

#[test]
fn iterative_prefixes_preserve_borrows_literal_folding_and_call_acknowledgements() {
    let (parsed, root) = parse_expression("&write -7");
    let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(root) else {
        panic!("borrow");
    };
    assert_eq!(borrow.access, language_core::ReferenceAccess::WriteOnly);
    let ExpressionNode::Integer(literal) = parsed.expressions.expression(borrow.target) else {
        panic!("negated integer literal");
    };
    assert_eq!(*literal, numerics::literals::IntegerLiteral::from_value(-7));
    let (parsed, root) = parse_expression("&write move value");
    let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(root) else {
        panic!("borrow");
    };
    assert_eq!(borrow.access, language_core::ReferenceAccess::WriteOnly);
    assert_eq!(parsed.display_expression_name(borrow.target), "value");
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
        let error = crate::parse_syntax_trees(&tokens).expect_err("reject invalid prefix");
        assert!(error.message.contains(expected), "{}", error.message);
    }
    parse_expression("suspend()");
    parse_expression("block()");
    parse_expression(&format!("{}value", "~".repeat(4096)));
}

#[test]
fn long_operator_chains_keep_bounded_parser_stack_and_exact_prefix_order() {
    let source = format!("{}value", "~".repeat(4096));
    let (parsed, mut expression) = parse_expression(&source);
    for offset in 0..4096 {
        let span = parsed.expressions.source_span(expression).span;
        assert_eq!((span.start, span.end), (offset, offset + 1));
        let ExpressionNode::Unary(unary) = parsed.expressions.expression(expression) else {
            panic!("each prefix retains its operand and source position");
        };
        assert_eq!(unary.operator, UnaryOperator::BitwiseNot);
        expression = unary.operand;
    }
    assert_eq!(parsed.display_expression_name(expression), "value");

    let source = std::iter::repeat_n("a | b ^ c & d << e + f * g", 1024)
        .collect::<Vec<_>>()
        .join(" | ");
    let (parsed, _) = parse_expression(&source);
    assert_eq!(
        parsed
            .expressions
            .iter_expressions()
            .filter(|(_, expression)| { matches!(expression, ExpressionNode::Binary(_)) })
            .count(),
        7 * 1024 - 1,
    );
}

/// Nested groups, calls, indexes, struct literals and matches are parsed
/// by the expression frame loop, so nesting costs heap frames rather than
/// host stack: a thousand levels of each fit a 256 KiB thread stack.
#[test]
fn nested_expressions_parse_on_a_small_thread_stack() {
    const LEVELS: usize = 1000;
    let shapes = [
        format!("{}value{}", "(".repeat(LEVELS), ")".repeat(LEVELS)),
        format!("{}value{}", "compute(".repeat(LEVELS), ")".repeat(LEVELS)),
        format!("{}0{}", "table[".repeat(LEVELS), "]".repeat(LEVELS)),
        format!(
            "{}value{}",
            "Pair { left: ".repeat(LEVELS),
            " }".repeat(LEVELS)
        ),
        format!(
            "{}subject{}",
            "match ".repeat(LEVELS),
            " { _ -> 1 }".repeat(LEVELS)
        ),
        format!(
            "{}0{}",
            "-(table[1 + ".repeat(LEVELS / 2),
            "].field)".repeat(LEVELS / 2)
        ),
    ];
    for (shape, source) in shapes.into_iter().enumerate() {
        let nodes = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                parse_expression(&source)
                    .0
                    .expressions
                    .iter_expressions()
                    .count()
            })
            .expect("spawn small-stack parser thread")
            .join()
            .expect("parse nested expression on a small stack");
        // Parentheses add no node; every other shape adds one per level.
        assert!(
            nodes >= if shape == 0 { 1 } else { LEVELS },
            "shape {shape}"
        );
    }
}
