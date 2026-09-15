use crate::parse;
use source::SourceId;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;

#[test]
fn appending_sources_preserves_root_handles_and_source_identity() {
    let mut trees = SyntaxTrees::new(SourceId(11));
    let first_tokens = Lexer::new("data First {} /* trailing trivia */")
        .tokenize()
        .unwrap();
    let first = parse(&mut trees, SourceId(11), &first_tokens).unwrap();
    let original = trees.root_item(first[0]).clone();
    let second_tokens = Lexer::new("data Second {} data Third {}")
        .tokenize()
        .unwrap();
    let second = parse(&mut trees, SourceId(22), &second_tokens).unwrap();

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
}
