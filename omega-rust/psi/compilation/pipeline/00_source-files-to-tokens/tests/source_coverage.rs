use source_files_to_tokens::Lexer;

#[test]
fn line_comments_leave_the_newline_in_the_token_stream() {
    for source in [
        "// comment\nname",
        "// comment\r\nname",
        "// café\n",
        "//",
        "// comment",
    ] {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let mut end = 0;
        for token in tokens.iter() {
            assert_eq!(token.span.start, end, "missing source bytes in {source:?}");
            end = token.span.end;
        }
        assert_eq!(end, source.len());
        let recovered: String = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(recovered, source);
    }
}
