//! Checks shared by the lexer's test files, chosen so they do not depend on how the lexer decides anything.

// Cargo compiles this module once per test file, and each file uses only some of these checks.
#![allow(dead_code)]

use omega_lexer::{Token, TokenKind, lex};

/// The tiling check: tokens must cover the source exactly, in order, with no gap, overlap or empty token.
/// Returns a description of the first violation, or None when the tokens tile the source.
///
/// This is the lexer's independent invariant. It needs nothing from the lexer except the spans, so a
/// bug that makes the scanner skip or double-count bytes shows up here whatever the token kinds say.
pub fn describe_tiling_problem(source_bytes: &[u8], tokens: &[Token]) -> Option<String> {
    let mut expected_start = 0;
    for (token_index, token) in tokens.iter().enumerate() {
        if token.span.start != expected_start {
            return Some(format!(
                "token {token_index} starts at byte {} but the previous token ended at {expected_start}",
                token.span.start
            ));
        }
        if token.span.end <= token.span.start {
            return Some(format!(
                "token {token_index} is empty at byte {}",
                token.span.start
            ));
        }
        expected_start = token.span.end;
    }

    (expected_start != source_bytes.len()).then(|| {
        format!(
            "tokens end at byte {expected_start} but the source has {} bytes",
            source_bytes.len()
        )
    })
}

/// The self-delimiting check: each token that carries meaning, lexed alone, must come back as that
/// same single token. A token whose kind depends on its neighbours would fail this.
pub fn describe_relex_problem(source_bytes: &[u8], tokens: &[Token]) -> Option<String> {
    for (token_index, token) in tokens.iter().enumerate() {
        if token.kind.is_trivia() {
            continue;
        }
        let spelling_bytes = token.span.slice(source_bytes)?;
        let expected_kind = rebase_kind(token.kind, token.span.start);

        match lex(spelling_bytes).as_deref() {
            Ok([lone_token]) if lone_token.kind == expected_kind => {}
            other => {
                return Some(format!(
                    "token {token_index} `{}` is {expected_kind:?} in context but lexes alone as {other:?}",
                    String::from_utf8_lossy(spelling_bytes)
                ));
            }
        }
    }
    None
}

/// A number's kind records an absolute byte offset; shift it to be relative to the token's own start.
fn rebase_kind(kind: TokenKind, token_start: usize) -> TokenKind {
    match kind {
        TokenKind::Integer {
            radix,
            suffix_start,
        } => TokenKind::Integer {
            radix,
            suffix_start: suffix_start - token_start,
        },
        TokenKind::Float { suffix_start } => TokenKind::Float {
            suffix_start: suffix_start - token_start,
        },
        other_kind => other_kind,
    }
}
