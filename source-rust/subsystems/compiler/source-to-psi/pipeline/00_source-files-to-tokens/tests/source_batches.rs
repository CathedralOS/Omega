use source_files::{DependencyScope, LoadedSource, SourceOrigin};
use source_lexer::lex_file;
use source_text::SourceId;
use std::path::PathBuf;
use std::sync::Arc;
use tokens::{TokenKind, TokenScratch};

#[test]
fn lexing_retains_source_backing_decoded_bytes_and_exact_instance_metadata() {
    let text: Arc<str> = Arc::from(r#"alpha "\xff""#);
    let mut scratch = TokenScratch::default();
    let mut results = Vec::new();
    let mut path_buffers = Vec::new();
    for (position, scope) in [DependencyScope::Build, DependencyScope::Product]
        .into_iter()
        .enumerate()
    {
        let mut source = LoadedSource {
            source_id: SourceId(11 + position),
            path: PathBuf::from("shared.omg"),
            source: text.clone(),
            origin: Some(SourceOrigin::Toolchain),
            scope,
        };
        path_buffers.push(source.path.as_os_str().as_encoded_bytes().as_ptr() as usize);
        results.push(lex_file(&mut source, &mut scratch).unwrap());
    }
    for (position, source) in results.iter().enumerate() {
        assert_eq!(source.source_id, SourceId(11 + position));
        assert_eq!(source.path, PathBuf::from("shared.omg"));
        assert_eq!(
            source.path.as_os_str().as_encoded_bytes().as_ptr() as usize,
            path_buffers[position]
        );
        assert_eq!(source.origin, Some(SourceOrigin::Toolchain));
        let backing = source.tokens.source_owner().unwrap();
        assert!(Arc::ptr_eq(backing, &text));
    }
    assert_eq!(results[0].scope, DependencyScope::Build);
    assert_eq!(results[1].scope, DependencyScope::Product);
    let retained_tokens = results[0].tokens.clone();
    drop(results);
    drop(text);
    assert_eq!(retained_tokens.spelling(&retained_tokens[0]), "alpha");
    let literal = retained_tokens
        .iter()
        .find(|token| token.kind == TokenKind::StringLiteral)
        .unwrap();
    assert_eq!(
        retained_tokens.literal_bytes(literal).unwrap().as_ref(),
        &[0xff]
    );
}

#[test]
fn lexing_reports_exact_source_location_on_failure_and_recycles_scratch() {
    let mut scratch = TokenScratch::default();
    let mut source = LoadedSource {
        path: PathBuf::from("first.omg"),
        source: Arc::from("\n  /*"),
        ..LoadedSource::default()
    };
    let error = lex_file(&mut source, &mut scratch)
        .unwrap_err()
        .into_diagnostic();
    assert_eq!(error.message, "first.omg:2:3: unterminated block comment");

    let mut next = LoadedSource {
        path: PathBuf::from("second.omg"),
        source: Arc::from("alpha"),
        ..LoadedSource::default()
    };
    let lexed = lex_file(&mut next, &mut scratch).unwrap();
    assert_eq!(lexed.tokens.spelling(&lexed.tokens[0]), "alpha");
}
