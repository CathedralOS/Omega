use crate::identifier::Identifier;
use crate::types::TypeReferenceTable;

#[test]
fn substitution_watermark_excludes_existing_type_nodes() {
    let mut types = TypeReferenceTable::new();
    let existing = types.insert_named(Identifier::generated("T"));
    let watermark = types.node_count();
    let copied = types.insert_named(Identifier::generated("T"));

    let nodes = types.named_nodes_from(watermark, &crate::SyntaxTextOwner::default());
    assert_eq!(nodes, vec![(copied, "T".to_string())]);
    assert_ne!(nodes[0].0, existing);
}
