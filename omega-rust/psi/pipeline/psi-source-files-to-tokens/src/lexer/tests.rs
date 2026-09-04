use super::Lexer;
use psi_tokens::{
    CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase,
    NumericLiteralKind, PunctuationKind, TokenKind,
};

use crate::lex_error::OUTSIDE_LEXICAL_PROFILE_MESSAGE;

fn semantic_kinds(source: &str) -> Vec<TokenKind> {
    Lexer::new(source)
        .tokenize()
        .expect("tokenization should succeed")
        .iter()
        .filter(|token| !token.is_non_semantic())
        .map(|token| token.kind)
        .collect()
}

#[test]
fn tokenizes_keywords_and_contextual_entry_as_identifiers() {
    assert_eq!(
        semantic_kinds("machine game entry self Self true false custom"),
        vec![
            TokenKind::Keyword(KeywordKind::Machine),
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Keyword(KeywordKind::SelfValue),
            TokenKind::Keyword(KeywordKind::SelfType),
            TokenKind::Keyword(KeywordKind::True),
            TokenKind::Keyword(KeywordKind::False),
            TokenKind::Identifier,
        ]
    );
}

#[test]
fn tokenizes_retired_invariant_word_as_identifier() {
    assert_eq!(semantic_kinds("invariant"), vec![TokenKind::Identifier]);
}

#[test]
fn accepts_exact_ascii_identifier_grammar() {
    let tokens = Lexer::new("_ alpha Z9 snake_case")
        .tokenize()
        .expect("ASCII identifiers should tokenize");
    assert!(
        tokens
            .iter()
            .filter(|token| !token.is_non_semantic())
            .all(|token| token.kind == TokenKind::Identifier)
    );
}

#[test]
fn rejects_non_ascii_identifier_spelling() {
    assert_profile_rejection("变量", 0, "变".len());
    assert_profile_rejection("café", 3, 5);
    assert_profile_rejection("μέτρο", 0, "μ".len());
    assert_profile_rejection("1é", 1, 3);
}

#[test]
fn tokenizes_multi_character_punctuation() {
    assert_eq!(
        semantic_kinds(":: -> == != << <= >> >= && ||"),
        vec![
            TokenKind::Punctuation(PunctuationKind::ColonColon),
            TokenKind::Punctuation(PunctuationKind::Arrow),
            TokenKind::Punctuation(PunctuationKind::EqualEqual),
            TokenKind::Punctuation(PunctuationKind::ExclamationEqual),
            TokenKind::Punctuation(PunctuationKind::LessLess),
            TokenKind::Punctuation(PunctuationKind::LessEqual),
            TokenKind::Punctuation(PunctuationKind::GreaterGreater),
            TokenKind::Punctuation(PunctuationKind::GreaterEqual),
            TokenKind::Punctuation(PunctuationKind::AndAnd),
            TokenKind::Punctuation(PunctuationKind::PipePipe),
        ]
    );
}

#[test]
fn tokenizes_stable_identity_prefix() {
    assert_eq!(
        semantic_kinds("#1"),
        vec![
            TokenKind::Punctuation(PunctuationKind::Hash),
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                base: NumericBase::Decimal,
                empty_digits: false,
                has_suffix: false,
            })),
        ]
    );
}

#[test]
fn preserves_line_comments_and_whitespace() {
    let tokens = Lexer::new("let // comment\n value")
        .tokenize()
        .expect("tokenization should succeed");

    assert_eq!(tokens.len(), 5);
    assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
    assert_eq!(tokens[1].kind, TokenKind::Whitespace);
    assert!(matches!(
        tokens[2].kind,
        TokenKind::Comment(CommentKind::Line)
    ));
    assert_eq!(tokens[3].kind, TokenKind::Whitespace);
    assert_eq!(tokens[4].kind, TokenKind::Identifier);
    assert_eq!(tokens[4].lexeme.as_str(), "value");
}

#[test]
fn accepts_only_the_closed_syntactic_whitespace_set() {
    let tokens = Lexer::new("a \t\r\nb")
        .tokenize()
        .expect("space, tab, CR, and LF should tokenize");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[1].kind, TokenKind::Whitespace);
    assert_eq!(tokens[1].lexeme.as_str(), " \t\r\n");

    for source in [
        "a\u{000b}b",
        "a\u{000c}b",
        "a\u{00a0}b",
        "a\u{2028}b",
        "a\u{3000}b",
    ] {
        let character = source[1..].chars().next().expect("test character");
        assert_profile_rejection(source, 1, 1 + character.len_utf8());
    }
}

#[test]
fn preserves_nested_block_comments() {
    let tokens = Lexer::new("let /* outer /* inner */ still outer */ value")
        .tokenize()
        .expect("tokenization should succeed");

    assert_eq!(tokens.len(), 5);
    assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
    assert_eq!(tokens[1].kind, TokenKind::Whitespace);
    assert!(matches!(
        tokens[2].kind,
        TokenKind::Comment(CommentKind::Block)
    ));
    assert_eq!(tokens[3].kind, TokenKind::Whitespace);
    assert_eq!(tokens[4].kind, TokenKind::Identifier);
    assert_eq!(tokens[4].lexeme.as_str(), "value");
}

#[test]
fn preserves_non_ascii_comment_and_literal_body_bytes() {
    let source = "// café 变量\n/* μέτρο */ \"café😀\"";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("non-ASCII payload bytes should remain legal");

    assert_eq!(tokens[0].lexeme.as_bytes(), "// café 变量".as_bytes());
    assert_eq!(tokens[1].lexeme.as_bytes(), "/* μέτρο */".as_bytes());
    assert_eq!(tokens[3].kind, TokenKind::StringLiteral);
    assert_eq!(tokens[3].lexeme.as_bytes(), "café😀".as_bytes());
}

#[test]
fn errors_on_unterminated_block_comment() {
    let error = Lexer::new("let /* comment")
        .tokenize()
        .expect_err("tokenization should fail");

    assert_eq!(error.message, "unterminated block comment");
}

#[test]
fn tokenizes_cooked_byte_strings() {
    let cooked = Lexer::new("\"line\\nvalue\"")
        .tokenize()
        .expect("tokenization should succeed");

    assert_eq!(cooked[0].kind, TokenKind::StringLiteral);
    assert_eq!(cooked[0].lexeme.as_str(), "line\nvalue");
}

#[test]
fn hex_escapes_decode_to_exact_bytes() {
    let mut source = String::from("\"");
    for byte in u8::MIN..=u8::MAX {
        source.push_str(&format!("\\x{byte:02x}"));
    }
    source.push('"');

    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenization should succeed");
    let expected: Vec<_> = (u8::MIN..=u8::MAX).collect();

    assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
    assert_eq!(tokens[0].lexeme.as_bytes(), expected);
    assert_eq!(tokens[0].lexeme.try_as_str(), None);
}

#[test]
fn rejects_retired_codepoint_escapes_and_raw_strings() {
    for source in [r#""\u{1f600}""#, r#""\u""#, r#""\u{}""#] {
        assert_profile_rejection(source, 1, 3);
    }

    assert_profile_rejection("r\"raw\"", 0, 2);
    assert_profile_rejection("r##\"raw\"##", 0, 4);
}

#[test]
fn raw_prefix_fragments_remain_ordinary_ascii_tokens() {
    let tokens = Lexer::new("r#1 r###candidate")
        .tokenize()
        .expect("incomplete raw candidates are not reserved V1 syntax");
    assert_eq!(
        tokens
            .iter()
            .filter(|token| !token.is_non_semantic())
            .map(|token| token.lexeme.as_str())
            .collect::<Vec<_>>(),
        ["r", "#", "1", "r", "#", "#", "#", "candidate"]
    );
}

#[test]
fn rejects_raw_newlines_in_quoted_literals() {
    assert_profile_rejection("\"line\nvalue\"", 5, 6);
    assert_profile_rejection("\"line\rvalue\"", 5, 6);
}

#[test]
fn rejects_malformed_hex_escapes() {
    for (source, expected) in [
        (r#""\xG0""#, "invalid hex escape digit"),
        (r#""\x0""#, "invalid hex escape digit"),
        (r#""\x"#, "unterminated hex escape"),
    ] {
        let error = Lexer::new(source)
            .tokenize()
            .expect_err("tokenization should fail");
        assert_eq!(error.message, expected, "source: {source:?}");
    }
}

#[test]
fn tokenizes_integer_and_float_literals_with_metadata() {
    let kinds = semantic_kinds("42 3.14 1. .5 1e10 0xff 0b1010 1_000");

    assert_eq!(
        kinds,
        vec![
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                base: NumericBase::Decimal,
                empty_digits: false,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                has_exponent: false,
                empty_exponent: false,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                has_exponent: false,
                empty_exponent: false,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                has_exponent: false,
                empty_exponent: false,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                has_exponent: true,
                empty_exponent: false,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                base: NumericBase::Hexadecimal,
                empty_digits: false,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                base: NumericBase::Binary,
                empty_digits: false,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                base: NumericBase::Decimal,
                empty_digits: false,
                has_suffix: false,
            })),
        ]
    );
}

#[test]
fn captures_numeric_suffixes_and_empty_parts() {
    let kinds = semantic_kinds("123abc 1e 0x");

    assert_eq!(
        kinds,
        vec![
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                base: NumericBase::Decimal,
                empty_digits: false,
                has_suffix: true,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                has_exponent: true,
                empty_exponent: true,
                has_suffix: false,
            })),
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                base: NumericBase::Hexadecimal,
                empty_digits: true,
                has_suffix: false,
            })),
        ]
    );
}

fn assert_profile_rejection(source: &str, expected_start: usize, expected_end: usize) {
    let error = Lexer::new(source)
        .tokenize()
        .expect_err("spelling outside Lexical Profile V1 should reject");
    assert_eq!(
        error.message, OUTSIDE_LEXICAL_PROFILE_MESSAGE,
        "source: {source:?}"
    );
    assert_eq!(error.span.start, expected_start, "source: {source:?}");
    assert_eq!(error.span.end, expected_end, "source: {source:?}");
}
