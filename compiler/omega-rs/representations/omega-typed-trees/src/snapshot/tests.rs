use super::TypedTreesSnapshot;
use crate::TypedTrees;

#[test]
fn snapshots_empty_typed_tree_as_json() {
    let program = TypedTrees::default();
    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);

    assert_eq!(snapshot.roots.data_definitions.len(), 0);
    assert!(snapshot.to_json_pretty().is_ok());
}
