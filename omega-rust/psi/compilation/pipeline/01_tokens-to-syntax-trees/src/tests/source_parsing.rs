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
    let first = parse(&mut trees, SourceId(11), &first_tokens).unwrap();
    let original = trees.root_item(first[0]).clone();
    let lexed = lexed_source(SourceId(22), "second.omg", "data Second {} data Third {}");
    let backing = lexed.sources.span_or_empty(lexed.batch)[0].source.clone();
    let parsed = parse_sources(lexed, &mut trees).unwrap();
    let sources = parsed.sources.span_or_empty(parsed.batch);
    assert_eq!(sources.len(), 1);
    let appended = &sources[0];
    assert_eq!(appended.source_id, SourceId(22));
    assert_eq!(appended.path, std::path::PathBuf::from("second.omg"));
    assert_eq!(appended.origin, Some(SourceOrigin::User));
    assert_eq!(appended.scope, DependencyScope::Build);
    assert!(Arc::ptr_eq(&appended.source, &backing));
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
    assert!(parse(&mut trees, SourceId(33), &trivia).unwrap().is_empty());
    assert_eq!(trees.root_item_count(), 3);
    let empty = parse_sources(tokens::LexedSources::default(), &mut trees).unwrap();
    assert!(empty.sources.span_or_empty(empty.batch).is_empty());
    assert_eq!(trees.root_item_count(), 3);
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
