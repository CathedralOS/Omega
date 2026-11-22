use crate::frontend_fixture::Lexer;
use tokens::TokenKind;

#[test]
fn punctuation_keeps_longest_matches_and_adjacent_tokens_separate() {
    let source = "+%+=+ <<!<<<= ..=.. >>=> ::->";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let spellings: Vec<_> = tokens
        .iter()
        .filter(|token| !token.is_trivia())
        .map(|token| {
            assert!(matches!(token.kind, TokenKind::Punctuation(_)));
            tokens.spelling(token)
        })
        .collect();
    assert_eq!(
        spellings,
        [
            "+%", "+=", "+", "<<!", "<<", "<=", "..=", "..", ">", ">=", ">", "::", "->"
        ]
    );
}

/// There is no `>>` token (wiki/spec/language/expressions.md): a right shift
/// reaches the parser as two `>` whose spans touch, and `> >` as two that do
/// not, so the parser can tell a shift from two closing angle brackets.
#[test]
fn a_right_shift_is_two_greater_tokens_whose_spans_touch() {
    for (source, touching) in [("a >> b", true), ("a > > b", false)] {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let greaters: Vec<_> = tokens
            .iter()
            .filter(|token| !token.is_trivia() && tokens.spelling(token) != "a")
            .take(2)
            .collect();
        let [first, second] = greaters[..] else {
            panic!("{source}: two tokens after `a`");
        };
        for token in [first, second] {
            assert_eq!(
                token.kind,
                TokenKind::Punctuation(tokens::PunctuationKind::Greater),
                "{source}"
            );
            assert_eq!(tokens.spelling(token), ">", "{source}");
        }
        assert_eq!(first.span.end == second.span.start, touching, "{source}");
    }
}

#[test]
fn keyword_prefixes_and_suffixes_remain_identifiers() {
    let tokens = Lexer::new("letter machine2 _if If calling_convention_extra")
        .tokenize()
        .unwrap();
    assert!(
        tokens
            .iter()
            .filter(|token| !token.is_trivia())
            .all(|token| token.kind == TokenKind::Identifier)
    );
}

#[test]
fn comment_scans_preserve_absolute_utf8_spans_and_nested_delimiters() {
    let source = "\"é\" /* 😀 /* inner */ tail **/ // café\nname";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let mut end = 0;
    for token in tokens.iter() {
        assert_eq!(token.span.start, end);
        assert!(source.is_char_boundary(token.span.end));
        end = token.span.end;
    }
    assert_eq!(end, source.len());
    let name = tokens.last().unwrap();
    assert_eq!(&source[name.span.start..name.span.end], "name");

    let invalid = "\"é\" /* 😀 /* inner */";
    let error = Lexer::new(invalid).tokenize().unwrap_err();
    assert_eq!(error.message, "unterminated block comment");
    assert_eq!(
        &invalid[error.span.start..error.span.end],
        "/* 😀 /* inner */"
    );
}

use source_files::LoadedSource;
use source_lexer as lexing;

#[path = "../../../../../../../tests/support/lexing.rs"]
mod frontend_fixture;
