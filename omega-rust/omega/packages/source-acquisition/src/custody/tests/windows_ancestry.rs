use super::*;
use crate::custody::platform::verify_windows_open_cache_ancestry_identity;

#[test]
fn windows_ancestry_rejects_replacement_after_classification() {
    let root = temp_root("ancestry-replacement");
    let ancestor = root.join("ancestor");
    std::fs::create_dir_all(&ancestor).unwrap();
    let root = root.canonicalize().unwrap();
    let ancestor = ancestor.canonicalize().unwrap();
    verify_cache_custody_root(&ancestor, CacheCustodyKind::Git).unwrap();
    let classified = std::fs::symlink_metadata(&ancestor).unwrap();
    let parent = open_absolute_directory_nofollow(&root).unwrap();
    std::fs::rename(&ancestor, root.join("original")).unwrap();
    std::fs::create_dir(&ancestor).unwrap();
    let error = verify_windows_open_cache_ancestry_identity(
        CacheCustodyKind::Git,
        &ancestor,
        &classified,
        Some(&parent),
    )
    .expect_err("replacement must not inherit the classified directory identity");
    assert!(
        matches!(error, SourceResolveError::GitCacheInvalid { message, .. }
        if message.contains("changed between classification"))
    );
    drop(parent);
    std::fs::remove_dir_all(&root).unwrap();
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
    let error = verify_windows_open_cache_ancestry_identity(
        CacheCustodyKind::Git,
        &child_path,
        &classified,
        Some(&parent),
    )
    .expect_err("old parent handle must not validate the new parent's child");
    assert!(
        matches!(error, SourceResolveError::GitCacheInvalid { message, .. }
        if message.contains("changed between classification"))
    );
    drop(parent);
    std::fs::remove_dir_all(&root).unwrap();
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
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    std::fs::remove_dir(&junction).unwrap();
    assert!(target.join("cache").is_dir());
    std::fs::remove_dir_all(&root).unwrap();
}
