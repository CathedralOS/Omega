use super::*;
use crate::UnmeasuredProduction;
fn parse(
    loaded: LoadedSources,
    first_page: u32,
    independent_units: bool,
) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
    SourceInput(loaded).parse_batch(first_page, independent_units, &mut UnmeasuredProduction)
}
use source::SourceId;
use std::sync::Arc;

fn loaded(texts: &[&str]) -> LoadedSources {
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| LoadedSource {
            source_id: SourceId(index),
            path: format!("source-{index}.omg").into(),
            source: Arc::from(*text),
            ..Default::default()
        })
        .collect()
}

#[test]
fn stage_batch_preserves_lexical_error_priority_and_source_order() {
    for texts in [
        vec!["machine", "\"unterminated"],
        vec!["machine", "data Good {}", "\"unterminated"],
    ] {
        let input = loaded(&texts);
        let expected = vec![
            source_files_to_tokens::lex(input.clone())
                .unwrap_err()
                .into_diagnostic(),
        ];
        let actual = parse(input, 1, false).unwrap_err();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    }
    let input = loaded(&["machine", "data"]);
    let lexed = source_files_to_tokens::lex(input.clone()).unwrap();
    let expected = tokens_to_syntax_trees::parse(&lexed, Default::default())
        .map_err(|error| vec![error.into_diagnostic()])
        .err()
        .unwrap();
    let actual = parse(input, 1, false).unwrap_err();
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
}

#[test]
fn stage_batch_matches_page_identity_metadata_and_owned_literals() {
    let texts = (0..97)
        .map(|index| format!("const Text{index}: [u8] = \"value\\n{index}\";"))
        .collect::<Vec<_>>();
    let input = loaded(&texts.iter().map(String::as_str).collect::<Vec<_>>());
    let lexed = source_files_to_tokens::lex(input.clone()).unwrap();
    let (expected_pages, expected_sources) = tokens_to_syntax_trees::parse(
        &lexed,
        tokens_to_syntax_trees::ParseOptions {
            first_page_identity: 5,
            ..Default::default()
        },
    )
    .unwrap()
    .into_parts();
    let (actual_pages, actual_sources) = parse(input, 5, false).unwrap();
    assert_eq!(
        format!("{actual_sources:?}"),
        format!("{expected_sources:?}")
    );
    assert_eq!(actual_pages.len(), expected_pages.len());
    for (actual, expected) in actual_pages.iter().zip(&expected_pages) {
        assert_eq!(actual.snapshot(), expected.snapshot());
    }
    let units = parse(loaded(&["data First {}", "data Second {}"]), 1, true)
        .unwrap()
        .0;
    assert_eq!(units.len(), 2);
    assert_eq!(units[0].root_item_handles()[0].generation(), 1);
    assert_eq!(units[1].root_item_handles()[0].generation(), 2);
}
