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
    for (texts, expected_path) in [
        (vec!["machine", "\"unterminated"], "source-1.omg"),
        (
            vec!["machine", "data Good {}", "\"unterminated"],
            "source-2.omg",
        ),
    ] {
        let actual = parse(loaded(&texts), 1, false).unwrap_err();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].stage, Some(DiagnosticStage::Lexing));
        assert!(
            actual[0]
                .message
                .starts_with(&format!("{expected_path}:1:1: unterminated string literal")),
            "{}",
            actual[0].message
        );
    }
    let actual = parse(loaded(&["machine", "data"]), 1, false).unwrap_err();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].stage, Some(DiagnosticStage::Parsing));
    assert!(
        actual[0].message.starts_with("source-0.omg:1:8:"),
        "{}",
        actual[0].message
    );
}

#[test]
fn stage_batch_preserves_page_identity_metadata_and_owned_literals() {
    const FIRST_PAGE: u32 = 5;
    let texts = (0..97)
        .map(|index| format!("const Text{index}: [u8] = \"value\\n{index}\";"))
        .collect::<Vec<_>>();
    let input = loaded(&texts.iter().map(String::as_str).collect::<Vec<_>>());
    let mut timings = CompileTimings::enabled();
    let (actual_pages, actual_sources) = SourceInput(input)
        .parse_batch(FIRST_PAGE, Layout::Shared, &mut timings)
        .unwrap();
    assert_eq!(timings.phases().len(), 1);
    assert_eq!(
        timings.phases()[0].phase,
        "Source ingress: SourceFiles -> SyntaxTrees"
    );
    let records = actual_sources.sources.span_or_empty(actual_sources.batch);
    assert_eq!(records.len(), texts.len());
    for (index, source) in records.iter().enumerate() {
        assert_eq!(source.source_id, SourceId(index));
        assert_eq!(
            source.path,
            std::path::PathBuf::from(format!("source-{index}.omg"))
        );
        let [root] = source.root_items.as_slice() else {
            panic!("expected one constant in {}", source.path.display());
        };
        let page = &actual_pages[(root.generation() - FIRST_PAGE) as usize];
        let syntax_trees::item::Item::Const(constant) = page.root_item(*root) else {
            panic!("expected a constant in {}", source.path.display());
        };
        assert_eq!(constant.name.source_span().source_id, source.source_id);
        assert_eq!(constant.name.as_str(page), format!("Text{index}"));
        let syntax_trees::expression::ExpressionNode::String(bytes) =
            page.expressions.expression(constant.initializer)
        else {
            panic!("expected decoded byte string in {}", source.path.display());
        };
        assert_eq!(bytes.as_ref(), format!("value\n{index}").as_bytes());
    }
    let units = parse(loaded(&["data First {}", "data Second {}"]), 1, true)
        .unwrap()
        .0;
    assert_eq!(units.len(), 2);
    assert_eq!(units[0].root_item_handles()[0].generation(), 1);
    assert_eq!(units[1].root_item_handles()[0].generation(), 2);
}

#[test]
fn stage_batch_validates_page_directory_coordinates_and_parallel_order() {
    for independent in [false, true] {
        let error = parse(LoadedSources::default(), 0, independent).unwrap_err();
        assert_eq!(
            error[0].message,
            "parser exhausted its source-page directory"
        );
        for first_page in [1, u32::MAX] {
            let (pages, sources) =
                parse(LoadedSources::default(), first_page, independent).unwrap();
            assert!(pages.is_empty());
            assert!(sources.sources.span_or_empty(sources.batch).is_empty());
        }
    }
    let (pages, sources) = parse(loaded(&["data Only {}"]), u32::MAX, false).unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(sources.sources.span_or_empty(sources.batch).len(), 1);

    let overflow = parse(loaded(&["data First {}", "data Second {}"]), u32::MAX, true).unwrap_err();
    assert_eq!(
        overflow[0].message,
        "parser exhausted its source-page directory"
    );

    let budget = worker_budget::ExecutionBudget::with_workers(1).unwrap();
    let (pages, sources) =
        budget.run(|| parse(loaded(&["", "", "", "", ""]), u32::MAX - 2, false).unwrap());
    assert_eq!(pages.len(), 3);
    assert_eq!(sources.sources.span_or_empty(sources.batch).len(), 5);

    let parallel_texts: Vec<String> = (0..16)
        .map(|position| {
            let padding = if position == 0 {
                "/* padding */".repeat(5_000)
            } else {
                String::new()
            };
            format!("{padding} data Item{position} {{}}")
        })
        .collect();
    let budget = worker_budget::ExecutionBudget::with_workers(4).unwrap();
    let (pages, sources) = budget.run(|| {
        parse(
            loaded(
                &parallel_texts
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
            1,
            true,
        )
        .unwrap()
    });
    assert_eq!(pages.len(), 16);
    for (position, source) in sources
        .sources
        .span_or_empty(sources.batch)
        .iter()
        .enumerate()
    {
        assert_eq!(source.source_id, SourceId(position));
        let root = source.root_items[0];
        let syntax = &pages[(root.generation() - 1) as usize];
        let syntax_trees::item::Item::Data(data) = syntax.root_item(root) else {
            panic!("data root")
        };
        assert_eq!(data.name.as_str(syntax), format!("Item{position}"));
    }
}

// One retained file used to put every fresh file of its batch on a page of
// its own, so a batch of 259 bundled files became 259 pages. Fresh files now
// share pages as an all-fresh batch would; only the retained file sits alone,
// and pages and records stay in capture order wherever it falls.
#[test]
fn retained_file_keeps_fresh_files_on_shared_pages_in_capture_order() {
    let files: Vec<LoadedSource> = (0..13)
        .map(|index| LoadedSource {
            source_id: SourceId(index),
            path: format!("File{index}.omg").into(),
            source: Arc::from(format!("data File{index} {{}}")),
            ..Default::default()
        })
        .collect();
    let retained = SourceInput(files.iter().cloned().collect())
        .into_parsed_units(&mut CompileTimings::default())
        .unwrap();
    // One worker allows four fresh pages, so the twelve fresh files share
    // pages three at a time.
    let budget = worker_budget::ExecutionBudget::with_workers(1).unwrap();
    let append_fresh = |forest: &mut SourceSyntax, range: std::ops::Range<usize>| {
        budget.run(|| {
            SourceInput(files[range].iter().cloned().collect())
                .append(forest, &mut CompileTimings::default())
                .unwrap()
        });
    };
    for (retained_index, expected_pages) in [
        (0, [0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4]),
        (6, [0, 0, 0, 1, 1, 1, 2, 3, 3, 3, 4, 4, 4]),
        (12, [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4]),
    ] {
        let mut forest = SourceSyntax::default();
        let pages_before = forest.trees.page_count();
        let batch = budget.run(|| {
            SourceInput(files.iter().cloned().collect())
                .append_reusing(&mut forest, &mut CompileTimings::default(), |file| {
                    (file.source_id.0 == retained_index).then(|| &retained[retained_index])
                })
                .unwrap()
        });
        assert_eq!(
            forest.trees.page_count() - pages_before,
            5,
            "retained file {retained_index}"
        );
        let records = batch.sources.span_or_empty(batch.batch);
        assert_eq!(records.len(), files.len());
        let first_page = records[0].root_items.handles[0].generation();
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record.source_id, SourceId(index));
            let [root] = record.root_items.handles.as_slice() else {
                panic!("expected one root in {}", record.path.display());
            };
            assert_eq!(
                root.generation() - first_page,
                expected_pages[index],
                "page of file {index} with retained file {retained_index}"
            );
            let syntax_trees::item::Item::Data(data) = forest.trees.root_item(*root) else {
                panic!("expected data in {}", record.path.display());
            };
            assert_eq!(data.name.as_str(&forest.trees), format!("File{index}"));
        }
        // When the fresh files form one run, their pages are exactly the
        // pages a fresh parse of that run would append.
        if retained_index != 6 {
            let mut expected = SourceSyntax::default();
            if retained_index == 0 {
                append_fresh(&mut expected, 0..1);
                append_fresh(&mut expected, 1..13);
            } else {
                append_fresh(&mut expected, 0..12);
                append_fresh(&mut expected, 12..13);
            }
            assert_eq!(forest, expected, "retained file {retained_index}");
        }
    }
}
