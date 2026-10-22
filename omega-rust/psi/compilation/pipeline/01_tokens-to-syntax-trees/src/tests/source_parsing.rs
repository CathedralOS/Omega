use crate::{parse, parse_sources};
use source::{DependencyScope, LoadedSource, LoadedSources, SourceId, SourceOrigin};
use source_files_to_tokens::{Lexer, lex_sources};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;

fn lexed_source(source_id: SourceId, path: &str, text: &str) -> tokens::LexedSources {
    let mut loaded = LoadedSources::default();
    loaded.batch = loaded.sources.insert_many([LoadedSource {
        source_id,
        path: path.into(),
        source: Arc::from(text),
        origin: Some(SourceOrigin::User),
        scope: DependencyScope::Build,
    }]);
    lex_sources(loaded).unwrap()
}

#[test]
fn appending_sources_preserves_root_handles_and_source_identity() {
    let mut trees = SyntaxTrees::new(SourceId(11));
    let first_tokens = Lexer::new("data First {} /* trailing trivia */")
        .tokenize()
        .unwrap();
    let first = parse(&mut trees, SourceId(11), &first_tokens)
        .unwrap()
        .items;
    let original = trees.root_item(first[0]).clone();
    let lexed = lexed_source(SourceId(22), "second.omg", "data Second {} data Third {}");
    let backing = lexed.sources.span_or_empty(lexed.batch)[0].source.clone();
    let path_buffer = lexed.sources.span_or_empty(lexed.batch)[0]
        .path
        .as_os_str()
        .as_encoded_bytes()
        .as_ptr();
    let parsed = parse_sources(lexed, &mut trees).unwrap();
    let sources = parsed.sources.span_or_empty(parsed.batch);
    assert_eq!(sources.len(), 1);
    let appended = &sources[0];
    assert_eq!(appended.source_id, SourceId(22));
    assert_eq!(appended.path, std::path::PathBuf::from("second.omg"));
    assert_eq!(appended.origin, Some(SourceOrigin::User));
    assert_eq!(appended.scope, DependencyScope::Build);
    assert!(Arc::ptr_eq(&appended.source, &backing));
    assert_eq!(
        appended.path.as_os_str().as_encoded_bytes().as_ptr(),
        path_buffer
    );
    let second = &appended.root_items;

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 2);
    assert_eq!(trees.root_item_handles(), [first[0], second[0], second[1]]);
    assert_eq!(trees.root_item(first[0]), &original);
    for (handle, expected_source) in [(first[0], SourceId(11)), (second[0], SourceId(22))] {
        let Item::Data(data) = trees.root_item(handle) else {
            panic!("data root")
        };
        assert_eq!(data.name.source_span().source_id, expected_source);
    }
    let trivia = Lexer::new(" // no declarations\n ").tokenize().unwrap();
    assert!(
        parse(&mut trees, SourceId(33), &trivia)
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(trees.root_item_count(), 3);
    let empty = parse_sources(tokens::LexedSources::default(), &mut trees).unwrap();
    assert!(empty.sources.span_or_empty(empty.batch).is_empty());
    assert_eq!(trees.root_item_count(), 3);
}

#[test]
fn parser_retains_one_worker_page_identity_through_nested_handles() {
    const PAGE_IDENTITY: u32 = 7;
    let source_id = SourceId(44);
    let tokens =
        Lexer::new("machine read(value: u64) -> u64 { let selected: u64 = value; selected }")
            .tokenize()
            .unwrap();
    let mut trees = SyntaxTrees::new_with_generation(source_id, PAGE_IDENTITY);
    let roots = parse(&mut trees, source_id, &tokens).unwrap().items;

    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].generation(), PAGE_IDENTITY);
    assert!(
        trees
            .expressions
            .iter_expressions()
            .all(|(handle, _)| handle.generation() == PAGE_IDENTITY)
    );

    let Item::Machine(machine) = trees.root_item(roots[0]) else {
        panic!("machine root")
    };
    assert_eq!(machine.states.start().generation(), PAGE_IDENTITY);
    for state in trees.items.state_handles(machine.states) {
        assert_eq!(state.generation(), PAGE_IDENTITY);
        let state = trees.items.state(*state);
        assert_eq!(state.parameters.start().generation(), PAGE_IDENTITY);
        assert_eq!(state.statements.start().generation(), PAGE_IDENTITY);
        for parameter in trees.items.state_parameters(state.parameters) {
            assert_eq!(parameter.generation(), PAGE_IDENTITY);
            assert_eq!(
                trees
                    .items
                    .state_parameter(*parameter)
                    .type_reference
                    .generation(),
                PAGE_IDENTITY
            );
        }
        assert!(
            trees
                .items
                .statements(state.statements)
                .iter()
                .all(|statement| statement.generation() == PAGE_IDENTITY)
        );
    }
}

#[test]
fn parallel_parser_pages_preserve_source_order_and_direct_handle_lookup() {
    let mut loaded = LoadedSources::default();
    loaded.batch = loaded.sources.insert_many((0..16).map(|position| {
        let padding = if position == 0 {
            "/* padding */".repeat(50_000)
        } else {
            String::new()
        };
        LoadedSource {
            source_id: SourceId(100 + position),
            path: format!("source-{position}.omg").into(),
            source: Arc::from(format!("{padding} data Item{position} {{}}")),
            origin: Some(SourceOrigin::User),
            scope: DependencyScope::Product,
        }
    }));
    let lexed = lex_sources(loaded).unwrap();
    let path_buffers = lexed
        .sources
        .span_or_empty(lexed.batch)
        .iter()
        .map(|source| source.path.as_os_str().as_encoded_bytes().as_ptr() as usize)
        .collect::<Vec<_>>();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let parsed = pool.install(|| crate::parse_source_units(lexed)).unwrap();

    assert_eq!(parsed.page_count(), 16);
    assert_eq!(parsed.sources().len(), 16);
    for (position, source) in parsed.sources().iter().enumerate() {
        assert_eq!(source.source().source_id, SourceId(100 + position));
        assert_eq!(source.source().root_items.len(), 1);
        assert_eq!(
            source.source().path.as_os_str().as_encoded_bytes().as_ptr() as usize,
            path_buffers[position]
        );
        let root = source.source().root_items[0];
        assert_eq!(root.generation(), source.syntax_handle().generation());
        let syntax = parsed.syntax_for(root).expect("root syntax page");
        let Item::Data(data) = syntax.root_item(root) else {
            panic!("data root")
        };
        assert_eq!(data.name.as_str(), format!("Item{position}"));
    }
}

#[test]
fn rejection_preserves_the_parsed_prefix_and_reports_the_current_source() {
    let mut trees = SyntaxTrees::new(SourceId(11));
    let tokens = Lexer::new("data Retained {} ;").tokenize().unwrap();
    let error = parse(&mut trees, SourceId(22), &tokens).unwrap_err();
    assert_eq!(error.source_span.source_id, SourceId(22));
    assert_eq!(error.source_span.span.start, 17);
    assert_eq!(trees.root_item_count(), 1);
    let Item::Data(data) = trees.root_items().next().unwrap() else {
        panic!("data root")
    };
    assert_eq!(data.name.as_str(), "Retained");

    let errors = parse_sources(
        lexed_source(SourceId(33), "appended.omg", "data Another {}\n  ;"),
        &mut trees,
    )
    .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.starts_with("appended.omg:2:3: "));
    assert_eq!(trees.root_item_count(), 2);
    let Item::Data(data) = trees.root_items().nth(1).unwrap() else {
        panic!("batch prefix must remain parsed");
    };
    assert_eq!(data.name.as_str(), "Another");
    assert_eq!(data.name.source_span().source_id, SourceId(33));
}

#[test]
fn import_bounds_preserve_interleaved_root_ordinals_on_both_parser_routes() {
    let cases = [
        ("", 0..0, vec![]),
        ("data Only {}", 0..0, vec![]),
        ("use alpha; data Tail {}", 0..1, vec![0]),
        ("data Head {} use alpha;", 1..2, vec![1]),
        (
            "data Head {} use alpha; let pi(): core::Real = term; data Middle {} use beta; data Tail {}",
            1..4,
            vec![1, 3],
        ),
    ];
    for (text, expected_range, expected_ordinals) in cases {
        let mut trees = SyntaxTrees::default();
        let parsed =
            parse_sources(lexed_source(SourceId(27), "imports.omg", text), &mut trees).unwrap();
        let source = &parsed.sources.span_or_empty(parsed.batch)[0];
        assert_eq!(source.import_root_range, expected_range);
        let actual = source.root_items[source.import_root_range.clone()]
            .iter()
            .enumerate()
            .filter_map(|(offset, root)| {
                matches!(trees.root_item(*root), Item::Use(_))
                    .then_some(source.import_root_range.start + offset)
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected_ordinals);

        let pages =
            crate::parse_source_units(lexed_source(SourceId(27), "imports.omg", text)).unwrap();
        let source = pages.sources()[0].source();
        assert_eq!(source.import_root_range, expected_range);
        let actual = source.root_items[source.import_root_range.clone()]
            .iter()
            .enumerate()
            .filter_map(|(offset, root)| {
                matches!(
                    pages.syntax_for(*root).unwrap().root_item(*root),
                    Item::Use(_)
                )
                .then_some(source.import_root_range.start + offset)
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected_ordinals);
    }
}

#[test]
fn syntax_names_remain_valid_after_all_token_and_source_rows_are_released() {
    let lexed = lexed_source(SourceId(55), "retained.omg", "data Retained {}");
    let mut syntax = SyntaxTrees::default();
    let parsed = parse_sources(lexed, &mut syntax).unwrap();
    let root = parsed.sources.span_or_empty(parsed.batch)[0].root_items[0];
    drop(parsed);
    let Item::Data(data) = syntax.root_item(root) else {
        panic!("data root")
    };
    assert_eq!(data.name.as_str(), "Retained");
    assert_eq!(data.name.source_span().source_id, SourceId(55));
    let retained_name = data.name.clone();
    drop(syntax);
    assert_eq!(retained_name.as_str(), "Retained");
}
