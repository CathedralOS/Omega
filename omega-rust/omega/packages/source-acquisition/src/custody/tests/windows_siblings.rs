use crate::custody::tree::{CacheCustodyKind, verify_retained_cache_child_custody_root};
use crate::test_support::temp_root;
use crate::tree::filesystem::open_absolute_directory_nofollow;

#[test]
fn sibling_walk_rejects_a_different_retained_child_identity() {
    let root = temp_root("sibling-retained-identity");
    let child = root.join("child");
    let other = root.join("other");
    std::fs::create_dir_all(&child).unwrap();
    std::fs::create_dir(&other).unwrap();
    let parent = open_absolute_directory_nofollow(&root).unwrap();
    let retained = open_absolute_directory_nofollow(&other).unwrap();
    let error = verify_retained_cache_child_custody_root(
        &root,
        &parent,
        &child,
        CacheCustodyKind::Git,
        &retained,
        "retained child changed",
    )
    .unwrap_err();
    assert!(error.to_string().contains("retained child changed"));
    drop(retained);
    drop(parent);
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sibling_walk_rejects_a_descendant_in_place_of_a_direct_child() {
    let root = temp_root("sibling-nested-child");
    let child = root.join("one/two");
    std::fs::create_dir_all(&child).unwrap();
    let parent = open_absolute_directory_nofollow(&root).unwrap();
    let retained = open_absolute_directory_nofollow(&child).unwrap();
    let error = verify_retained_cache_child_custody_root(
        &root,
        &parent,
        &child,
        CacheCustodyKind::Git,
        &retained,
        "retained child changed",
    )
    .unwrap_err();
    assert!(error.to_string().contains("not a direct child"));
    drop(retained);
    drop(parent);
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sibling_walk_reconciles_the_named_child_with_its_opened_parent() {
    let root = temp_root("sibling-parent-identity");
    let parent_path = root.join("named");
    let other_parent_path = root.join("other");
    let child = parent_path.join("child");
    std::fs::create_dir_all(&child).unwrap();
    std::fs::create_dir_all(other_parent_path.join("child")).unwrap();
    let parent = open_absolute_directory_nofollow(&other_parent_path).unwrap();
    let retained = open_absolute_directory_nofollow(&child).unwrap();
    let error = verify_retained_cache_child_custody_root(
        &parent_path,
        &parent,
        &child,
        CacheCustodyKind::Git,
        &retained,
        "retained child changed",
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("changed between classification and no-follow open")
    );
    drop(retained);
    drop(parent);
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}
