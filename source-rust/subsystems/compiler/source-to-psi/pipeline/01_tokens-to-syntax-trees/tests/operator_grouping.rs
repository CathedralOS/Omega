//! How the parser groups two binary operators, for every pair of them.
//!
//! The parser owes the next stage one row per binary operation:
//! `TableBinaryExpression { left, operator, right, policy }` in the
//! expression table, with the operator's own span beside it. Parentheses add
//! no row, so which operation is the operand of which is the only record of
//! how `a + b * c` groups. A wrong tier or a right-associative tier still
//! parses; only the rows show that the program now computes something else.
//!
//! `wiki/spec/language/expressions.md` fixes multiplicative above additive
//! and both left-associative, and says: "A suffixed form has the precedence,
//! associativity and arity of its base operator", "The comparison operators
//! `==`, `!=`, `<`, `<=`, `>` and `>=` require parentheses when either
//! operand is itself a comparison", and "Right shift keeps its two adjacent
//! `>` tokens, and its suffix is the `%`, `|` or `!` immediately following
//! the second `>`". The spec does not list the remaining tiers. They are
//! the parser's own table (`expressions/operators.rs`), restated here from
//! tightest to loosest so that a change to either one shows.
//!
//! The first test writes `a OP1 b OP2 c` for every ordered pair and requires
//! the rows the tiers give. Because every operator is paired with every
//! other, changing only the second operator of a pair moves the grouping
//! exactly when its tier says so. The second test is the right-shift case in
//! one program: two touching `>` are a shift between two values and two
//! closers after `Vec<Vec<u8>`.
//!
//! Every test lexes with the real lexer and parses through
//! `source_parser::parse`.

use crate::parsing_support::{parse, trees};
use numerics::arithmetic::ArithmeticDomain::{self, Exact, Saturating, Trapping, Wrapping};
use serde_json::json;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::BinaryOperator::{self, *};
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::Item;
use syntax_trees::statement::{StatementNode, TransitionTargetNode};

/// The operators that take a policy suffix, one tier per row, tightest first.
const SUFFIXED_TIERS: [&[(&str, BinaryOperator)]; 3] = [
    &[("*", Multiply), ("/", Divide), ("%", Modulo)],
    &[("+", Add), ("-", Subtract)],
    &[("<<", ShiftLeft), (">>", ShiftRight)],
];

/// The operators that take no suffix, continuing from the shifts to the
/// loosest tier. The two comparison tiers are rows 3 and 4.
const PLAIN_TIERS: [&[(&str, BinaryOperator)]; 7] = [
    &[("&", BitwiseAnd)],
    &[("^", BitwiseXor)],
    &[("|", BitwiseOr)],
    &[
        ("<", Less),
        ("<=", LessOrEqual),
        (">", Greater),
        (">=", GreaterOrEqual),
    ],
    &[("==", Equal), ("!=", NotEqual)],
    &[("&&", And)],
    &[("||", Or)],
];

const SUFFIXES: [(&str, ArithmeticDomain); 4] = [
    ("", Exact),
    ("%", Wrapping),
    ("|", Saturating),
    ("!", Trapping),
];

struct Spelled {
    spelling: String,
    operator: BinaryOperator,
    policy: ArithmeticDomain,
    tier: usize,
}

impl Spelled {
    fn is_comparison(&self) -> bool {
        matches!(
            self.operator,
            Less | LessOrEqual | Greater | GreaterOrEqual | Equal | NotEqual
        )
    }

    /// The row for this operator between `left` and `right`, when the
    /// operator starts at byte `start`. A suffixed token's span covers its
    /// suffix. `>>` reads its suffix from the next token, so its span is the
    /// two `>` whatever follows them.
    fn row(&self, left: &str, right: &str, start: usize) -> String {
        let length = if self.operator == ShiftRight {
            2
        } else {
            self.spelling.len()
        };
        format!(
            "({left} {:?}/{:?}@{start}..{} {right})",
            self.operator,
            self.policy,
            start + length
        )
    }
}

fn every_operator() -> Vec<Spelled> {
    let mut operators = Vec::new();
    for (tier, row) in SUFFIXED_TIERS.iter().enumerate() {
        for (base, operator) in *row {
            for (suffix, policy) in SUFFIXES {
                operators.push(Spelled {
                    spelling: format!("{base}{suffix}"),
                    operator: *operator,
                    policy,
                    tier,
                });
            }
        }
    }
    for (tier, row) in PLAIN_TIERS.iter().enumerate() {
        for (spelling, operator) in *row {
            operators.push(Spelled {
                spelling: (*spelling).to_owned(),
                operator: *operator,
                policy: Exact,
                tier: SUFFIXED_TIERS.len() + tier,
            });
        }
    }
    operators
}

/// The value the body of the machine named `name` ends in.
fn tail_value(trees: &SyntaxTrees, name: &str) -> ExpressionHandle {
    let machine = trees
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine) if machine.name.as_str(trees) == name => Some(machine),
            _ => None,
        })
        .expect("the machine");
    let state = trees
        .items
        .state(trees.items.state_handles(machine.states)[0]);
    let last = *trees
        .items
        .statements(state.statements)
        .last()
        .expect("a body");
    let StatementNode::Transition(transition) = trees.statements.statement(last) else {
        panic!("the body ends in a value");
    };
    let TransitionTargetNode::Value(value) = trees.statements.transition_target(transition.target)
    else {
        panic!("the body ends in a value");
    };
    *value
}

/// An expression as the table holds it: a name as its text, and a binary
/// operation as `(left Operator/Policy@start..end right)` with the span the
/// table records for that row.
fn rows(trees: &SyntaxTrees, expression: ExpressionHandle) -> String {
    match trees.expressions.expression(expression) {
        ExpressionNode::Name { members, .. } => trees
            .expressions
            .identifier_path_members(*members)
            .iter()
            .map(|member| member.as_str(trees))
            .collect::<Vec<_>>()
            .join("::"),
        ExpressionNode::Binary(binary) => {
            let span = trees.expressions.source_span(expression).span;
            format!(
                "({} {:?}/{:?}@{}..{} {})",
                rows(trees, binary.left),
                binary.operator,
                binary.policy,
                span.start,
                span.end,
                rows(trees, binary.right)
            )
        }
        other => panic!("an operand that is neither a name nor an operation: {other:?}"),
    }
}

const HEAD: &str = "machine m(a: u8, b: u8, c: u8) -> u8 { ";

/// Which operation the source puts in parentheses.
#[derive(Clone, Copy, PartialEq)]
enum Grouped {
    Neither,
    First,
    Second,
}

/// `a FIRST b SECOND c` in a machine body, and where each operator starts.
fn written(first: &Spelled, second: &Spelled, grouped: Grouped) -> (String, usize, usize) {
    let mut source = HEAD.to_owned();
    if grouped == Grouped::First {
        source.push('(');
    }
    source.push_str("a ");
    let first_start = source.len();
    source.push_str(&first.spelling);
    source.push(' ');
    if grouped == Grouped::Second {
        source.push('(');
    }
    source.push('b');
    if grouped == Grouped::First {
        source.push(')');
    }
    source.push(' ');
    let second_start = source.len();
    source.push_str(&second.spelling);
    source.push_str(" c");
    if grouped == Grouped::Second {
        source.push(')');
    }
    source.push_str(" }");
    (source, first_start, second_start)
}

/// The rows for `(a FIRST b) SECOND c` when `first_is_operand`, and for
/// `a FIRST (b SECOND c)` otherwise.
fn expected_rows(
    first: &Spelled,
    second: &Spelled,
    first_is_operand: bool,
    first_start: usize,
    second_start: usize,
) -> String {
    if first_is_operand {
        second.row(&first.row("a", "b", first_start), "c", second_start)
    } else {
        first.row("a", &second.row("b", "c", second_start), first_start)
    }
}

/// For every ordered pair of operators, `a OP1 b OP2 c` groups by tier and
/// to the left within a tier, each row keeps its own operator, policy and
/// span, and two comparisons are refused at the second one until one of
/// them is put in parentheses.
#[test]
fn every_pair_of_operators_groups_by_tier_and_to_the_left_within_one() {
    let operators = every_operator();
    assert_eq!(operators.len(), 39);
    let mut refused = 0;

    for first in &operators {
        for second in &operators {
            let (source, first_start, second_start) = written(first, second, Grouped::Neither);
            if !(first.is_comparison() && second.is_comparison()) {
                let parsed = trees(&source);
                assert_eq!(
                    rows(&parsed, tail_value(&parsed, "m")),
                    expected_rows(
                        first,
                        second,
                        first.tier <= second.tier,
                        first_start,
                        second_start
                    ),
                    "{source}"
                );
                continue;
            }

            let error = parse(&source).expect_err(&source);
            assert_eq!(
                (
                    error.message.as_str(),
                    error.source_span.span.start,
                    error.source_span.span.end
                ),
                (
                    "comparison operators cannot be chained without parentheses",
                    second_start,
                    second_start + second.spelling.len()
                ),
                "{source}"
            );
            refused += 1;
            for (grouped, first_is_operand) in [(Grouped::First, true), (Grouped::Second, false)] {
                let (source, first_start, second_start) = written(first, second, grouped);
                let parsed = trees(&source);
                assert_eq!(
                    rows(&parsed, tail_value(&parsed, "m")),
                    expected_rows(first, second, first_is_operand, first_start, second_start),
                    "{source}"
                );
            }
        }
    }
    assert_eq!(refused, 36);
}

/// Two touching `>` between two values are one shift, with the policy of a
/// suffix that touches the second, and the shift is the left operand of a
/// later `>`. After `Vec<Vec<u8>` the same two `>` close two argument
/// lists. Two `>` apart, or a suffix apart from its `>>`, are neither.
#[test]
fn touching_closers_are_one_shift_between_values_and_two_closers_after_arguments() {
    let source = "data Vec<T> {}\n\
                  machine shifts(a: u32, b: u32, c: u32, v: Vec<Vec<u8>>) -> bool { a >> b > c }\n\
                  machine wraps(a: u32, b: u32) -> u32 { a >>% b }";
    let parsed = trees(source);
    let at = |text: &str| source.find(text).expect("the text is in the source");

    let (shift, greater, wrapping) = (at(">> b"), at("> c"), at(">>%"));
    assert_eq!(
        rows(&parsed, tail_value(&parsed, "shifts")),
        format!(
            "((a ShiftRight/Exact@{shift}..{} b) Greater/Exact@{greater}..{} c)",
            shift + 2,
            greater + 1,
        )
    );
    assert_eq!(
        rows(&parsed, tail_value(&parsed, "wraps")),
        format!("(a ShiftRight/Wrapping@{wrapping}..{} b)", wrapping + 2)
    );

    let tree = serde_json::to_value(parsed.snapshot()).expect("a syntax snapshot serializes");
    let argument = &tree["root_items"][1]["states"][0]["parameters"][3]["type_reference"];
    assert_eq!(
        (
            &argument["kind"],
            &argument["base_name"]["text"],
            &argument["arguments"][0]["kind"],
            &argument["arguments"][0]["base_name"]["text"],
            &argument["arguments"][0]["arguments"][0]["name"]["text"],
            argument["arguments"].as_array().map(Vec::len),
            argument["arguments"][0]["arguments"]
                .as_array()
                .map(Vec::len),
        ),
        (
            &json!("generic"),
            &json!("Vec"),
            &json!("generic"),
            &json!("Vec"),
            &json!("u8"),
            Some(1),
            Some(1),
        )
    );

    for (separated, refused_at) in [
        ("machine f(a: u32, b: u32) -> bool { a > > b }", "> b }"),
        ("machine f(a: u32, b: u32) -> u32 { a >> % b }", "% b }"),
    ] {
        let error = parse(separated).expect_err(separated);
        let start = separated.find(refused_at).expect("the refused token");
        assert_eq!(
            (
                error.message.as_str(),
                error.source_span.span.start,
                error.source_span.span.end
            ),
            ("expected expression", start, start + 1),
            "{separated}"
        );
    }
}

use source_files::LoadedSource;
use source_lexer as lexing;
use source_parser as parsing;

#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
#[path = "support/parsing.rs"]
mod parsing_support;
