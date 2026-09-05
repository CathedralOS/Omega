//! Canonical checkpoint observation for the spelling-level lexer.
//!
//! This is an independently implemented comparator encoding. It is versioned
//! checkpoint tooling, not a serialization of Rust enums or a language ABI.

use tokens::{
    CommentKind, KeywordKind, NumericBase, NumericLiteralKind, PunctuationKind, Token, TokenKind,
};

use crate::lex_error::OUTSIDE_LEXICAL_PROFILE_MESSAGE;
use crate::{LexError, Lexer};

pub const MAGIC: &[u8; 8] = b"OMGLEX1\0";
pub const VERSION: u64 = 2;
const TOKEN_CAPACITY: usize = 16_384;
const DECODED_CAPACITY: usize = 65_536;

pub fn encode(source: &[u8]) -> Vec<u8> {
    let retained = &source[..source.len().min(65_536)];
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    push_u64(&mut output, VERSION);

    if source.len() > retained.len() {
        push_header(&mut output, false, 10, 1, 65_536, 65_536, retained);
        push_u64(&mut output, 0);
        return output;
    }

    let Ok(text) = std::str::from_utf8(retained) else {
        let error = std::str::from_utf8(retained).unwrap_err();
        let start = error.valid_up_to();
        push_header(&mut output, false, 1, 1, start, start + 1, retained);
        push_u64(&mut output, 0);
        return output;
    };

    let observation = Lexer::new(text).tokenize_with_diagnostic();
    let tokens = observation.tokens.as_slice();
    let decoded_overflow = first_decoded_overflow(tokens);
    let capacity_diagnostic =
        if let Some(index) = decoded_overflow.filter(|index| *index <= TOKEN_CAPACITY) {
            Some((11, tokens[index].span.start, tokens[index].span.end, index))
        } else if tokens.len() > TOKEN_CAPACITY {
            let token = &tokens[TOKEN_CAPACITY];
            Some((9, token.span.start, token.span.end, TOKEN_CAPACITY))
        } else {
            None
        };
    let (accepted, code, start, end, token_count) = match capacity_diagnostic {
        Some((code, start, end, token_count)) => (false, code, start, end, token_count),
        None => match observation.diagnostic.as_ref() {
            None => (true, 0, 0, 0, tokens.len()),
            Some(error) => (
                false,
                diagnostic_code(error),
                error.span.start,
                error.span.end,
                tokens.len(),
            ),
        },
    };
    push_header(&mut output, accepted, code, 1, start, end, retained);
    push_u64(&mut output, token_count as u64);
    for token in &tokens[..token_count] {
        push_token(&mut output, retained, token);
    }
    output
}

fn first_decoded_overflow(tokens: &[Token<'_>]) -> Option<usize> {
    let mut decoded_length = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token.kind, TokenKind::StringLiteral) {
            let Some(next) = decoded_length.checked_add(token.lexeme.as_bytes().len()) else {
                return Some(index);
            };
            if next > DECODED_CAPACITY {
                return Some(index);
            }
            decoded_length = next;
        }
    }
    None
}

fn push_header(
    output: &mut Vec<u8>,
    accepted: bool,
    diagnostic: u8,
    source_id: u64,
    diagnostic_start: usize,
    diagnostic_end: usize,
    source: &[u8],
) {
    output.push(u8::from(accepted));
    output.push(diagnostic);
    push_u64(output, source_id);
    push_u64(output, diagnostic_start as u64);
    push_u64(output, diagnostic_end as u64);
    push_u64(output, source.len() as u64);
    output.extend_from_slice(source);
}

fn push_token(output: &mut Vec<u8>, source: &[u8], token: &Token<'_>) {
    let (tag, metadata) = token_tag(token.kind);
    output.push(tag);
    output.extend_from_slice(&metadata);
    push_u64(output, 1);
    push_u64(output, token.span.start as u64);
    push_u64(output, token.span.end as u64);
    let raw = &source[token.span.start..token.span.end];
    push_u64(output, raw.len() as u64);
    output.extend_from_slice(raw);
    let decoded = if matches!(token.kind, TokenKind::StringLiteral) {
        token.lexeme.as_bytes()
    } else {
        &[]
    };
    push_u64(output, decoded.len() as u64);
    output.extend_from_slice(decoded);
}

fn token_tag(kind: TokenKind) -> (u8, [u8; 3]) {
    match kind {
        TokenKind::Identifier => (1, [0; 3]),
        TokenKind::NumericLiteral(NumericLiteralKind::Integer(kind)) => (
            2,
            [
                numeric_base_tag(kind.base),
                u8::from(kind.empty_digits),
                u8::from(kind.has_suffix),
            ],
        ),
        TokenKind::NumericLiteral(NumericLiteralKind::Float(kind)) => (
            3,
            [
                u8::from(kind.has_exponent),
                u8::from(kind.empty_exponent),
                u8::from(kind.has_suffix),
            ],
        ),
        TokenKind::StringLiteral => (4, [0; 3]),
        TokenKind::Keyword(kind) => (5, [keyword_tag(kind), 0, 0]),
        TokenKind::Punctuation(kind) => (6, [punctuation_tag(kind), 0, 0]),
        TokenKind::Whitespace => (7, [0; 3]),
        TokenKind::Comment(CommentKind::Line) => (8, [0; 3]),
        TokenKind::Comment(CommentKind::Block) => (9, [0; 3]),
    }
}

fn numeric_base_tag(base: NumericBase) -> u8 {
    match base {
        NumericBase::Binary => 0,
        NumericBase::Octal => 1,
        NumericBase::Decimal => 2,
        NumericBase::Hexadecimal => 3,
    }
}

fn keyword_tag(kind: KeywordKind) -> u8 {
    match kind {
        KeywordKind::As => 0,
        KeywordKind::CallingConvention => 1,
        KeywordKind::Capability => 2,
        KeywordKind::Contains => 3,
        KeywordKind::Data => 4,
        KeywordKind::Else => 5,
        KeywordKind::Enum => 6,
        KeywordKind::False => 7,
        KeywordKind::Foreign => 8,
        KeywordKind::Host => 9,
        KeywordKind::If => 10,
        KeywordKind::Let => 11,
        KeywordKind::Library => 12,
        KeywordKind::Loop => 13,
        KeywordKind::Machine => 14,
        KeywordKind::Match => 15,
        KeywordKind::Owns => 16,
        KeywordKind::Platform => 17,
        KeywordKind::Pub => 18,
        KeywordKind::Return => 19,
        KeywordKind::SelfType => 20,
        KeywordKind::SelfValue => 21,
        KeywordKind::State => 22,
        KeywordKind::Struct => 23,
        KeywordKind::Target => 24,
        KeywordKind::Transition => 25,
        KeywordKind::True => 26,
        KeywordKind::Use => 27,
        KeywordKind::When => 28,
        KeywordKind::While => 29,
        KeywordKind::Unknown => unreachable!("lexer cannot publish an unknown keyword"),
    }
}

fn punctuation_tag(kind: PunctuationKind) -> u8 {
    match kind {
        PunctuationKind::Ampersand => 0,
        PunctuationKind::AndAnd => 1,
        PunctuationKind::Apostrophe => 2,
        PunctuationKind::Arrow => 3,
        PunctuationKind::Asterisk => 4,
        PunctuationKind::Caret => 5,
        PunctuationKind::Colon => 6,
        PunctuationKind::ColonColon => 7,
        PunctuationKind::Comma => 8,
        PunctuationKind::Dot => 9,
        PunctuationKind::DotDot => 10,
        PunctuationKind::DotDotEqual => 11,
        PunctuationKind::Equal => 12,
        PunctuationKind::EqualEqual => 13,
        PunctuationKind::Exclamation => 14,
        PunctuationKind::ExclamationEqual => 15,
        PunctuationKind::Greater => 16,
        PunctuationKind::GreaterEqual => 17,
        PunctuationKind::GreaterGreater => 18,
        PunctuationKind::Hash => 19,
        PunctuationKind::LeftBrace => 20,
        PunctuationKind::LeftBracket => 21,
        PunctuationKind::LeftParen => 22,
        PunctuationKind::Less => 23,
        PunctuationKind::LessEqual => 24,
        PunctuationKind::LessLess => 25,
        PunctuationKind::Minus => 26,
        PunctuationKind::Percent => 27,
        PunctuationKind::Pipe => 28,
        PunctuationKind::PipePipe => 29,
        PunctuationKind::Plus => 30,
        PunctuationKind::PlusEqual => 31,
        PunctuationKind::MinusEqual => 32,
        PunctuationKind::AsteriskEqual => 33,
        PunctuationKind::SlashEqual => 34,
        PunctuationKind::PercentEqual => 35,
        PunctuationKind::RightBrace => 36,
        PunctuationKind::RightBracket => 37,
        PunctuationKind::RightParen => 38,
        PunctuationKind::Semicolon => 39,
        PunctuationKind::Slash => 40,
        PunctuationKind::Tilde => 41,
        PunctuationKind::Unknown => unreachable!("lexer cannot publish unknown punctuation"),
    }
}

fn diagnostic_code(error: &LexError) -> u8 {
    match error.message.as_str() {
        OUTSIDE_LEXICAL_PROFILE_MESSAGE => 2,
        "unterminated block comment" => 3,
        "unterminated string literal" => 4,
        "unterminated string escape" => 5,
        message if message.starts_with("unsupported escape sequence") => 6,
        "unterminated hex escape" => 7,
        "invalid hex escape digit" => 8,
        message => panic!("unmapped Rust lexical diagnostic: {message}"),
    }
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use tokens::{Span, Token, TokenKind, TokenText};

    use super::{MAGIC, encode, first_decoded_overflow};

    #[test]
    fn observation_preserves_source_and_decoded_string_bytes() {
        let encoded = encode("\"line\\n\" \"café\"".as_bytes());
        assert_eq!(&encoded[..8], MAGIC);
        assert!(encoded.windows(8).any(|window| window == b"\"line\\n\""));
        assert!(encoded.windows(5).any(|window| window == b"line\n"));
        assert!(encoded.windows(5).any(|window| window == "café".as_bytes()));
    }

    #[test]
    fn accepted_observation_has_canonical_empty_diagnostic_coordinates() {
        let encoded = encode(b"alpha");
        assert_eq!(encoded[16], 1);
        assert_eq!(encoded[17], 0);
        assert_eq!(u64::from_le_bytes(encoded[18..26].try_into().unwrap()), 1);
        assert_eq!(u64::from_le_bytes(encoded[26..34].try_into().unwrap()), 0);
        assert_eq!(u64::from_le_bytes(encoded[34..42].try_into().unwrap()), 0);
    }

    #[test]
    fn decoded_lengths_do_not_drift_across_consecutive_token_kinds() {
        let source = br#""a" "bc" x"#;
        let encoded = encode(source);
        let mut cursor = 50 + source.len();
        let token_count = read_u64(&encoded, &mut cursor);
        let mut decoded_lengths = Vec::new();
        for _ in 0..token_count {
            cursor += 4 + 8 + 8 + 8;
            let raw_length = read_u64(&encoded, &mut cursor) as usize;
            cursor += raw_length;
            let decoded_length = read_u64(&encoded, &mut cursor);
            decoded_lengths.push(decoded_length);
            cursor += decoded_length as usize;
        }
        assert_eq!(decoded_lengths, [1, 0, 2, 0, 0]);
    }

    #[test]
    fn observation_retains_prefix_tokens_and_exact_diagnostic_span() {
        let encoded = encode(b"alpha @");
        assert_eq!(encoded[17], 2);
        assert_eq!(u64::from_le_bytes(encoded[26..34].try_into().unwrap()), 6);
        assert_eq!(u64::from_le_bytes(encoded[34..42].try_into().unwrap()), 7);
        let token_count_offset = 50 + b"alpha @".len();
        assert_eq!(
            u64::from_le_bytes(
                encoded[token_count_offset..token_count_offset + 8]
                    .try_into()
                    .unwrap()
            ),
            2
        );
    }

    #[test]
    fn observation_distinguishes_source_escape_spelling() {
        assert_ne!(encode(b"\"a\""), encode(b"\"\\x61\""));
        assert_ne!(encode(b"a b"), encode(b"a\tb"));
    }

    #[test]
    fn observation_uses_one_profile_diagnostic_for_retired_spellings() {
        for source in [
            "变量".as_bytes(),
            "a\u{00a0}b".as_bytes(),
            br#""\u{61}""#,
            b"r#\"raw\"#",
            b"\"line\nvalue\"",
        ] {
            let encoded = encode(source);
            assert_eq!(encoded[16], 0, "source: {source:?}");
            assert_eq!(encoded[17], 2, "source: {source:?}");
        }
    }

    #[test]
    fn observation_enforces_token_capacity_before_later_input() {
        let source = "; ".repeat(16_385);
        let encoded = encode(source.as_bytes());
        assert_eq!(encoded[16], 0);
        assert_eq!(encoded[17], 9);
        let token_count_offset = 50 + source.len();
        assert_eq!(
            u64::from_le_bytes(
                encoded[token_count_offset..token_count_offset + 8]
                    .try_into()
                    .unwrap()
            ),
            16_384
        );
    }

    #[test]
    fn observation_uses_one_raw_byte_for_invalid_utf8_diagnostic() {
        let encoded = encode(b"ok\xfftail");
        assert_eq!(encoded[17], 1);
        assert_eq!(u64::from_le_bytes(encoded[26..34].try_into().unwrap()), 2);
        assert_eq!(u64::from_le_bytes(encoded[34..42].try_into().unwrap()), 3);
    }

    #[test]
    fn observation_retains_only_the_source_capacity_prefix() {
        let source = vec![b'a'; 65_537];
        let encoded = encode(&source);
        assert_eq!(encoded[17], 10);
        assert_eq!(
            u64::from_le_bytes(encoded[42..50].try_into().unwrap()),
            65_536
        );
        assert_eq!(&encoded[50..50 + 65_536], &source[..65_536]);
    }

    #[test]
    fn decoded_capacity_accounting_rejects_the_first_overflowing_token() {
        let tokens = [Token {
            kind: TokenKind::StringLiteral,
            lexeme: TokenText::owned_bytes(vec![0; 65_537]),
            span: Span::new(0, 1),
        }];
        assert_eq!(first_decoded_overflow(&tokens), Some(0));
    }

    fn read_u64(encoded: &[u8], cursor: &mut usize) -> u64 {
        let end = *cursor + 8;
        let value = u64::from_le_bytes(encoded[*cursor..end].try_into().unwrap());
        *cursor = end;
        value
    }
}
