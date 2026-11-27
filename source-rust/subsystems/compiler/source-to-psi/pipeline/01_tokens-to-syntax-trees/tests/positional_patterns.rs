//! What the parser hands name resolution for a data pattern whose rows do
//! not all name a field: a positional pattern, and a final `..`.
//!
//! The parser cannot know which field a place selects, because that depends
//! on the subject's record. So the fact it owes the next stage is what was
//! written: each component's place counted from zero, whether it binds or
//! waives, and one more row for a final `..` that spans exactly those two
//! bytes. Typing reads these rows and gives each its field
//! (`03_symbol-resolved-trees-to-typed-trees/tests/positional_patterns.rs`).
//!
//! `wiki/spec/language/patterns.md`, "Data and case decomposition": "A
//! positional pattern binds fields by position; binding names need not match
//! declaration names", and "An explicit `..` may waive the remaining
//! components."
//!
//! The tests lex with the real lexer, parse through `source_parser::parse_into`
//! and inspect the syntax trees directly.

use crate::parsing_support::{parse, trees};
use language_core::PatternSelector;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionNode, MatchFieldAction, MatchPattern, TableMatchField};
use syntax_trees::item::Item;
use syntax_trees::statement::StatementNode;

/// How one pattern row picks its field.
#[derive(Debug, PartialEq, Eq)]
enum Selects {
    Named,
    Place(u32),
    Rest,
}

/// One row of a pattern as the parser recorded it: how it selects, the
/// source text its span covers, and the name it binds when it binds one.
#[derive(Debug, PartialEq, Eq)]
struct Row<'source> {
    selects: Selects,
    written: &'source str,
    binds: Option<String>,
}

fn collect_pattern_fields<'trees>(
    parsed: &'trees SyntaxTrees,
    found: &mut Vec<&'trees TableMatchField>,
) {
    for item in parsed.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        for &state_handle in parsed.items.state_handles(machine.states) {
            let state = parsed.items.state(state_handle);
            for &statement in parsed.items.statements(state.statements) {
                match parsed.statements.statement(statement) {
                    StatementNode::LocalData(local) => {
                        found.extend(parsed.expressions.match_fields(local.pattern_fields));
                    }
                    StatementNode::Dispatch(dispatch) => {
                        for arm in &parsed.statements.dispatch(*dispatch).arms {
                            for slot in &arm.patterns {
                                if let MatchPattern::Destructure(pattern) = slot.pattern {
                                    found.extend(parsed.expressions.match_fields(pattern.fields));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    for (_, expression) in parsed.expressions.iter_expressions() {
        if let ExpressionNode::Match(dispatch) = expression {
            for arm in parsed.expressions.match_arms(dispatch.arms) {
                if let MatchPattern::Destructure(pattern) = arm.pattern {
                    found.extend(parsed.expressions.match_fields(pattern.fields));
                }
            }
        }
    }
}

fn pattern_rows(source: &str) -> Vec<Row<'_>> {
    let parsed = trees(source);
    let mut fields = Vec::new();
    collect_pattern_fields(&parsed, &mut fields);
    fields
        .into_iter()
        .map(|field| {
            let span = field.member.source_span().span;
            Row {
                selects: match field.selector {
                    PatternSelector::Name => Selects::Named,
                    PatternSelector::Position(place) => Selects::Place(place),
                    PatternSelector::Rest => Selects::Rest,
                },
                written: &source[span.start..span.end],
                binds: match &field.action {
                    MatchFieldAction::Bind(binding) => Some(binding.as_str(&parsed).to_owned()),
                    MatchFieldAction::Equal(_) | MatchFieldAction::Waive => None,
                },
            }
        })
        .collect()
}

fn place<'source>(place: u32, written: &'source str, binds: Option<&str>) -> Row<'source> {
    Row {
        selects: Selects::Place(place),
        written,
        binds: binds.map(str::to_owned),
    }
}

const REST: Row<'static> = Row {
    selects: Selects::Rest,
    written: "..",
    binds: None,
};

/// A `let`, a `match` arm and a `transition` arm record the same rows for
/// the same pattern, and the final `..` is a row of its own.
#[test]
fn a_positional_pattern_records_each_place_and_its_final_rest() {
    for source in [
        "machine f(x: Pair) -> u8 { let (first, /* omitted */ ..) = x; first }",
        "machine f(x: Pair) -> u8 { match x { (first, ..) -> first, } }",
        "machine f(x: Pair) { transition x { (first, ..) -> done(first), } machine done(v: u8) {} }",
    ] {
        assert_eq!(
            pattern_rows(source),
            [place(0, "first", Some("first")), REST],
            "{source}"
        );
    }
    for (source, expected) in [
        (
            "machine f(x: Command) -> u8 { let (_, operation) = x; operation }",
            vec![
                place(0, "_", None),
                place(1, "operation", Some("operation")),
            ],
        ),
        (
            "machine f(x: Triple) -> u8 { match x { (_, second, ..) -> second, } }",
            vec![
                place(0, "_", None),
                place(1, "second", Some("second")),
                REST,
            ],
        ),
        ("machine f(x: Pair) { let (..) = x; }", vec![REST]),
        ("machine f(x: Empty) { let () = x; }", vec![]),
        (
            "machine f(x: Single) -> u8 { let (only,) = x; only }",
            vec![place(0, "only", Some("only"))],
        ),
    ] {
        assert_eq!(pattern_rows(source), expected, "{source}");
    }
}

/// A positional row names no field: its member is empty, so nothing later
/// can take the binding's spelling for a field name. The `..` row spells
/// the two bytes it spans.
#[test]
fn a_positional_row_names_no_field_and_the_rest_row_spans_two_bytes() {
    let source = "machine f(x: Pair) -> u8 { let (second, ..) = x; second }";
    let parsed = trees(source);
    let mut fields = Vec::new();
    collect_pattern_fields(&parsed, &mut fields);
    let [binding, rest] = fields.as_slice() else {
        panic!("one component and one rest: {fields:?}")
    };
    assert_eq!(binding.member.as_str(&parsed), "");
    assert_eq!(rest.member.as_str(&parsed), "..");
    let start = source.find("..").unwrap();
    let rest_span = rest.member.source_span().span;
    assert_eq!((rest_span.start, rest_span.end), (start, start + 2));
}

/// Each use of a positional binding is a read of its place in the subject:
/// the member has a position and no name. A named pattern's read keeps the
/// field's name and has no position.
#[test]
fn a_positional_binding_is_read_by_its_place() {
    for (source, expected) in [
        (
            "machine f(x: Pair) -> u8 { let (_, second) = x; second }",
            (Some(1), ""),
        ),
        (
            "machine f(x: Pair) -> u8 { match x { (_, second) -> second, } }",
            (Some(1), ""),
        ),
        (
            "machine f(x: Pair) -> u8 { let Pair { first as _, second } = x; second }",
            (None, "second"),
        ),
    ] {
        let parsed = trees(source);
        let found: Vec<_> = parsed
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                ExpressionNode::Member(member) => {
                    Some((member.position, member.member.as_str(&parsed).to_owned()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(found, [(expected.0, expected.1.to_owned())], "{source}");
    }
}

/// A named `let` pattern takes the same final `..`, with or without the
/// record's name in front.
#[test]
fn a_named_let_pattern_records_its_final_rest() {
    for source in [
        "machine f(x: Pair) -> u8 { let { first, .. } = x; first }",
        "machine f(x: Pair) -> u8 { let Pair { first, .. } = x; first }",
        "machine f(x: Pair) -> u8 { let Pair { first, .., } = x; first }",
    ] {
        assert_eq!(
            pattern_rows(source),
            [
                Row {
                    selects: Selects::Named,
                    written: "first",
                    binds: Some("first".to_owned()),
                },
                REST
            ],
            "{source}"
        );
    }
}

/// The message and the source text at which the parser refuses `source`.
fn refusal(source: &str) -> (String, &str) {
    let error = parse(source).expect_err(source);
    let span = error.source_span.span;
    (error.message, &source[span.start..span.end])
}

/// `..` is the last thing in a pattern. A component or a second `..` after
/// it is refused at the comma that would introduce it.
#[test]
fn a_rest_that_is_not_final_is_refused_where_the_pattern_continues() {
    for pattern in ["(.., last)", "(first, .., ..)"] {
        let source = format!("machine f(x: Pair) {{ let {pattern} = x; }}");
        let (message, at) = refusal(&source);
        assert!(message.contains("it is the last thing"), "{message}");
        assert_eq!(at, ",", "{source}");
        let error = parse(&source).unwrap_err();
        assert_eq!(
            error.source_span.span.start,
            source.find("..,").unwrap() + 2,
            "{source}"
        );
    }
    let source = "machine f(x: Pair) -> u8 { match x { (.., last) -> last, } }";
    assert_eq!(refusal(source).1, ",");
    let source = "machine f(x: Pair) -> u8 { let { .., first } = x; first }";
    let (message, at) = refusal(source);
    assert!(message.contains("it is the last thing"), "{message}");
    assert_eq!(at, "first");
}

/// What a component may be. A value test, a repeated binding and one name
/// in parentheses are each refused at the token that makes them so.
#[test]
fn a_component_is_a_binding_name_or_a_waiver() {
    for (pattern, at, says) in [
        ("(first, 0)", "0", "a positional pattern component is"),
        ("(first, true)", "true", "a positional pattern component is"),
        (
            "(first, Pair { a })",
            "Pair",
            "a positional pattern component is",
        ),
        ("(same, same)", "same", "introduced more than once"),
        (
            "(only)",
            "(",
            "one name in parentheses is not a positional pattern",
        ),
    ] {
        let source = format!("machine f(x: Pair) {{ let {pattern} = x; }}");
        let (message, found) = refusal(&source);
        assert!(message.contains(says), "{pattern}: {message}");
        assert_eq!(found, at, "{pattern}");
        if pattern == "(same, same)" {
            let error = parse(&source).unwrap_err();
            assert_eq!(
                error.source_span.span.start,
                source.rfind("same").unwrap(),
                "the second `same` is the repeated one"
            );
        }
    }
    let source = "machine f(x: Pair) -> u8 { let Pair { first: 0, second } = x; second }";
    let (message, at) = refusal(source);
    assert!(message.contains("cannot test its value"), "{message}");
    assert_eq!(at, ":");
}

/// Parentheses around one value are a grouped expression in an arm, and a
/// `;` after at most one name is a proof-output binding. Neither is a
/// positional pattern, so neither records a pattern row.
#[test]
fn a_grouped_value_and_a_proof_output_binding_are_not_positional_patterns() {
    for source in [
        "machine f(x: u8, y: u8) -> u8 { match x { (y) -> 1u8, _ -> 2u8, } }",
        "machine f() { let (value; proof: term) = produce(); }",
        "machine f() { let (; proof: term) = produce(); }",
    ] {
        assert_eq!(pattern_rows(source), [], "{source}");
    }
}

use source_files::LoadedSource;
use source_lexer as lexing;
use source_parser as parsing;

#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
#[path = "support/parsing.rs"]
mod parsing_support;
