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

fn assert_grouping(
    expression: &str,
    groups_left: bool,
) {
    use NodeKind::{Binary, PathExpression};
    let source = format!("const result = {expression};");
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap_or_else(|error| panic!("{expression}: {error}"));
    // The two binary nodes and three operands in preorder distinguish (a op b) op c from a op (b op c).
    let nodes: Vec<_> = tree
        .get_events()
        .iter()
        .filter_map(|event| match event {
            TreeEvent::Open(kind @ (Binary | PathExpression)) => Some(*kind),
            _ => None,
        })
        .collect();
    let expected = if groups_left {
        [Binary, Binary, PathExpression, PathExpression, PathExpression]
    } else {
        [Binary, PathExpression, Binary, PathExpression, PathExpression]
    };
    assert_eq!(nodes, expected, "{expression}");
}

#[test]
fn every_operator_pair_has_the_specified_grouping() {
    for (first_tier, first_operators) in TIERS.iter().enumerate() {
        for &first in *first_operators {
            for (second_tier, second_operators) in TIERS.iter().enumerate() {
                for &second in *second_operators {
                    let expression = format!("a {first} b {second} c");
                    if is_comparison(first) && is_comparison(second) {
                        let source = format!("const result = {expression};");
                        let tokens = lex(source.as_bytes()).unwrap();
                        let error = parse_file(source.as_bytes(), &tokens).expect_err(&expression);
                        assert_eq!(error.get_kind(), ParseErrorKind::ChainedComparison, "{expression}");
                        let second_start = "const result = a ".len() + first.len() + " b ".len();
                        assert_eq!(error.get_span().get_start_offset(), second_start, "{expression}");
                        assert_eq!(error.get_span().get_end_offset(), second_start + second.len(), "{expression}");
                        assert_grouping(&format!("(a {first} b) {second} c"), true);
                        assert_grouping(&format!("a {first} (b {second} c)"), false);
                    } else {
                        assert_grouping(&expression, first_tier <= second_tier);
                    }
                }
            }
        }
    }
}
