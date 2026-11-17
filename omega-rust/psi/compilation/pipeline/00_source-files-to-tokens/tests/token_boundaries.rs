use source_files_to_tokens::Lexer;
use tokens::TokenKind;

#[test]
fn punctuation_keeps_longest_matches_and_adjacent_tokens_separate() {
    let source = "+%+=+ <<!<<<= ..=.. >>=> ::->";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let spellings: Vec<_> = tokens
        .iter()
        .filter(|token| !token.is_non_semantic())
        .map(|token| {
            assert!(matches!(token.kind, TokenKind::Punctuation(_)));
            tokens.text(token).as_str()
        })
        .collect();
    assert_eq!(
        spellings,
        [
            "+%", "+=", "+", "<<!", "<<", "<=", "..=", "..", ">>", "=", ">", "::", "->"
        ]
    );
}

#[test]
fn keyword_prefixes_and_suffixes_remain_identifiers() {
    let tokens = Lexer::new("letter machine2 _if If calling_convention_extra")
        .tokenize()
        .unwrap();
    assert!(
        tokens
            .iter()
            .filter(|token| !token.is_non_semantic())
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
