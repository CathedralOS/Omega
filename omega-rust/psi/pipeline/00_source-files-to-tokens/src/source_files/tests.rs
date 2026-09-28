use super::{lex_sources, load_sources};
use source::{DependencyScope, LoadedSource, LoadedSources, SourceId, SourceOrigin};
use std::path::PathBuf;
use std::sync::Arc;
use tokens::{TokenKind, TokenText};

#[test]
fn loading_preserves_order_and_distinct_scopes_and_reports_unreadable_paths() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let loaded = load_sources(
        vec![
            (path.clone(), DependencyScope::Build),
            (path.clone(), DependencyScope::Product),
        ],
        11,
    )
    .unwrap();
    let batch = loaded.sources.span_or_empty(loaded.batch);
    assert_eq!(batch.len(), 2);
    for (position, scope) in [DependencyScope::Build, DependencyScope::Product]
        .into_iter()
        .enumerate()
    {
        assert_eq!(batch[position].source_id, SourceId(11 + position));
        assert_eq!(batch[position].path, path);
        assert_eq!(batch[position].scope, scope);
        assert_eq!(batch[position].origin, None);
        assert_eq!(
            batch[position].source.as_ref(),
            include_str!("../../Cargo.toml")
        );
    }

    let unreadable = path.join("not-a-directory");
    let errors =
        load_sources(vec![(unreadable.clone(), DependencyScope::Product)], 11).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .message
            .starts_with(&format!("failed to read {}:", unreadable.display()))
    );
}

#[test]
fn lexing_retains_source_backing_decoded_bytes_and_exact_instance_metadata() {
    let text: Arc<str> = Arc::from(r#"alpha "\xff""#);
    let mut loaded = LoadedSources::default();
    loaded.batch = loaded.sources.insert_many(
        [DependencyScope::Build, DependencyScope::Product]
            .into_iter()
            .enumerate()
            .map(|(position, scope)| LoadedSource {
                source_id: SourceId(11 + position),
                path: PathBuf::from("shared.omg"),
                source: text.clone(),
                origin: Some(SourceOrigin::Toolchain),
                scope,
            }),
    );
    let lexed = lex_sources(loaded).unwrap();
    let batch = lexed.sources.span_or_empty(lexed.batch);
    assert_eq!(batch.len(), 2);
    for (position, source) in batch.iter().enumerate() {
        assert_eq!(source.source_id, SourceId(11 + position));
        assert_eq!(source.path, PathBuf::from("shared.omg"));
        assert_eq!(source.origin, Some(SourceOrigin::Toolchain));
        assert!(Arc::ptr_eq(&source.source, &text));
        let TokenText::Shared {
            source: backing, ..
        } = &source.tokens[0].lexeme
        else {
            panic!("identifier must retain its source");
        };
        assert!(Arc::ptr_eq(backing, &text));
    }
    assert_eq!(batch[0].scope, DependencyScope::Build);
    assert_eq!(batch[1].scope, DependencyScope::Product);
    let retained_tokens = batch[0].tokens.clone();
    drop(lexed);
    drop(text);
    assert_eq!(retained_tokens[0].lexeme.as_str(), "alpha");
    let literal = retained_tokens
        .iter()
        .find(|token| token.kind == TokenKind::StringLiteral)
        .unwrap();
    assert_eq!(literal.lexeme.as_bytes(), &[0xff]);
}

#[test]
fn lexing_stops_at_the_first_failure_with_its_exact_source_location() {
    let mut loaded = LoadedSources::default();
    loaded.batch = loaded
        .sources
        .insert_many(
            ["first.omg", "second.omg"]
                .into_iter()
                .map(|path| LoadedSource {
                    path: PathBuf::from(path),
                    source: Arc::from("\n  /*"),
                    ..LoadedSource::default()
                }),
        );
    let errors = lex_sources(loaded).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "first.omg:2:3: unterminated block comment"
    );
}
