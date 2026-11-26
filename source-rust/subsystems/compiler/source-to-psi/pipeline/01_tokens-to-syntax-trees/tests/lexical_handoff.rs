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
//! The first test changes only the bytes between tokens. Each later test
//! holds one small program in which a wrong token boundary gives a different
//! tree, and reads the fact where the parser writes it: the syntax snapshot,
//! or the expression table when the snapshot does not carry the fact (a
//! literal's value and carrier).
//!
//! Every test lexes with the real lexer and parses through
//! `source_parser::parse`.

use crate::frontend_fixture::Lexer;
use crate::parsing_support::{parse, snapshot, trees};
use serde_json::{Value, json};
use syntax_trees::expression::ExpressionNode;

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

/// The snapshot with each name the parser read from the source written as
/// `text@start..end`, or as its bare text, so that an expected tree fits on
/// a few lines and still states each name's byte offsets when they matter.
fn names_as_text(tree: &Value, with_offsets: bool) -> Value {
    match tree {
        Value::Object(fields) => {
            if let (Some(Value::String(text)), Some(start), Some(end), Some(_)) = (
                fields.get("text"),
                fields.get("start"),
                fields.get("end"),
                fields.get("source_backed"),
            ) {
                return Value::String(if with_offsets {
                    format!("{text}@{start}..{end}")
                } else {
                    text.clone()
                });
            }
            Value::Object(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), names_as_text(value, with_offsets)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| names_as_text(value, with_offsets))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The value a machine's body ends in, for the root item at `item`.
fn tail_value(tree: &Value, item: usize) -> &Value {
    let statements = tree["root_items"][item]["states"][0]["statements"]
        .as_array()
        .expect("a machine body");
    &statements.last().expect("a tail value")["target"]["expression"]
}

/// The message of a parse error and the source text its span covers.
fn refusal(source: &str) -> (String, usize, usize) {
    let error = parse(source).expect_err("the parser refuses this source");
    let span = error.source_span.span;
    (error.message, span.start, span.end)
}

/// How many names the two snapshots share, and how many numbers differ between them.
#[derive(Default)]
struct Moved {
    names: usize,
    offsets: usize,
}

/// The two snapshots must have the same shape and the same values. A number
/// may differ only when it is a token's start or end in the plain source and
/// the spaced snapshot holds where that same token starts or ends. A name read
/// from the source must have moved at both ends.
fn compare_trees(
    plain: &Value,
    spaced: &Value,
    tokens: &[((usize, usize), (usize, usize))],
    moved: &mut Moved,
    path: &str,
) {
    match (plain, spaced) {
        (Value::Object(plain), Value::Object(spaced)) => {
            assert_eq!(
                plain.keys().collect::<Vec<_>>(),
                spaced.keys().collect::<Vec<_>>(),
                "{path}"
            );
            if plain.get("source_backed") == Some(&Value::Bool(true)) {
                assert_ne!(plain["start"], spaced["start"], "{path}");
                assert_ne!(plain["end"], spaced["end"], "{path}");
                moved.names += 1;
            }
            for (key, value) in plain {
                let path = format!("{path}.{key}");
                compare_trees(value, &spaced[key], tokens, moved, &path);
            }
        }
        (Value::Array(plain), Value::Array(spaced)) => {
            assert_eq!(plain.len(), spaced.len(), "{path}");
            for (index, (plain, spaced)) in plain.iter().zip(spaced).enumerate() {
                let path = format!("{path}[{index}]");
                compare_trees(plain, spaced, tokens, moved, &path);
            }
        }
        (Value::Number(plain), Value::Number(spaced)) if plain != spaced => {
            let plain = plain.as_u64().expect("an offset") as usize;
            let spaced = spaced.as_u64().expect("an offset") as usize;
            assert!(
                tokens.iter().any(|(before, after)| {
                    (before.0, after.0) == (plain, spaced) || (before.1, after.1) == (plain, spaced)
                }),
                "{path}: {plain} became {spaced}, which is not where a token moved"
            );
            moved.offsets += 1;
        }
        _ => assert_eq!(plain, spaced, "{path}"),
    }
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

    let mut moved = Moved::default();
    compare_trees(
        &snapshot(PROGRAM),
        &snapshot(&spaced),
        &tokens,
        &mut moved,
        "tree",
    );
    assert_eq!(moved.names, 81);
    assert!(moved.offsets >= 2 * moved.names, "{}", moved.offsets);
}

/// A character of two, three or four bytes in a comment or a quoted literal
/// moves every later name by that many bytes, and reaches the tree only as
/// the literal's bytes. Counting characters instead would put `next` at 63.
#[test]
fn wider_characters_in_comments_and_literals_leave_later_names_at_their_byte_offsets() {
    let source = "// \u{e9} line\nconst label: Label = /* \u{fc} */ \"na\u{ef}ve\"; \
                  /* \u{1f600}\u{65e5} */ const next: Label = label;";
    let tree = names_as_text(&snapshot(source), true);
    let items = tree["root_items"].as_array().expect("root items");

    assert_eq!(items.len(), 2, "comment text declared something");
    assert_eq!(items[0]["name"], "label@17..22");
    assert_eq!(
        items[0]["value"],
        json!({ "kind": "string", "bytes": "na\u{ef}ve".as_bytes() })
    );
    assert_eq!(items[1]["name"], "next@71..75");
    assert_eq!(
        items[1]["value"],
        json!({ "kind": "name", "path": ["label@85..90"] })
    );
    assert_eq!((&source[71..75], &source[85..90]), ("next", "label"));
    // The literal's bytes are numbers in the snapshot, so nothing else in it
    // may hold a byte outside ASCII.
    assert!(tree.to_string().is_ascii());
}

/// `0x1_0`, `0b1_1`, `0o7_7` and `1_000` are one number each: a member's
/// stable number, or one operand of the sum. Radix and `_` change how the
/// number is written, never its value.
#[test]
fn each_radix_spelling_is_one_number_with_the_value_its_radix_gives() {
    let source = "data Packet {\n    #0x1_0 tag: u8;\n    #0b1_1 size: u8;\n}\n\
                  const mask: u32 = 1_000 + 0o7_7;";
    let parsed = trees(source);
    let tree = names_as_text(
        &serde_json::to_value(parsed.snapshot()).expect("a syntax snapshot serializes"),
        true,
    );

    let members: Vec<_> = tree["root_items"][0]["members"]
        .as_array()
        .expect("data members")
        .iter()
        .map(|member| (member["name"].as_str(), member["identity"].as_u64()))
        .collect();
    assert_eq!(
        members,
        [
            (Some("tag@25..28"), Some(16)),
            (Some("size@45..49"), Some(3))
        ]
    );

    // The snapshot prints a literal in the radix it was written in, so the
    // sum's shape comes from the snapshot and the values from the table.
    assert_eq!(
        tree["root_items"][1]["value"],
        json!({
            "kind": "binary",
            "left": { "kind": "integer", "text": "1000" },
            "operator": "add",
            "right": { "kind": "integer", "text": "0o77" },
        })
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
    let tree = names_as_text(&snapshot(source), true);
    let float = |text: &str| json!({ "kind": "float", "text": text });
    let integer = |text: &str| json!({ "kind": "integer", "text": text });

    assert_eq!(
        tree["root_items"][0]["value"],
        json!({
            "kind": "binary",
            "left": {
                "kind": "binary",
                "left": float("1.5"),
                "operator": "add",
                "right": float("2e-3"),
            },
            "operator": "add",
            "right": float("6.02e+23"),
        })
    );
    assert_eq!(
        *tail_value(&tree, 1),
        json!({
            "kind": "indexed",
            "collection": { "kind": "name", "path": ["v@69..70"] },
            "index": {
                "kind": "range",
                "start": integer("0"),
                "end": integer("5"),
                "end_inclusive": false,
            },
        })
    );
}

/// The parser tells the lexer's float from its integer: `#15` numbers a
/// member, and `#1.5` in the same place is refused at the literal.
#[test]
fn a_float_is_refused_where_a_member_number_is_expected() {
    let numbered = snapshot("data Packet {\n    #15 tag: u8;\n}");
    assert_eq!(numbered["root_items"][0]["members"][0]["identity"], 15);

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

    let tree = serde_json::to_value(parsed.snapshot()).expect("a syntax snapshot serializes");
    assert_eq!(
        tree["root_items"][1]["value"],
        json!({
            "kind": "binary",
            "left": { "kind": "float", "text": "1.5f32" },
            "operator": "add",
            "right": { "kind": "float", "text": "2e3f64" },
        })
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
    let tree = names_as_text(&snapshot(source), true);
    let items = tree["root_items"].as_array().expect("root items");

    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0]["value"],
        json!({ "kind": "string", "bytes": b"say \"hi\"; done" })
    );
    assert_eq!(items[1]["name"], "t@43..44");
    assert_eq!(
        items[1]["value"],
        json!({ "kind": "string", "bytes": [0x41, 0x5c, 0x09, 0x00] })
    );
}

/// The language has no raw literal, so a lone `r` is a name like any other,
/// both where it is declared and where it is read.
#[test]
fn a_lone_r_is_an_ordinary_name_on_both_sides_of_a_declaration() {
    let tree = names_as_text(&snapshot("const r: u8 = r;"), true);
    assert_eq!(tree["root_items"][0]["name"], "r@6..7");
    assert_eq!(
        tree["root_items"][0]["value"],
        json!({ "kind": "name", "path": ["r@14..15"] })
    );
}

/// `+!`, `&&`, `::`, `<=` and `+%` each reach the parser as one spelling:
/// `a +!b` is a trapping add of two names, and only the space in `a + !b`
/// makes an add of a negation.
#[test]
fn the_longest_operator_spelling_reaches_the_parser_as_one_operator() {
    let source = "machine p(a: u8, b: u8, e: u8, f: u8) -> bool { a +!b && c::d <= e +% f }\n\
                  machine q(a: u8, b: bool) -> u8 { a + !b }";
    let tree = names_as_text(&snapshot(source), false);
    let name = |text: &str| json!({ "kind": "name", "path": [text] });

    assert_eq!(
        *tail_value(&tree, 0),
        json!({
            "kind": "binary",
            "left": {
                "kind": "binary",
                "left": name("a"),
                "operator": "add_trapping",
                "right": name("b"),
            },
            "operator": "and",
            "right": {
                "kind": "binary",
                "left": { "kind": "name", "path": ["c", "d"] },
                "operator": "less_or_equal",
                "right": {
                    "kind": "binary",
                    "left": name("e"),
                    "operator": "add_wrapping",
                    "right": name("f"),
                },
            },
        })
    );
    assert_eq!(
        *tail_value(&tree, 1),
        json!({
            "kind": "binary",
            "left": name("a"),
            "operator": "add",
            "right": { "kind": "unary", "operator": "!", "operand": name("b") },
        })
    );
}

use source_files::LoadedSource;
use source_lexer as lexing;
use source_parser as parsing;

#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
#[path = "support/parsing.rs"]
mod parsing_support;
