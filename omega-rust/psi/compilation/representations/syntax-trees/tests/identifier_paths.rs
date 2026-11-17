use source::{SourceId, SourceSpan, Span};
use std::sync::Arc;
use syntax_trees::{SyntaxTrees, identifier::Identifier};

#[test]
fn joining_retained_names_keeps_the_full_source_span() {
    let mut trees = SyntaxTrees::new(SourceId(17));
    let owner = trees.text.retain(Arc::from("outer /* gap */ inner"));
    let members = [
        Identifier::source_backed(owner, SourceSpan::new(SourceId(17), Span::new(0, 5))),
        Identifier::source_backed(owner, SourceSpan::new(SourceId(17), Span::new(16, 21))),
    ];
    let joined = Identifier::from_path(&trees, &members);
    assert_eq!(joined.as_str(&trees), "outer::inner");
    assert_eq!(
        joined.source_span(),
        SourceSpan::new(SourceId(17), Span::new(0, 21))
    );
    let single = Identifier::from_path(&trees, &members[..1]);
    assert_eq!(single.as_str(&trees), "outer");
    assert_eq!(single.source_span(), members[0].source_span());
}
