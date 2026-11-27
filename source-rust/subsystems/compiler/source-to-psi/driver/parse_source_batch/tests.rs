use super::*;
use diagnostics::DiagnosticStage;
use source_text::SourceId;
use std::sync::Arc;

fn parse(
    loaded: LoadedSources,
    first_page: u32,
    independent_units: bool,
) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
    SourceInput(loaded).parse_batch(
        first_page,
        if independent_units {
            Layout::Independent
        } else {
            Layout::Shared
        },
        &mut CompileTimings::default(),
    )
}

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
            source_lexer::lex(input.clone())
                .unwrap_err()
                .into_diagnostic(),
        ];
        let actual = parse(input, 1, false).unwrap_err();
        assert_eq!(actual, DiagnosticStage::Lexing.attribute(expected));
    }
    let input = loaded(&["machine", "data"]);
    let lexed = source_lexer::lex(input.clone()).unwrap();
    let expected = source_parser::parse(&lexed, Default::default())
        .map_err(|error| vec![error.into_diagnostic()])
        .err()
        .unwrap();
    let actual = parse(input, 1, false).unwrap_err();
    assert_eq!(actual, DiagnosticStage::Parsing.attribute(expected));
}

#[test]
fn stage_batch_matches_page_identity_metadata_and_owned_literals() {
    const FIRST_PAGE: u32 = 5;
    let texts = (0..97)
        .map(|index| format!("const Text{index}: [u8] = \"value\\n{index}\";"))
        .collect::<Vec<_>>();
    let input = loaded(&texts.iter().map(String::as_str).collect::<Vec<_>>());
    let lexed = source_lexer::lex(input.clone()).unwrap();
    let (expected_pages, expected_sources) = source_parser::parse(
        &lexed,
        source_parser::ParseOptions {
            first_page_identity: FIRST_PAGE,
            ..Default::default()
        },
    )
    .unwrap()
    .into_parts();
    let mut timings = CompileTimings::enabled();
    let (actual_pages, actual_sources) =
        SourceInput(input).parse_batch(FIRST_PAGE, Layout::Shared, &mut timings).unwrap();
    assert_eq!(timings.phases().len(), 1);
    assert_eq!(timings.phases()[0].phase, "Source ingress: SourceFiles -> SyntaxTrees");
    assert_eq!(
        format!("{actual_sources:?}"),
        format!("{expected_sources:?}")
    );
    assert_eq!(actual_pages.len(), expected_pages.len());
    for (actual, expected) in actual_pages.iter().zip(&expected_pages) {
        assert_eq!(actual, expected);
    }
    // Both routes call the same parser. Check each returned root against its
    // input file as well, so the routes cannot agree on a misplaced declaration.
    let records = actual_sources.sources.span_or_empty(actual_sources.batch);
    assert_eq!(records.len(), texts.len());
    for (index, source) in records.iter().enumerate() {
        assert_eq!(source.source_id, SourceId(index));
        let [root] = source.root_items.as_slice() else {
            panic!("expected one constant in {}", source.path.display());
        };
        let page = &actual_pages[(root.generation() - FIRST_PAGE) as usize];
        let syntax_trees::item::Item::Const(constant) = page.root_item(*root) else {
            panic!("expected a constant in {}", source.path.display());
        };
        assert_eq!(constant.name.source_span().source_id, source.source_id);
        assert_eq!(constant.name.as_str(page), format!("Text{index}"));
    }
    let units = parse(loaded(&["data First {}", "data Second {}"]), 1, true)
        .unwrap()
        .0;
    assert_eq!(units.len(), 2);
    assert_eq!(units[0].root_item_handles()[0].generation(), 1);
    assert_eq!(units[1].root_item_handles()[0].generation(), 2);
}
