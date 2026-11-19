//! Checks that must hold for any accepted source, chosen so they do not depend on how the lexer decides anything.

use omega_lexer::{Token, TokenKind, lex};

/// The tiling check: tokens must cover the source exactly, in order, with no gap, overlap or empty token.
/// Returns a description of the first violation, or None when the tokens tile the source.
///
/// This is the lexer's independent invariant. It needs nothing from the lexer except the spans, so a
/// bug that makes the scanner skip or double-count bytes shows up here whatever the token kinds say.
pub fn describe_tiling_problem(
    source_bytes: &[u8],
    tokens: &[Token],
) -> Option<String> {
    let mut expected_token_start = 0usize;

    for (token_index, token) in tokens.iter().enumerate() {
        let token_start = token.get_span().get_start_offset();
        let token_end = token.get_span().get_end_offset();

        if token_start != expected_token_start {
            return Some(format!(
                "token {token_index} starts at byte {token_start} but the previous token ended at {expected_token_start}"
            ));
        }

        if token_end <= token_start {
            return Some(format!("token {token_index} is empty at byte {token_start}"));
        }

        expected_token_start = token_end;
    }

    if expected_token_start != source_bytes.len() {
        return Some(format!(
            "tokens end at byte {expected_token_start} but the source has {} bytes",
            source_bytes.len()
        ));
    }

    None
}

/// The self-delimiting check: each token that carries meaning, lexed alone, must come back as that
/// same single token. A token whose kind depends on its neighbours would fail this.
pub fn describe_relex_problem(
    source_bytes: &[u8],
    tokens: &[Token],
) -> Option<String> {
    for (token_index, token) in tokens.iter().enumerate() {
        if token.get_kind().is_trivia() {
            continue;
        }

        let spelling_bytes = token.get_span().slice(source_bytes)?;
        let expected_token_kind = rebase_token_kind(token.get_kind(), token.get_span().get_start_offset());

        match lex(spelling_bytes).as_deref() {
            Ok([lone_token]) if lone_token.get_kind() == expected_token_kind => {}
            relex_result => {
                return Some(format!(
                    "token {token_index} `{}` is {expected_token_kind:?} in context but lexes alone as {relex_result:?}",
                    String::from_utf8_lossy(spelling_bytes)
                ));
            }
        }
    }

    None
}

/// A number's kind records an absolute byte offset; shift it to be relative to the token's own start.
fn rebase_token_kind(
    token_kind: TokenKind,
    token_start: usize,
) -> TokenKind {
    match token_kind {
        TokenKind::Integer { radix, suffix_start_offset } => TokenKind::Integer {
            radix,
            suffix_start_offset: suffix_start_offset - token_start,
        },
        TokenKind::Float { suffix_start_offset } => TokenKind::Float {
            suffix_start_offset: suffix_start_offset - token_start,
        },
        other_token_kind => other_token_kind,
    }
}
