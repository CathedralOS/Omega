//! What the parser builds from the lexer's tokens when only the bytes between
//! tokens change.
//!
//! The test lexes its sources with the real lexer, parses them through
//! `source_parser::parse` and compares the syntax snapshots.

use crate::frontend_fixture::{Lexer, parse_syntax_trees};
use serde_json::Value;

/// Declarations, a constant expression with a group, a record binding, a call
/// with static arguments, an index, and dispatches with a destructuring arm, a
/// guard and tuple patterns.
///
/// It has no machine header clause. `requires a > 0` followed by `b > 0` on
/// the next line parses as two facts and on the same line is refused, and a
/// clause records a token count that includes its comments, so a clause's
/// tree does change with the bytes between its tokens. The code repair queue
/// has the entry.
const PROGRAM: &str = r#"data Pair { first: u8; second: u8; }
data Command { case Stop; case Say(text: u8); }
const LIMIT: u32 = (1 + 2) * 3 - 0x10;
const NAME: Label = "caf\xc3\xa9 é";
machine second_of(pair: Pair) -> u8 { let { first as _, second } = pair; second }
machine pick(command: Command, values: [u8; 2]) -> u8 {
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

fn snapshot(source: &str) -> Value {
    let stream = Lexer::new(source).tokenize().expect("the source lexes");
    let trees = parse_syntax_trees(&stream).unwrap_or_else(|error| {
        let span = error.source_span.span;
        panic!(
            "{} at `{}` in {source:?}",
            error.message,
            &source[span.start..span.end]
        )
    });
    serde_json::to_value(trees.snapshot()).expect("a syntax snapshot serializes")
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
    // The snapshot holds 77 names read from the source. Each has a start and an end, and both moved.
    assert_eq!(moved.names, 77);
    assert!(moved.offsets >= 2 * moved.names, "{}", moved.offsets);
}

use source_files::LoadedSource;
use source_lexer as lexing;
use source_parser as parsing;

#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
