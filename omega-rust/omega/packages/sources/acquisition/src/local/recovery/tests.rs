use super::*;
use crate::SourceResolverStorage;
use crate::local::operations::resolve_local_source_snapshot_with_storage;
use crate::snapshot::permissions::make_tree_owner_writable;
use crate::test_support::temp_root;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = temp_root(&format!(
            "local-baseline-cache-{}",
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("source")).unwrap();
        fs::write(root.join("source/main.omg"), "old source\n").unwrap();
        Self(fs::canonicalize(root).unwrap())
    }

    fn storage(&self) -> SourceResolverStorage {
        SourceResolverStorage::for_hardened_base(self.0.join("cache")).unwrap()
    }

    fn capture(&self, storage: &SourceResolverStorage) -> crate::ResolvedLocalSnapshot {
        resolve_local_source_snapshot_with_storage(
            self.0.join("source"),
            storage,
            LocalSourceLimits::default(),
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        make_tree_owner_writable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn historical_snapshot_survives_changed_and_deleted_live_source() {
    let fixture = Fixture::new();
    let storage = fixture.storage();
    let old = fixture.capture(&storage);
    let expected = SourceContentDigest::derive(old.normalized().content_identity.as_bytes());
    fs::write(fixture.0.join("source/main.omg"), "new source\n").unwrap();
    let current = fixture.capture(&storage);
    assert_ne!(old.snapshot_root(), current.snapshot_root());
    fs::remove_dir_all(fixture.0.join("source")).unwrap();
    let recovered = recover_cached_local_source_in_lane(
        old.canonical_live_root(),
        &expected,
        storage.external_local_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered.root, old.snapshot_root());
    assert_eq!(
        fs::read(recovered.root.join("main.omg")).unwrap(),
        b"old source\n"
    );
    assert!(
        !fixture.0.join("source").exists(),
        "historical recovery recreated live source"
    );
}

#[test]
fn absent_collection_does_not_create_an_archive_or_index() {
    let fixture = Fixture::new();
    let storage = fixture.storage();
    let expected = SourceContentDigest::derive(b"missing");
    assert!(
        recover_cached_local_source_in_lane(
            &fixture.0.join("source"),
            &expected,
            storage.external_local_sources(),
            LocalSourceLimits::default()
        )
        .unwrap()
        .is_none()
    );
    assert!(
        !storage
            .external_local_sources()
            .path()
            .join(LOCAL_CACHE_SNAPSHOTS)
            .exists()
    );
}

#[test]
fn byte_identical_content_does_not_cross_local_origins() {
    let fixture = Fixture::new();
    let storage = fixture.storage();
    let old = fixture.capture(&storage);
    let expected = SourceContentDigest::derive(old.normalized().content_identity.as_bytes());
    let other = fixture.0.join("other-origin");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("main.omg"), "old source\n").unwrap();
    assert!(
        recover_cached_local_source_in_lane(
            &other,
            &expected,
            storage.external_local_sources(),
            LocalSourceLimits::default()
        )
        .unwrap()
        .is_none()
    );
    assert!(
        recover_cached_local_source_in_lane(
            Path::new("relative"),
            &expected,
            storage.external_local_sources(),
            LocalSourceLimits::default()
        )
        .is_err()
    );
    // PathBuf::join removes parent components from verbatim Windows roots.
    // Keep the malformed spelling intact so this probe reaches the guard.
    let mut traversing_origin = other.clone();
    let separator = std::path::MAIN_SEPARATOR;
    traversing_origin
        .as_mut_os_string()
        .push(format!("{separator}..{separator}source"));
    assert!(
        traversing_origin
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    );
    assert!(
        recover_cached_local_source_in_lane(
            &traversing_origin,
            &expected,
            storage.external_local_sources(),
            LocalSourceLimits::default()
        )
        .is_err()
    );
}

#[test]
fn corrupted_snapshot_is_rejected_without_repairing_it() {
    let fixture = Fixture::new();
    let storage = fixture.storage();
    let old = fixture.capture(&storage);
    let expected = SourceContentDigest::derive(old.normalized().content_identity.as_bytes());
    let source = old.snapshot_root().join("main.omg");
    let permissions = fs::metadata(&source).unwrap().permissions();
    let root_permissions = fs::metadata(old.snapshot_root()).unwrap().permissions();
    make_tree_owner_writable(old.snapshot_root());
    fs::write(&source, b"tampered source\n").unwrap();
    fs::set_permissions(&source, permissions).unwrap();
    fs::set_permissions(old.snapshot_root(), root_permissions).unwrap();
    assert!(
        recover_cached_local_source_in_lane(
            old.canonical_live_root(),
            &expected,
            storage.external_local_sources(),
            LocalSourceLimits::default()
        )
        .is_err()
    );
    assert_eq!(fs::read(source).unwrap(), b"tampered source\n");
}

#[test]
fn source_and_lookup_limits_are_enforced_independently() {
    let fixture = Fixture::new();
    let storage = fixture.storage();
    let old = fixture.capture(&storage);
    let expected = SourceContentDigest::derive(old.normalized().content_identity.as_bytes());
    assert!(
        recover_with_entry_limit(
            old.canonical_live_root(),
            &expected,
            storage.external_local_sources(),
            LocalSourceLimits::default(),
            0
        )
        .is_err()
    );
    let limits = LocalSourceLimits {
        max_bytes: 1,
        ..LocalSourceLimits::default()
    };
    assert!(
        recover_cached_local_source_in_lane(
            old.canonical_live_root(),
            &expected,
            storage.external_local_sources(),
            limits
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn symlinked_collection_is_not_followed() {
    let fixture = Fixture::new();
    let storage = fixture.storage();
    let expected = SourceContentDigest::derive(b"missing");
    std::os::unix::fs::symlink(
        fixture.0.join("source"),
        storage
            .external_local_sources()
            .path()
            .join(LOCAL_CACHE_SNAPSHOTS),
    )
    .unwrap();
    assert!(
        recover_cached_local_source_in_lane(
            &fixture.0.join("source"),
            &expected,
            storage.external_local_sources(),
            LocalSourceLimits::default()
        )
        .is_err()
    );
    assert_eq!(
        fs::read(fixture.0.join("source/main.omg")).unwrap(),
        b"old source\n"
    );
}
