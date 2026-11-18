/*
    L0: the Omega lexer. Source bytes in, tokens out.

    This is the first layer of Omega-V2. It cuts one source file into tokens
    and does nothing else: it decides where each token starts and ends, and it
    refuses files that break a lexical rule. It does not know what any word
    means. The rules it enforces, each with the spec section it comes from, are
    in CONTRACT.md beside this crate; that file is the authority and this code
    implements it.

    How it works. We check once that the whole file is valid UTF-8, then walk
    it byte by byte. All of Omega's syntax is ASCII, so every decision about
    where a token starts can be made from a single byte: a letter or underscore
    starts a word, a digit starts a number, `"` starts a quoted literal, a
    slash followed by a slash or a star starts a comment, and anything else is
    looked up in the punctuation table. Bytes above 0x7F are only legal inside comments and quoted literals,
    where we skip over them without interpreting them, so we never need to
    decode a character except to report how wide an illegal one was.

    Every byte of the file ends up inside exactly one token. Whitespace and
    comments are tokens too, of kinds a parser will skip. That gives us a check
    that does not depend on the lexer's own logic: lay the tokens end to end
    and you must get the original file back, byte for byte. The tests run that
    check on every fixture and on every code example in the spec.

    A token is a kind and a byte range, 32 bytes, with no pointer into the
    source. The spelling of a token is `source[span]`; the bytes a quoted
    literal denotes are computed on request by `decode_quoted_bytes`.
*/

/*
    Why the lexer has no keyword list.

    The obvious design classifies `machine`, `let`, `if` and friends as keyword
    tokens, and Omega V1 does exactly that with a table of 28 words. We do not,
    for two reasons.

    First, the spec gives the lexer nothing to go on. Its only statement about
    words is their shape, `[A-Za-z_][A-Za-z0-9_]*`, plus the remark that "no
    identifier is reserved for builtin types". There is no list of reserved
    words anywhere in it. V1's table is an implementation choice, and it has
    already drifted: it still reserves `state`, which the spec retired.

    Second, which words are special depends on where they appear, and that is
    grammar. `self` is a legal subject inside a domain's `requires` clause and
    a rejected receiver in a machine. Deciding that here would mean the lexer
    knows the grammar. So every word is one kind, `Word`, and the parser, which
    has to know the grammar anyway, decides what each one is.

    Why the first error stops the file.

    A lexer can recover from most errors and keep going, which lets a compiler
    report several mistakes at once. We stop at the first, because a recovered
    token stream is a guess, and every layer after this one would have to be
    correct on guessed input too. One error per file keeps the output either
    exactly right or absent.
*/

/*
    Contract: CONTRACT.md in this crate.
    Tests: `tests/golden_fixtures.rs` (accept and reject fixtures under `fixtures/`),
    `tests/byte_cases.rs` (inputs that cannot be written as readable files), and
    `tests/spec_corpus.rs` (every `omega` example in `wiki/spec`).
    Reference: Omega V1's lexer, `omega-rust/psi/compilation/pipeline/01_source-compilation/
    src/operations/lexing/lexer.rs`. We read it for behaviour the spec leaves open; CONTRACT.md
    lists where we deliberately differ.
*/

#![forbid(unsafe_code)]

mod lex_error;
mod punctuation;
mod quoted_bytes;
mod scanner;
mod source_position;
mod span;
mod token;
mod token_dump;

pub use lex_error::{LexError, LexErrorKind};
pub use punctuation::Punctuation;
pub use quoted_bytes::decode_quoted_bytes;
pub use source_position::{LineIndex, SourcePosition};
pub use span::Span;
pub use token::{Radix, Token, TokenKind};
pub use token_dump::{dump_error, dump_tokens};

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
/// let tokens = lex(source_bytes).expect("this source is lexically valid");
///
/// // Whitespace and comments are tokens, so that every byte is accounted for.
/// // A parser asks each token whether it is trivia and moves on.
/// let spellings: Vec<&[u8]> = tokens
///     .iter()
///     .filter(|token| !token.kind.is_trivia())
///     .filter_map(|token| token.span.slice(source_bytes))
///     .collect();
/// assert_eq!(spellings, [&b"let"[..], b"mask", b":", b"u16", b"=", b"0xFFFFu16", b";"]);
///
/// // `let` and `mask` are the same kind of token. The lexer has no keyword list.
/// assert_eq!(tokens[0].kind, TokenKind::Word);
/// assert_eq!(tokens[2].kind, TokenKind::Word);
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
/// for source_bytes in [&b"let a = 1;"[..], b"b", b"machine main() {}"] {
///     lex_into(source_bytes, &mut tokens).expect("each source is lexically valid");
///     // Use the tokens here, before the next call empties the list.
///     assert_eq!(tokens.last().map(|token| token.span.end), Some(source_bytes.len()));
/// }
/// ```
pub fn lex_into(source_bytes: &[u8], tokens: &mut Vec<Token>) -> Result<(), LexError> {
    tokens.clear();

    if let Err(utf8_error) = std::str::from_utf8(source_bytes) {
        let invalid_start = utf8_error.valid_up_to();
        // `error_len` is None when the file ends in the middle of a character; then the rest is the error.
        let invalid_end = utf8_error
            .error_len()
            .map_or(source_bytes.len(), |invalid_byte_count| {
                invalid_start + invalid_byte_count
            });
        return Err(LexError::new(
            LexErrorKind::InvalidUtf8,
            Span::new(invalid_start, invalid_end),
        ));
    }

    let scan_result = scanner::Scanner::new(source_bytes).scan_all(tokens);
    if scan_result.is_err() {
        tokens.clear();
    }
    scan_result
}
