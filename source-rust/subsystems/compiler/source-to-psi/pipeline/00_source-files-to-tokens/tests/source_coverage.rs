use crate::frontend_fixture::Lexer;

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

// Parsing reuses these buffers across files. Check both source spelling and
// decoded payload as the same scratch storage is returned to the lexer.
#[test]
fn token_spans_reconstruct_the_file_and_each_token_lexes_on_its_own() {
    let mut scratch = tokens::TokenScratch::default();
    for source in [
        "",
        " \t\r\n",
        "machine main() -> u32 { /* 文 /* inner */ */ 0xff_u32 } // fin\r\n",
        r#""café\n\xff" ".." 1. .5 1..2 a >> b Vec<Vec<T>>"#,
        "123abc 1e 0x",
    ] {
        let stream = lexing::lex_unit_reusing(source, &mut scratch).unwrap();
        let mut next = 0;
        for token in stream.iter() {
            assert_eq!(token.span.start, next, "gap or overlap in {source:?}");
            assert!(token.span.end > next);
            let spelling = stream.spelling(token);
            let isolated = Lexer::new(spelling).tokenize().unwrap();
            assert_eq!(isolated.len(), 1, "{spelling:?}");
            assert_eq!(isolated[0].kind, token.kind, "{spelling:?}");
            assert_eq!(isolated.spelling(&isolated[0]), spelling);
            assert_eq!(
                isolated.literal_bytes(&isolated[0]),
                stream.literal_bytes(token),
                "{spelling:?}"
            );
            next = token.span.end;
        }
        assert_eq!(next, source.len(), "unscanned suffix in {source:?}");
        stream.recycle(&mut scratch);
    }
}

use source_files::LoadedSource;
use source_lexer as lexing;

#[path = "../../../../../../../tests/support/lexing.rs"]
mod frontend_fixture;
