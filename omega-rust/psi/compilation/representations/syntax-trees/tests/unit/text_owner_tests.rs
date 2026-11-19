use crate::text_owner::{SyntaxTextContext, SyntaxTextOwner};
use std::sync::Arc;

#[test]
fn adoption_keeps_independent_page_identities_and_moves_source_ownership() {
    let text: Arc<str> = Arc::from("retained");
    let mut destination = SyntaxTextOwner::default();
    let first = destination.retain(Arc::from("original"));
    let mut source = SyntaxTextOwner::default();
    let second = source.retain(text.clone());
    destination.adopt(source);
    assert_eq!(Arc::strong_count(&text), 2, "adoption moves the owner");
    assert_eq!(destination.source_text(first), "original");
    assert_eq!(destination.source_text(second), "retained");
}

#[test]
#[should_panic(expected = "does not belong to this forest")]
fn unrelated_forest_cannot_resolve_a_name_page() {
    let mut source = SyntaxTextOwner::default();
    let handle = source.retain(Arc::from("owned"));
    SyntaxTextOwner::default().source_text(handle);
}
