//! What the lexer hands the parser for any text a source file can hold.
//!
//! The parser reads only tokens, and every later stage reports errors by a
//! token's span. So the lexer owes it two facts for every input: the tokens
//! cover the source byte for byte, in order and without overlap, and a
//! refusal names the bytes of the one character that is not Omega syntax.
//! A lexer that drops a byte, or ends a comment one byte early, still
//! returns tokens; every later span is then wrong by that byte.
//!
//! `wiki/spec/language/data_and_literals.md`, "Lexical profile": "Syntactic
//! whitespace is only space, tab, carriage return, and line feed.
//! Punctuation, operators, keywords, and numeric spellings use ASCII;
//! non-ASCII bytes occur only in comments and literal bodies. Other Unicode
//! whitespace rejects." `wiki/spec/language/expressions.md`: "The token
//! vocabulary is closed and compiler-owned".
//!
//! The lexer takes `&str`: source loading has already refused bytes that
//! are not UTF-8, so these tests start from valid text. They read
//! `TokenStream`, the product the parser consumes, and `LexError::span`.

use crate::frontend_fixture::Lexer;
use tokens::{
    CommentKind, IntegerLiteralKind, NumericBase, NumericLiteralKind, TokenKind, TokenStream,
};

const OUTSIDE_LEXICAL_PROFILE: &str = "spelling is outside the current language profile";

/// Each token's kind, start and end.
fn extents(source: &str) -> Vec<(TokenKind, usize, usize)> {
    Lexer::new(source)
        .tokenize()
        .expect("the source lexes")
        .iter()
        .map(|token| (token.kind, token.span.start, token.span.end))
        .collect()
}

/// The message of the lexer's refusal and the bytes it names.
fn refusal(source: &str) -> (String, usize, usize) {
    let error = Lexer::new(source)
        .tokenize()
        .expect_err("the lexer refuses this source");
    (error.message, error.span.start, error.span.end)
}

/// The one-character operators and delimiters. Written out here, not read
/// from `PunctuationKind`, so that a character added to or dropped from the
/// lexer's vocabulary changes this test's verdict for that byte.
const SINGLE_BYTE_PUNCTUATION: &str = "+-*/%<>=!.:,;&|^~@$#'\\()[]{}";

/// Every ASCII byte, alone in a file, has exactly one verdict: whitespace, a
/// name, a number, one of the 28 punctuation characters, or a refusal that
/// names that one byte.
#[test]
fn every_ascii_byte_alone_is_one_token_or_is_refused_at_that_byte() {
    assert_eq!(SINGLE_BYTE_PUNCTUATION.len(), 28);
    let mut refused = Vec::new();

    for byte in 0u8..=127 {
        let source = char::from(byte).to_string();
        let expected_kind = match byte {
            b' ' | b'\t' | b'\r' | b'\n' => Some(TokenKind::Whitespace),
            b'_' | b'a'..=b'z' | b'A'..=b'Z' => Some(TokenKind::Identifier),
            b'0'..=b'9' => Some(TokenKind::NumericLiteral(NumericLiteralKind::Integer(
                IntegerLiteralKind {
                    base: NumericBase::Decimal,
                    empty_digits: false,
                    has_suffix: false,
                },
            ))),
            _ => None,
        };

        if let Some(kind) = expected_kind {
            assert_eq!(extents(&source), [(kind, 0, 1)], "byte {byte:#04x}");
        } else if SINGLE_BYTE_PUNCTUATION.contains(char::from(byte)) {
            let tokens = Lexer::new(&source).tokenize().expect("punctuation lexes");
            let spellings: Vec<_> = tokens
                .iter()
                .map(|token| {
                    (
                        token.punctuation().map(|kind| kind.lexeme()),
                        token.span.start,
                        token.span.end,
                    )
                })
                .collect();
            assert_eq!(
                spellings,
                [(Some(source.as_str()), 0, 1)],
                "byte {byte:#04x}"
            );
        } else {
            let message = if byte == b'"' {
                "unterminated string literal"
            } else {
                OUTSIDE_LEXICAL_PROFILE
            };
            assert_eq!(
                refusal(&source),
                (message.to_owned(), 0, 1),
                "byte {byte:#04x}"
            );
            refused.push(byte);
        }
    }

    // The control characters other than tab, LF and CR, then `"`, `?`, `` ` ``
    // and DEL.
    let mut expected: Vec<u8> = (0u8..0x20)
        .filter(|byte| !matches!(byte, b'\t' | b'\n' | b'\r'))
        .collect();
    expected.extend([b'"', b'?', b'`', 0x7f]);
    assert_eq!(refused, expected);
}

/// A byte order mark is not whitespace and a NUL is not a token. Each is
/// refused where it stands, the mark at all three of its bytes, and the
/// same text without it lexes.
#[test]
fn a_byte_order_mark_and_a_nul_are_refused_at_their_own_bytes() {
    assert_eq!(
        refusal("\u{feff}let"),
        (OUTSIDE_LEXICAL_PROFILE.to_owned(), 0, 3)
    );
    assert_eq!(extents("let").len(), 1);

    assert_eq!(refusal("a\0b"), (OUTSIDE_LEXICAL_PROFILE.to_owned(), 1, 2));
    assert_eq!(extents("a b").len(), 3);
}

/// A line comment ends before the line feed, so with a CR LF line ending the
/// carriage return is the comment's last byte and every later token starts
/// one byte later than with LF alone. The comment holds a four-byte
/// character, so its end is not its character count.
#[test]
fn a_line_comment_before_crlf_keeps_the_carriage_return() {
    let comment = TokenKind::Comment(CommentKind::Line);
    assert_eq!(
        extents("// \u{1f600}\r\nx"),
        [
            (comment, 0, 8),
            (TokenKind::Whitespace, 8, 9),
            (TokenKind::Identifier, 9, 10),
        ]
    );
    assert_eq!(
        extents("// \u{1f600}\nx"),
        [
            (comment, 0, 7),
            (TokenKind::Whitespace, 7, 8),
            (TokenKind::Identifier, 8, 9),
        ]
    );
}

/// Characters the lexer has a rule about, and some it must refuse: two-,
/// three- and four-byte characters, a NUL, a form feed, `?` and `` ` ``.
const INTERESTING_CHARACTERS: &str = "abefiorux_AEFX0123456789 \t\r\n\"\\/*.+-<>=!&|:'#%^~,;()[]{}@?$`\0\u{c}\u{e9}\u{65e5}\u{1f600}";

const INPUT_COUNT: usize = 60_000;
const LONGEST_INPUT: u64 = 24;

/// A fixed-seed xorshift, so a failure reproduces exactly.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Why `tokens` is not an exact cover of `source`, if it is not.
fn tiling_problem(source: &str, tokens: &TokenStream<'_>) -> Option<String> {
    let mut next = 0;
    for token in tokens.iter() {
        if token.span.start != next {
            return Some(format!(
                "a token starts at {} after one ended at {next}",
                token.span.start
            ));
        }
        if token.span.end <= token.span.start || !source.is_char_boundary(token.span.end) {
            return Some(format!(
                "the token at {}..{} is empty or splits a character",
                token.span.start, token.span.end
            ));
        }
        next = token.span.end;
    }
    (next != source.len()).then(|| format!("the tokens end at {next} of {}", source.len()))
}

/// For thousands of short texts built from those characters in every order:
/// accepted tokens cover the source exactly and each one lexes alone to the
/// same single token, and a refusal names a non-empty range inside the
/// source that does not split a character.
#[test]
fn random_texts_are_covered_exactly_or_refused_at_whole_characters() {
    let characters: Vec<char> = INTERESTING_CHARACTERS.chars().collect();
    let mut generator = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut scratch = tokens::TokenScratch::default();
    let mut alone_scratch = tokens::TokenScratch::default();
    let (mut accepted, mut refused) = (0usize, 0usize);

    for _ in 0..INPUT_COUNT {
        let length = generator.next() % (LONGEST_INPUT + 1);
        let source: String = (0..length)
            .map(|_| characters[(generator.next() % characters.len() as u64) as usize])
            .collect();

        match lexing::lex_unit_reusing(&source, &mut scratch) {
            Ok(tokens) => {
                accepted += 1;
                if let Some(problem) = tiling_problem(&source, &tokens) {
                    panic!("{source:?} was accepted but {problem}");
                }
                for token in tokens.iter() {
                    let spelling = tokens.spelling(token);
                    let alone = lexing::lex_unit_reusing(spelling, &mut alone_scratch)
                        .unwrap_or_else(|error| {
                            panic!("{spelling:?} from {source:?}: {}", error.message)
                        });
                    assert_eq!(
                        alone
                            .iter()
                            .map(|token| (token.kind, token.span.start, token.span.end))
                            .collect::<Vec<_>>(),
                        [(token.kind, 0, spelling.len())],
                        "{spelling:?} from {source:?}"
                    );
                    alone.recycle(&mut alone_scratch);
                }
                tokens.recycle(&mut scratch);
            }
            Err(error) => {
                refused += 1;
                let (start, end) = (error.span.start, error.span.end);
                assert!(
                    start < end && end <= source.len(),
                    "{source:?}: `{}` names the empty or outside range {start}..{end}",
                    error.message
                );
                assert!(
                    source.is_char_boundary(start) && source.is_char_boundary(end),
                    "{source:?}: `{}` at {start}..{end} splits a character",
                    error.message
                );
            }
        }
    }

    // Both outcomes must be common, or half of the lexer is not exercised.
    assert!(accepted > INPUT_COUNT / 20, "only {accepted} accepted");
    assert!(refused > INPUT_COUNT / 20, "only {refused} refused");
}

use source_files::LoadedSource;
use source_lexer as lexing;

#[path = "../../../../../../../tests/support/lexing.rs"]
mod frontend_fixture;
