use source_files::LoadedSource;
use source_parser::{ParseOptions, parse};
use source_text::SourceId;

#[test]
fn parsing_returns_owned_pages_without_consuming_or_changing_tokens() {
    let loaded = [LoadedSource {
        source_id: SourceId(17),
        path: "empty.omg".into(),
        ..Default::default()
    }]
    .into_iter()
    .collect();
    let tokens = source_lexer::lex(loaded).unwrap();
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
    let tokens = source_lexer::lex(loaded).unwrap();
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

#[test]
fn shared_pages_validate_the_pages_actually_used_after_rounding() {
    let loaded = (0..5)
        .map(|index| LoadedSource {
            source_id: SourceId(index),
            ..Default::default()
        })
        .collect();
    let tokens = source_lexer::lex(loaded).unwrap();
    let original = tokens.clone();
    let budget = worker_budget::ExecutionBudget::with_workers(1).unwrap();
    // Five sources divided among at most four pages need three two-source chunks.
    let parsed = budget.run(|| {
        parse(
            &tokens,
            ParseOptions {
                first_page_identity: u32::MAX - 2,
                ..Default::default()
            },
        )
        .unwrap()
    });
    assert_eq!(parsed.page_count(), 3);
    assert_eq!(parsed.sources().len(), 5);
    assert_eq!(tokens, original);
}

#[test]
fn empty_batches_refuse_zero_page_identity_and_accept_valid_extremes() {
    let tokens = source_lexer::lex(Default::default()).unwrap();
    let original = tokens.clone();
    for layout in [
        source_parser::ParseLayout::SharedPages,
        source_parser::ParseLayout::IndependentUnits,
    ] {
        let error = parse(
            &tokens,
            ParseOptions {
                first_page_identity: 0,
                layout,
            },
        )
        .err()
        .expect("zero page identity must refuse even without sources");
        assert_eq!(error.message, "parser exhausted its source-page directory");
        for first_page_identity in [1, u32::MAX] {
            let parsed = parse(
                &tokens,
                ParseOptions {
                    first_page_identity,
                    layout,
                },
            )
            .unwrap();
            assert_eq!(parsed.page_count(), 0);
            assert!(parsed.sources().is_empty());
        }
    }
    assert_eq!(tokens, original);
}

#[test]
fn a_single_page_can_use_the_last_identity() {
    let tokens =
        source_lexer::lex([LoadedSource::default()].into_iter().collect()).unwrap();
    let parsed = parse(
        &tokens,
        ParseOptions {
            first_page_identity: u32::MAX,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(parsed.page_count(), 1);
    assert_eq!(parsed.sources().len(), 1);
}
