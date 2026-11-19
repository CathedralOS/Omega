//! The two entry points of the layer: `lex` and `lex_into`.

use crate::errors::lex_error::LexError;
use crate::errors::lex_error_kind::LexErrorKind;
use crate::lexing::token_scanner::TokenScanner;
use crate::locations::source_span::SourceSpan;
use crate::tokens::token::Token;

/// Cuts one source file into tokens, or reports the first lexical rule it breaks.
///
/// The returned tokens cover `source_bytes` exactly: the first starts at byte 0, each starts where
/// the previous one ended, and the last ends at `source_bytes.len()`. An empty file gives no tokens.
///
/// A typical caller lexes a file, skips the tokens a parser does not care about, and reads each
/// remaining token's spelling straight out of the source:
///
/// ```
/// use omega_lexer::{TokenKind, lex};
///
/// let source_bytes = b"let mask: u16 = 0xFFFFu16; // all ones";
/// let tokens = lex(source_bytes).expect("Expected this source to be lexically valid.");
///
/// // Whitespace and comments are tokens, so that every byte is accounted for.
/// // A parser asks each token whether it is trivia and moves on.
/// let spellings: Vec<&[u8]> = tokens
///     .iter()
///     .filter(|token| !token.get_kind().is_trivia())
///     .filter_map(|token| token.get_span().slice(source_bytes))
///     .collect();
///
/// assert_eq!(spellings, [&b"let"[..], b"mask", b":", b"u16", b"=", b"0xFFFFu16", b";"]);
///
/// // `let` and `mask` are the same kind of token. The lexer has no keyword list.
/// assert_eq!(tokens[0].get_kind(), TokenKind::Word);
/// assert_eq!(tokens[2].get_kind(), TokenKind::Word);
/// ```
pub fn lex(source_bytes: &[u8]) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();

    lex_into(source_bytes, &mut tokens)?;

    Ok(tokens)
}

/// Does what `lex` does, but writes the tokens into a list the caller already owns.
///
/// `tokens` is emptied first and keeps its storage, so a caller lexing many files can hand the
/// same list back in for each one and never allocate after the first few. That matters far more
/// than it sounds: with every hardware thread lexing its own files, a fresh list per file makes
/// the threads queue for the memory allocator, and lexing stops getting faster as threads are
/// added. See experiment 4 in EXPERIMENTS.md.
///
/// On a refusal `tokens` is left empty. The output of this layer is exactly right or absent.
///
/// ```
/// use omega_lexer::lex_into;
///
/// let mut tokens = Vec::new();
///
/// for source_bytes in [&b"let a = 1;"[..], b"b", b"machine main() {}"] {
///     lex_into(source_bytes, &mut tokens).expect("Expected each source to be lexically valid.");
///
///     // Use the tokens here, before the next call empties the list.
///     assert_eq!(tokens.last().map(|token| token.get_span().get_end_offset()), Some(source_bytes.len()));
/// }
/// ```
pub fn lex_into(
    source_bytes: &[u8],
    tokens: &mut Vec<Token>,
) -> Result<(), LexError> {
    tokens.clear();

    if let Err(utf8_error) = std::str::from_utf8(source_bytes) {
        let invalid_start = utf8_error.valid_up_to();
        // `error_len` is None when the file ends in the middle of a character; then the rest is the error.
        let invalid_end = utf8_error
            .error_len()
            .map_or(source_bytes.len(), |invalid_byte_count| invalid_start + invalid_byte_count);

        return Err(LexError::new(LexErrorKind::InvalidUtf8, SourceSpan::new(invalid_start, invalid_end)));
    }

    let scan_result = TokenScanner::new(source_bytes).scan_all(tokens);

    if scan_result.is_err() {
        tokens.clear();
    }

    scan_result
}
