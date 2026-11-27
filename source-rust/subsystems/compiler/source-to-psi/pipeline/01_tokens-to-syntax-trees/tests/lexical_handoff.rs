//! What the parser builds from the lexer's tokens.
//!
//! The lexer decides where each token starts and ends, which bytes are
//! comments, and how long a number, a quoted literal or an operator is. The
//! parser sees only those tokens, so each lexical rule owes it one fact: the
//! tree holds one literal, one name or one operator where the rule says one
//! token is, at that token's byte offsets. A lexer that splits `0x1_0`, ends
//! a quoted literal at an escaped quote, or reads `+!` as `+` then `!` still
//! produces tokens; only the tree shows the program changed.
//!
//! `wiki/spec/language/data_and_literals.md`, "Lexical profile": "non-ASCII
//! bytes occur only in comments and literal bodies", and this profile "has
//! no Unicode identifiers or raw-payload literal form". "Integer radix
//! spellings": "Radix and grouping affect source notation, not numeric
//! meaning". "Quoted bytes": "The lexer copies literal-body source bytes
//! except these byte escapes". `wiki/spec/language/expressions.md`: "Lexing
//! takes the longest operator, so `a +!b` is a trapping add".
//!
//! Every test lexes with the real lexer, parses through `source_parser::parse_into`
//! and inspects the resulting `SyntaxTrees` directly.

use crate::frontend_fixture::Lexer;
use crate::parsing_support::{parse, trees};
use numerics::arithmetic::ArithmeticDomain;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DataMember, Item};
use syntax_trees::statement::StatementNode;

/// Declarations, a constant expression with a group, a record binding, a call
/// with static arguments, an index, and dispatches with a destructuring arm, a
/// guard and tuple patterns.
const PROGRAM: &str = r#"data Pair { first: u8; second: u8; }
domain u8::Nonzero requires self > 0;
data Command { case Stop; case Say(text: u8); }
const LIMIT: u32 = (1 + 2) * 3 - 0x10;
const NAME: Label = "caf\xc3\xa9 é";
machine second_of(pair: Pair) -> u8 { let { first as _, second } = pair; second }
machine pick(command: Command, values: [u8; 2]) -> u8 requires values[0] > 0 {
    let chosen: u8 = values[0] + scale::<u8>(1, LIMIT);
    match command {
        Command::Say { text } if text < chosen -> text,
        _ -> second_of(Pair { first: chosen, second: 7 }),
    }
}
machine route(command: Command, limit: u8) -> u8 {
    transition (command, limit) {
        (Command::Say { text }, _) if text < limit -> text,
        (_, _) -> limit,
    }
}"#;

/// Every kind of whitespace and comment the lexer admits. Each starts with
/// whitespace so that a `/` or `*` token before it cannot open a comment.
const SEPARATORS: [&str; 8] = [
    " ",
    "\n",
    "\t",
    "\r\n",
    " /* note */ ",
    " // note\n",
    " /* outer /* inner */ caf\u{e9} */",
    "\n\n  ",
];

/// The kind, spelling, start and end of each token that is not whitespace or a comment.
fn grammar_tokens(source: &str) -> Vec<(tokens::TokenKind, String, usize, usize)> {
    let stream = Lexer::new(source).tokenize().expect("the source lexes");
    stream
        .iter()
        .filter(|token| !token.is_trivia())
        .map(|token| {
            (
                token.kind,
                stream.spelling(token).to_owned(),
                token.span.start,
                token.span.end,
            )
        })
        .collect()
}

fn name_with_offsets(parsed: &SyntaxTrees, name: &Identifier) -> String {
    let span = name.source_span().span;
    format!("{}@{}..{}", name.as_str(parsed), span.start, span.end)
}

/// The value a machine's body ends in, for the root item at `item`.
fn tail_value(parsed: &SyntaxTrees, item: usize) -> ExpressionHandle {
    let Item::Machine(machine) = parsed.root_item(parsed.root_item_handles()[item]) else {
        panic!("expected machine at root item {item}");
    };
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let &last = parsed
        .items
        .statements(state.statements)
        .last()
        .expect("a machine body");
    let StatementNode::Transition(transition) = parsed.statements.statement(last) else {
        panic!("expected tail transition statement");
    };
    parsed
        .statements
        .transition_target(transition.target)
        .expression()
        .expect("a tail expression")
}

/// Compact structural rendering of an expression for readability in assertions.
fn render_expression(parsed: &SyntaxTrees, handle: ExpressionHandle, with_offsets: bool) -> String {
    match parsed.expressions.expression(handle) {
        ExpressionNode::Integer(literal) => format!("int({})", literal),
        ExpressionNode::Float(float) => format!("float({})", float.as_str(parsed)),
        ExpressionNode::String(bytes) => format!("bytes({:?})", bytes.as_ref()),
        ExpressionNode::Name { members, .. } => {
            let parts: Vec<String> = parsed
                .expressions
                .identifier_path_members(*members)
                .iter()
                .map(|member| {
                    if with_offsets {
                        name_with_offsets(parsed, member)
                    } else {
                        member.as_str(parsed).to_owned()
                    }
                })
                .collect();
            parts.join("::")
        }
        ExpressionNode::Unary(unary) => {
            let op = match unary.operator {
                UnaryOperator::LogicalNot => "!",
                UnaryOperator::BitwiseNot => "~",
                UnaryOperator::Dereference => "*",
            };
            format!(
                "({op} {})",
                render_expression(parsed, unary.operand, with_offsets)
            )
        }
        ExpressionNode::Binary(binary) => {
            let op = match (binary.operator, binary.policy) {
                (BinaryOperator::Add, ArithmeticDomain::Exact) => "+",
                (BinaryOperator::Add, ArithmeticDomain::Wrapping) => "+%",
                (BinaryOperator::Add, ArithmeticDomain::Trapping) => "+!",
                (BinaryOperator::And, _) => "&&",
                (BinaryOperator::LessOrEqual, _) => "<=",
                other => panic!("unexpected operator in test: {other:?}"),
            };
            format!(
                "({} {op} {})",
                render_expression(parsed, binary.left, with_offsets),
                render_expression(parsed, binary.right, with_offsets)
            )
        }
        ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            render_expression(parsed, indexed.collection, with_offsets),
            render_expression(parsed, indexed.index, with_offsets)
        ),
        ExpressionNode::Range(range) => format!(
            "{}{}{}",
            render_expression(parsed, range.start, with_offsets),
            if range.end_inclusive { "..=" } else { ".." },
            render_expression(parsed, range.end, with_offsets)
        ),
        other => panic!("unexpected expression in test: {other:?}"),
    }
}

/// The message of a parse error and the source text its span covers.
fn refusal(source: &str) -> (String, usize, usize) {
    let error = parse(source).expect_err("the parser refuses this source");
    let span = error.source_span.span;
    (error.message, span.start, span.end)
}

#[derive(Default)]
struct Moved {
    names: usize,
    spans: usize,
}

fn parse_span_prefix(text: &str) -> Option<((usize, usize), usize)> {
    let rest = text.strip_prefix("Span { start: ")?;
    let comma = rest.find(", end: ")?;
    let start: usize = rest[..comma].parse().ok()?;
    let after_end = &rest[comma + ", end: ".len()..];
    let close = after_end.find(" }")?;
    let end: usize = after_end[..close].parse().ok()?;
    let consumed = "Span { start: ".len() + comma + ", end: ".len() + close + " }".len();
    Some(((start, end), consumed))
}

/// Every table row in `plain` and `spaced` must match identically except for
/// retained source-page handles and byte spans, and every non-empty span must
/// move strictly according to `tokens` while slicing the same spelling.
fn compare_trees(
    plain: &SyntaxTrees,
    spaced: &SyntaxTrees,
    plain_text: &str,
    spaced_text: &str,
    tokens: &[((usize, usize), (usize, usize))],
) -> Moved {
    let left = format!(
        "{:?}",
        (
            &plain.roots,
            &plain.items,
            &plain.expressions,
            &plain.statements,
            &plain.type_references
        )
    );
    let right = format!(
        "{:?}",
        (
            &spaced.roots,
            &spaced.items,
            &spaced.expressions,
            &spaced.statements,
            &spaced.type_references
        )
    );
    let (mut li, mut ri) = (0usize, 0usize);
    let mut moved = Moved::default();
    let mut expecting_source_backed_span = false;

    while li < left.len() && ri < right.len() {
        if left[li..].starts_with("Identifier { source: ")
            && right[ri..].starts_with("Identifier { source: ")
        {
            let l_end = left[li..].find(", source_span:").expect("identifier span");
            let r_end = right[ri..].find(", source_span:").expect("identifier span");
            li += l_end;
            ri += r_end;
            expecting_source_backed_span = true;
            continue;
        }
        if let (Some(((ls, le), l_len)), Some(((rs, re), r_len))) = (
            parse_span_prefix(&left[li..]),
            parse_span_prefix(&right[ri..]),
        ) {
            if (ls, le) != (0, 0) || (rs, re) != (0, 0) {
                assert_ne!(ls, rs, "span start did not move: {ls}..{le}");
                assert_ne!(le, re, "span end did not move: {ls}..{le}");
                assert!(
                    tokens
                        .iter()
                        .any(|(before, after)| before.0 == ls && after.0 == rs),
                    "span start {ls} -> {rs} is not a token start"
                );
                assert!(
                    tokens
                        .iter()
                        .any(|(before, after)| before.1 == le && after.1 == re),
                    "span end {le} -> {re} is not a token end"
                );
                moved.spans += 1;
                if expecting_source_backed_span {
                    assert_eq!(&plain_text[ls..le], &spaced_text[rs..re]);
                    moved.names += 1;
                }
            }
            expecting_source_backed_span = false;
            li += l_len;
            ri += r_len;
            continue;
        }
        assert_eq!(
            left.as_bytes()[li],
            right.as_bytes()[ri],
            "tree diverged near:\n  left:  {}\n  right: {}",
            &left[li.saturating_sub(40)..left.len().min(li + 80)],
            &right[ri.saturating_sub(40)..right.len().min(ri + 80)]
        );
        li += 1;
        ri += 1;
    }
    assert_eq!((li, ri), (left.len(), right.len()));
    moved
}

/// Comments and whitespace are not part of the program: putting some between
/// every two tokens moves every span and changes nothing else in the tree.
#[test]
fn comments_and_whitespace_between_tokens_move_spans_and_change_nothing_else() {
    let plain_tokens = grammar_tokens(PROGRAM);
    let mut spaced = String::new();
    for (index, (_, spelling, _, _)) in plain_tokens.iter().enumerate() {
        spaced.push_str(SEPARATORS[index % SEPARATORS.len()]);
        spaced.push_str(spelling);
    }
    spaced.push_str(" // end");

    // The respelled source holds the same tokens, so only trivia differs.
    let spaced_tokens = grammar_tokens(&spaced);
    assert_eq!(
        plain_tokens
            .iter()
            .map(|(kind, spelling, _, _)| (kind, spelling))
            .collect::<Vec<_>>(),
        spaced_tokens
            .iter()
            .map(|(kind, spelling, _, _)| (kind, spelling))
            .collect::<Vec<_>>()
    );
    let tokens = plain_tokens
        .iter()
        .zip(&spaced_tokens)
        .map(|(plain, spaced)| ((plain.2, plain.3), (spaced.2, spaced.3)))
        .collect::<Vec<_>>();
    assert!(
        tokens
            .iter()
            .all(|(before, after)| before.0 != after.0 && before.1 != after.1)
    );

    let moved = compare_trees(&trees(PROGRAM), &trees(&spaced), PROGRAM, &spaced, &tokens);
    assert!(
        moved.names >= 60,
        "expected moved names, got {}",
        moved.names
    );
    assert!(moved.spans >= moved.names, "{}", moved.spans);
}

/// A character of two, three or four bytes in a comment or a quoted literal
/// moves every later name by that many bytes, and reaches the tree only as
/// the literal's bytes. Counting characters instead would put `next` at 63.
#[test]
fn wider_characters_in_comments_and_literals_leave_later_names_at_their_byte_offsets() {
    let source = "// \u{e9} line\nconst label: Label = /* \u{fc} */ \"na\u{ef}ve\"; \
                  /* \u{1f600}\u{65e5} */ const next: Label = label;";
    let parsed = trees(source);
    let items: Vec<_> = parsed.root_items().collect();
    assert_eq!(items.len(), 2, "comment text declared something");

    let Item::Const(first) = items[0] else {
        panic!("expected first const");
    };
    let Item::Const(second) = items[1] else {
        panic!("expected second const");
    };
    assert_eq!(name_with_offsets(&parsed, &first.name), "label@17..22");
    assert_eq!(
        render_expression(&parsed, first.initializer, true),
        format!("bytes({:?})", "na\u{ef}ve".as_bytes())
    );
    assert_eq!(name_with_offsets(&parsed, &second.name), "next@71..75");
    assert_eq!(
        render_expression(&parsed, second.initializer, true),
        "label@85..90"
    );
    assert_eq!((&source[71..75], &source[85..90]), ("next", "label"));
}

/// `0x1_0`, `0b1_1`, `0o7_7` and `1_000` are one number each: a member's
/// stable number, or one operand of the sum. Radix and `_` change how the
/// number is written, never its value.
#[test]
fn each_radix_spelling_is_one_number_with_the_value_its_radix_gives() {
    let source = "data Packet {\n    #0x1_0 tag: u8;\n    #0b1_1 size: u8;\n}\n\
                  const mask: u32 = 1_000 + 0o7_7;";
    let parsed = trees(source);
    let Item::Data(packet) = parsed.root_item(parsed.root_item_handles()[0]) else {
        panic!("expected data declaration");
    };
    let members: Vec<_> = parsed
        .items
        .data_members(packet.members)
        .iter()
        .map(|member| match member {
            DataMember::Field(field) => (name_with_offsets(&parsed, &field.name), field.identity),
            other => panic!("unexpected member: {other:?}"),
        })
        .collect();
    assert_eq!(
        members,
        [
            ("tag@25..28".to_owned(), Some(16)),
            ("size@45..49".to_owned(), Some(3)),
        ]
    );

    let Item::Const(mask) = parsed.root_item(parsed.root_item_handles()[1]) else {
        panic!("expected const declaration");
    };
    assert_eq!(
        render_expression(&parsed, mask.initializer, true),
        "(int(1000) + int(0o77))"
    );
    let values: Vec<_> = parsed
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Integer(literal) => Some(literal.value_i64()),
            _ => None,
        })
        .collect();
    assert_eq!(values, [Some(1000), Some(63)]);
}

/// A fraction, an exponent with either sign, or both make one float literal.
/// `0..5` has no digit after its first `.`, so the parser receives two
/// integers around `..` and builds a range.
#[test]
fn a_fraction_or_an_exponent_makes_one_float_and_two_dots_leave_a_range() {
    let source = "const f: f64 = 1.5 + 2e-3 + 6.02e+23;\n\
                  machine s(v: @[u8]) -> @[u8] { v[0..5] }";
    let parsed = trees(source);
    let Item::Const(constant) = parsed.root_item(parsed.root_item_handles()[0]) else {
        panic!("expected const");
    };
    assert_eq!(
        render_expression(&parsed, constant.initializer, true),
        "((float(1.5) + float(2e-3)) + float(6.02e+23))"
    );
    assert_eq!(
        render_expression(&parsed, tail_value(&parsed, 1), true),
        "v@69..70[int(0)..int(5)]"
    );
}

/// The parser tells the lexer's float from its integer: `#15` numbers a
/// member, and `#1.5` in the same place is refused at the literal.
#[test]
fn a_float_is_refused_where_a_member_number_is_expected() {
    let parsed = trees("data Packet {\n    #15 tag: u8;\n}");
    let Item::Data(packet) = parsed.root_item(parsed.root_item_handles()[0]) else {
        panic!("expected data");
    };
    let DataMember::Field(field) = &parsed.items.data_members(packet.members)[0] else {
        panic!("expected field");
    };
    assert_eq!(field.identity, Some(15));

    assert_eq!(
        refusal("data Packet {\n    #1.5 tag: u8;\n}"),
        (
            "expected stable identity, found numeric literal `1.5`".to_owned(),
            19,
            22
        )
    );
}

/// `7u8`, `12i64`, `1.5f32` and `2e3f64` are one literal each. The integer
/// lands in the carrier its suffix spells; no suffix reaches the parser as a
/// separate name.
#[test]
fn a_carrier_suffix_stays_inside_its_literal() {
    use numeric_literals::LandedIntegerType::Fixed;
    use numerics::integer_carrier::FixedInteger;

    let source = "const n: u64 = 7u8 + 12i64;\nconst g: f64 = 1.5f32 + 2e3f64;";
    let parsed = trees(source);
    let integers: Vec<_> = parsed
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Integer(literal) => Some((
                literal.value_i64(),
                literal.landing().map(|landing| landing.landed_type),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        integers,
        [
            (Some(7), Some(Fixed(FixedInteger::unsigned(8)))),
            (Some(12), Some(Fixed(FixedInteger::signed(64)))),
        ]
    );

    let Item::Const(float_const) = parsed.root_item(parsed.root_item_handles()[1]) else {
        panic!("expected second const");
    };
    assert_eq!(
        render_expression(&parsed, float_const.initializer, true),
        "(float(1.5f32) + float(2e3f64))"
    );
    assert!(
        !parsed
            .expressions
            .iter_expressions()
            .any(|(_, expression)| matches!(expression, ExpressionNode::Name { .. })),
        "a suffix became a name"
    );
}

/// `wiki/spec/language/numeric_values.md`, "Unbounded computational
/// integers", admits the bare suffixes `7i` and `7u`. The lexer hands each
/// to the parser as one literal, and the parser refuses it today (`TASKS.md`,
/// **INTEGER-CARRIER-FAMILY-CONSTRAINTS**). This pins that verdict at the
/// literal, so the suffix is at least never read as a separate name.
#[test]
fn a_bare_sign_suffix_reaches_the_parser_as_one_literal_and_is_refused_today() {
    for (source, start, end) in [
        ("const n: u64 = 3i;", 15, 17),
        ("const n: u64 = 9u + 1;", 15, 17),
    ] {
        assert_eq!(
            refusal(source),
            ("unknown integer literal suffix".to_owned(), start, end),
            "{source}"
        );
    }
}

/// An escaped quote does not end a quoted literal, so the `;` after it is
/// literal text and the file still holds two declarations. Each escape
/// reaches the tree as the one byte it names.
#[test]
fn an_escaped_quote_does_not_end_a_quoted_literal() {
    let source = r#"const q: Label = "say \"hi\"; done"; const t: Label = "\x41\\\t\0";"#;
    let parsed = trees(source);
    let items: Vec<_> = parsed.root_items().collect();
    assert_eq!(items.len(), 2);

    let Item::Const(first) = items[0] else {
        panic!("expected first const");
    };
    let Item::Const(second) = items[1] else {
        panic!("expected second const");
    };
    assert_eq!(
        render_expression(&parsed, first.initializer, true),
        format!("bytes({:?})", b"say \"hi\"; done".as_slice())
    );
    assert_eq!(name_with_offsets(&parsed, &second.name), "t@43..44");
    assert_eq!(
        render_expression(&parsed, second.initializer, true),
        format!("bytes({:?})", &[0x41u8, 0x5c, 0x09, 0x00][..])
    );
}

/// The language has no raw literal, so a lone `r` is a name like any other,
/// both where it is declared and where it is read.
#[test]
fn a_lone_r_is_an_ordinary_name_on_both_sides_of_a_declaration() {
    let parsed = trees("const r: u8 = r;");
    let Item::Const(constant) = parsed.root_item(parsed.root_item_handles()[0]) else {
        panic!("expected const");
    };
    assert_eq!(name_with_offsets(&parsed, &constant.name), "r@6..7");
    assert_eq!(
        render_expression(&parsed, constant.initializer, true),
        "r@14..15"
    );
}

/// `+!`, `&&`, `::`, `<=` and `+%` each reach the parser as one spelling:
/// `a +!b` is a trapping add of two names, and only the space in `a + !b`
/// makes an add of a negation.
#[test]
fn the_longest_operator_spelling_reaches_the_parser_as_one_operator() {
    let source = "machine p(a: u8, b: u8, e: u8, f: u8) -> bool { a +!b && c::d <= e +% f }\n\
                  machine q(a: u8, b: bool) -> u8 { a + !b }";
    let parsed = trees(source);
    assert_eq!(
        render_expression(&parsed, tail_value(&parsed, 0), false),
        "((a +! b) && (c::d <= (e +% f)))"
    );
    assert_eq!(
        render_expression(&parsed, tail_value(&parsed, 1), false),
        "(a + (! b))"
    );
}

use source_files::LoadedSource;
use source_lexer as lexing;
use source_parser as parsing;

#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
#[path = "support/parsing.rs"]
mod parsing_support;
