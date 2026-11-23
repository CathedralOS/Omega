//! Each pair of operator spellings is checked against the precedence table in GRAMMAR.md.

use omega_lexer::lex;
use omega_parser::{NodeKind, ParseErrorKind, TreeEvent, parse_file};

// Tightest first. Kept independent of the parser's punctuation and binding-level tables.
const TIERS: &[&[&str]] = &[
    &[
        "*", "/", "%", "*%", "*|", "*!", "/%", "/|", "/!", "%%", "%|", "%!",
    ],
    &["+", "-", "+%", "+|", "+!", "-%", "-|", "-!"],
    &["<<", "<<%", "<<|", "<<!", ">>", ">>%", ">>|", ">>!"],
    &["&"],
    &["^"],
    &["|"],
    &["in"],
    &["<", "<=", ">", ">="],
    &["==", "!="],
    &["&&"],
    &["||"],
];

fn is_comparison(operator: &str) -> bool {
    matches!(operator, "<" | "<=" | ">" | ">=" | "==" | "!=")
}

#[test]
fn machine_and_trait_heads_bind_the_fixed_declaration_operators() {
    // V1's OperatorSpelling::ALL and the V2 machine production agree on these 13.
    const BINDABLE: &[&str] = &[
        "+", "-", "*", "/", "%", "<", ">", "<=", ">=", "==", "!=", "[]", "[..]",
    ];
    for &operator in BINDABLE.iter().chain(TIERS.iter().flat_map(|tier| tier.iter())) {
        for source in [
            format!("machine {operator} op(a: T, b: T) -> T {{ a }}"),
            format!("trait Ops {{ machine {operator} op(a: T, b: T) -> T; }}"),
        ] {
            let tokens = lex(source.as_bytes()).unwrap();
            let result = parse_file(source.as_bytes(), &tokens);
            if BINDABLE.contains(&operator) {
                let tree = result.unwrap_or_else(|error| panic!("{source}: {error}"));
                assert!(
                    tree.get_events()
                        .contains(&TreeEvent::Open(NodeKind::OperatorToken)),
                    "{source}"
                );
            } else {
                assert!(result.is_err(), "{source}");
            }
        }
    }
}

fn assert_grouping(
    first: &str,
    second: &str,
    groups_left: bool,
    parenthesized: bool,
) {
    let inner = if groups_left { ("a", first, "b") } else { ("b", second, "c") };
    let nested = format!("{} {} {}", inner.0, inner.1, inner.2);
    let nested = if parenthesized { format!("({nested})") } else { nested };
    let outer = if groups_left {
        (nested.as_str(), second, "c")
    } else {
        ("a", first, nested.as_str())
    };
    let source = format!("const result = {} {} {};", outer.0, outer.1, outer.2);
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap_or_else(|error| panic!("{source}: {error}"));
    let index = tree.index(&source, &tokens).unwrap();
    let text = |id| &source[index.span(id).get_start_offset()..index.span(id).get_end_offset()];

    // Read the operator and its direct operands from the product L2 consumes. A
    // preorder of node kinds alone cannot detect a different operator at the same depth.
    let mut actual = Vec::new();
    for (id, node) in index
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == NodeKind::Binary)
    {
        let [lhs, rhs] = node.children[..] else {
            panic!("expected two operands: {source}")
        };
        for child in [lhs, rhs] {
            let kind = match text(child) {
                "a" | "b" | "c" => NodeKind::PathExpression,
                s if s.starts_with('(') => NodeKind::Paren,
                _ => NodeKind::Binary,
            };
            assert_eq!(index.nodes[child].kind, kind, "{source}");
        }
        let op = index.operator(id).unwrap();
        // One space on each side; for repeated spellings this pins which occurrence
        // belongs to this node, including both `>` tokens and a possible suffix.
        assert_eq!(op.span.get_start_offset(), index.span(lhs).get_end_offset() + 1, "{source}");
        assert_eq!(op.span.get_end_offset() + 1, index.span(rhs).get_start_offset(), "{source}");
        actual.push((text(lhs), op.token, text(rhs)));
    }
    assert_eq!(actual, [outer, inner], "{source}");
}

#[test]
fn every_operator_pair_has_the_specified_grouping() {
    spec_corpus::observe_handoff!("product/l1", compare_operator_pairs());
}

fn compare_operator_pairs() {
    for (first_tier, first_operators) in TIERS.iter().enumerate() {
        for &first in *first_operators {
            for (second_tier, second_operators) in TIERS.iter().enumerate() {
                for &second in *second_operators {
                    if is_comparison(first) && is_comparison(second) {
                        let expression = format!("a {first} b {second} c");
                        let source = format!("const result = {expression};");
                        let tokens = lex(source.as_bytes()).unwrap();
                        let error = parse_file(source.as_bytes(), &tokens).expect_err(&expression);
                        assert_eq!(error.get_kind(), ParseErrorKind::ChainedComparison, "{expression}");
                        let second_start = "const result = a ".len() + first.len() + " b ".len();
                        assert_eq!(error.get_span().get_start_offset(), second_start, "{expression}");
                        assert_eq!(error.get_span().get_end_offset(), second_start + second.len(), "{expression}");
                        assert_grouping(first, second, true, true);
                        assert_grouping(first, second, false, true);
                    } else {
                        assert_grouping(first, second, first_tier <= second_tier, false);
                    }
                }
            }
        }
    }
}

#[test]
fn completing_a_tighter_operand_does_not_end_the_comparison_guard() {
    for (first, second) in [("==", "<="), ("<=", "==")] {
        let source = format!("const x = a {first} b - c - d /* {second} */\r\n {second} e;");
        let tokens = lex(source.as_bytes()).unwrap();
        let error = parse_file(source.as_bytes(), &tokens).unwrap_err();
        let start = source.rfind(second).unwrap();
        assert_eq!(error.get_kind(), ParseErrorKind::ChainedComparison, "{source}");
        assert_eq!(error.get_span().get_start_offset(), start, "{source}");
        assert_eq!(error.get_span().get_end_offset(), start + second.len(), "{source}");
    }
}

#[test]
fn a_call_named_if_preserves_its_argument_diagnostic() {
    let source = b"machine m() { if(a < b < c); }";
    let tokens = lex(source).unwrap();
    let error = parse_file(source, &tokens).unwrap_err();
    assert_eq!(error.get_kind(), ParseErrorKind::ChainedComparison);
    assert_eq!(error.get_span().get_start_offset(), source.iter().rposition(|&byte| byte == b'<').unwrap());
}
