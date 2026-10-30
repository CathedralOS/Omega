use crate::custody::platform::retain_verified_windows_cache_ancestor;
use crate::custody::tree::{
    CacheCustodyKind, verify_cache_custody_root, verify_retained_cache_custody_root,
};
use crate::test_support::temp_root;
use crate::tree::filesystem::open_absolute_directory_nofollow;

#[test]
fn windows_ancestry_accepts_deep_concrete_directories() {
    let root = temp_root("windows-ancestry-deep");
    let leaf = root.join("one/two/three/four/five/six");
    std::fs::create_dir_all(&leaf).unwrap();
    let leaf = leaf.canonicalize().unwrap();
    let retained = open_absolute_directory_nofollow(&leaf).unwrap();
    verify_retained_cache_custody_root(
        &leaf,
        CacheCustodyKind::LocalSnapshot,
        &retained,
        "retained leaf changed",
    )
    .unwrap();
    drop(retained);
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn windows_ancestry_rejects_replaced_child_after_classification() {
    let root = temp_root("windows-ancestry-child-replaced");
    let child = root.join("child");
    std::fs::create_dir_all(&child).unwrap();
    let parent = open_absolute_directory_nofollow(&root).unwrap();
    let classified = std::fs::symlink_metadata(&child).unwrap();
    std::fs::rename(&child, root.join("old-child")).unwrap();
    std::fs::create_dir(&child).unwrap();
    let error = retain_verified_windows_cache_ancestor(
        CacheCustodyKind::Git,
        &child,
        &classified,
        Some(&parent),
    )
    .unwrap_err();
    assert!(error.to_string().contains("changed between classification"));
    drop(parent);
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn windows_ancestry_reconciles_retained_leaf_after_parent_replacement() {
    let root = temp_root("windows-ancestry-parent-replaced");
    let leaf = root.join("parent/child");
    std::fs::create_dir_all(&leaf).unwrap();
    std::fs::rename(root.join("parent"), root.join("old-parent")).unwrap();
    // Windows may forbid renaming a parent with a live descendant handle.
    // Retain the original identity after moving it, then replace its old name.
    let retained = open_absolute_directory_nofollow(&root.join("old-parent/child")).unwrap();
    std::fs::create_dir_all(&leaf).unwrap();
    let error = verify_retained_cache_custody_root(
        &leaf,
        CacheCustodyKind::LocalSnapshot,
        &retained,
        "retained leaf changed",
    )
    .unwrap_err();
    assert!(error.to_string().contains("retained leaf changed"));
    drop(retained);
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn windows_ancestry_rejects_child_from_a_different_parent() {
    let root = temp_root("ancestry-parent-replacement");
    let parent_path = root.join("parent");
    let child_path = parent_path.join("child");
    std::fs::create_dir_all(&child_path).unwrap();
    let root = root.canonicalize().unwrap();
    let parent_path = parent_path.canonicalize().unwrap();
    let child_path = child_path.canonicalize().unwrap();
    let original_parent = root.join("original-parent");
    std::fs::rename(&parent_path, &original_parent).unwrap();
    let parent = open_absolute_directory_nofollow(&original_parent).unwrap();
    std::fs::create_dir_all(&child_path).unwrap();
    let classified = std::fs::symlink_metadata(&child_path).unwrap();
    let error = retain_verified_windows_cache_ancestor(
        CacheCustodyKind::Git,
        &child_path,
        &classified,
        Some(&parent),
    )
    .expect_err("old parent handle must not validate the new parent's child");
    assert!(error.to_string().contains("changed between classification"));
    drop(parent);
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn windows_ancestry_rejects_junction_parent() {
    let root = temp_root("ancestry-junction");
    let target = root.join("target");
    std::fs::create_dir_all(target.join("cache")).unwrap();
    let junction = root.join("junction");
    let created = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let error = verify_cache_custody_root(&junction.join("cache"), CacheCustodyKind::Git)
        .expect_err("an ancestor junction must not redirect cache custody");
    assert!(error.to_string().contains("non-directory") || error.to_string().contains("reparse"));
    std::fs::remove_dir(&junction).unwrap();
    assert!(target.join("cache").is_dir());
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}
