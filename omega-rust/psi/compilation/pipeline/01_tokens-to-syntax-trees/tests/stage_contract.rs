use source::{LoadedSource, SourceId};
use tokens_to_syntax_trees::{ParseOptions, parse};

#[test]
fn parsing_returns_owned_pages_without_consuming_or_changing_tokens() {
    let loaded = [LoadedSource {
        source_id: SourceId(17),
        path: "empty.omg".into(),
        ..Default::default()
    }]
    .into_iter()
    .collect();
    let tokens = source_files_to_tokens::lex(loaded).unwrap();
    let original = tokens.clone();
    let first = parse(&tokens, Default::default()).unwrap();
    let second = parse(&tokens, Default::default()).unwrap();
    assert_eq!(tokens, original);
    assert_eq!(first.page_count(), 1);
    assert_eq!(first.sources()[0].source().source_id, SourceId(17));
    assert_eq!(first.into_parts(), second.into_parts());
}

#[test]
fn invalid_page_coordinates_refuse_before_publishing_any_syntax() {
    let loaded = [LoadedSource::default(), LoadedSource::default()]
        .into_iter()
        .collect();
    let tokens = source_files_to_tokens::lex(loaded).unwrap();
    let original = tokens.clone();
    for first_page_identity in [0, u32::MAX] {
        let error = parse(
            &tokens,
            ParseOptions {
                first_page_identity,
                ..Default::default()
            },
        )
        .err()
        .expect("invalid page coordinates must refuse");
        assert_eq!(error.message, "parser exhausted its source-page directory");
        assert_eq!(tokens, original);
    }
    assert_eq!(
        parse(&tokens, Default::default()).unwrap().sources().len(),
        2
    );
}
