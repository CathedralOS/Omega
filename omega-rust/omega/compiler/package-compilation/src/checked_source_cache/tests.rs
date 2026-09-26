//! Fixture trees and coverage for the persistent checked-source cache.

use super::{CheckedSourceCache, CheckedSourceCacheLimits, CheckedSourceCacheOutcome};
use crate::capture_package_source_input;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-checked-source-cache-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary cache fixture tree");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create fixture directory");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        unseal_source_tree(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn seal_source_tree(root: &Path) {
    set_source_tree_permissions(root, true)
}

fn unseal_source_tree(root: &Path) {
    set_source_tree_permissions(root, false)
}

#[cfg(unix)]
fn set_source_tree_permissions(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(root).expect("inspect source tree entry");
    if metadata.file_type().is_symlink() {
        return;
    }
    if metadata.is_dir() {
        if !sealed {
            fs::set_permissions(root, fs::Permissions::from_mode(0o755))
                .expect("unseal source directory");
        }
        for entry in fs::read_dir(root).expect("enumerate source directory") {
            set_source_tree_permissions(&entry.expect("read source entry").path(), sealed);
        }
        if sealed {
            fs::set_permissions(root, fs::Permissions::from_mode(0o555))
                .expect("seal source directory");
        }
    } else {
        let executable = metadata.permissions().mode() & 0o111 != 0;
        let mode = match (sealed, executable) {
            (true, true) => 0o555,
            (true, false) => 0o444,
            (false, true) => 0o755,
            (false, false) => 0o644,
        };
        fs::set_permissions(root, fs::Permissions::from_mode(mode))
            .expect("set source file permissions");
    }
}

#[cfg(not(unix))]
fn set_source_tree_permissions(_root: &Path, _sealed: bool) {}

fn sealed_fixture(root: &Path) {
    fs::write(root.join("main.omg"), "data Main { value: u8; }\n").expect("write main");
    let nested = root.join("nested");
    fs::create_dir(&nested).expect("create nested directory");
    fs::write(nested.join("part.omg"), "machine answer() -> u64 { 42 }\n")
        .expect("write nested source");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let tool = nested.join("tool.sh");
        fs::write(&tool, b"exit 0\n").expect("write executable");
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o555)).expect("seal tool");
        std::os::unix::fs::symlink("nested", root.join("link")).expect("create fixture link");
    }
    seal_source_tree(root);
}

#[test]
fn cold_capture_stores_a_verified_record_matching_the_authoritative_capture() {
    let tree = TempTree::new();
    let root = tree.path("package");
    sealed_fixture(&root);
    let cache_dir = tree.path("cache");

    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");
    let cold = cache.capture(&root).expect("cold capture");
    assert_eq!(cold.outcome(), CheckedSourceCacheOutcome::Cold);
    assert!(cold.record_stored());

    let authoritative =
        capture_package_source_input(&root).expect("authoritative capture of the same root");
    assert_eq!(
        cold.canonical_source_metadata(),
        authoritative.canonical_source_metadata(),
        "the cached cold capture commits exactly the index the hashing capture derives"
    );
    assert_eq!(
        1,
        fs::read_dir(&cache_dir).expect("enumerate cache").count(),
        "one record is stored for one root"
    );
}

#[test]
fn warm_capture_replays_the_stored_index_without_rehashing() {
    let tree = TempTree::new();
    let root = tree.path("package");
    sealed_fixture(&root);
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");

    let cold = cache.capture(&root).expect("cold capture");
    let warm = cache.capture(&root).expect("warm capture");
    assert_eq!(warm.outcome(), CheckedSourceCacheOutcome::Warm);
    assert_eq!(
        cold.canonical_source_metadata(),
        warm.canonical_source_metadata(),
        "the warm index is byte-identical to the freshly checked index"
    );
    // A second cache handle over the same directory still hits: the store is
    // genuinely persistent across invocations, not process-local state.
    let reopened = CheckedSourceCache::open_or_create(&cache_dir).expect("reopen cache");
    let replayed = reopened.capture(&root).expect("reopened capture");
    assert_eq!(replayed.outcome(), CheckedSourceCacheOutcome::Warm);
    assert_eq!(
        cold.canonical_source_metadata(),
        replayed.canonical_source_metadata()
    );
}

#[cfg(unix)]
#[test]
fn edited_content_produces_a_cold_capture_and_new_commitment() {
    let tree = TempTree::new();
    let root = tree.path("package");
    sealed_fixture(&root);
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");
    let before = cache.capture(&root).expect("initial capture");

    unseal_source_tree(&root);
    fs::write(root.join("main.omg"), "data Main { value: u16; }\n").expect("edit source");
    seal_source_tree(&root);

    let after = cache.capture(&root).expect("post-edit capture");
    assert_eq!(after.outcome(), CheckedSourceCacheOutcome::Cold);
    assert!(after.record_stored());
    assert_ne!(
        before
            .canonical_source_metadata()
            .source_content_commitment(),
        after
            .canonical_source_metadata()
            .source_content_commitment(),
        "content edits produce a distinct checked-source commitment"
    );
}

#[cfg(unix)]
#[test]
fn a_new_member_invalidates_the_warm_record() {
    let tree = TempTree::new();
    let root = tree.path("package");
    sealed_fixture(&root);
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");
    cache.capture(&root).expect("initial capture");

    unseal_source_tree(&root);
    fs::write(root.join("added.omg"), "data Added { }\n").expect("add a member");
    seal_source_tree(&root);

    let capture = cache.capture(&root).expect("post-addition capture");
    assert_eq!(capture.outcome(), CheckedSourceCacheOutcome::Cold);
    let expected = capture_package_source_input(&root).expect("authoritative capture");
    assert_eq!(
        capture.canonical_source_metadata(),
        expected.canonical_source_metadata()
    );
}

#[test]
fn a_corrupt_record_is_a_miss_and_is_removed() {
    let tree = TempTree::new();
    let root = tree.path("package");
    sealed_fixture(&root);
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");
    cache.capture(&root).expect("initial capture");
    let record = fs::read_dir(&cache_dir)
        .expect("enumerate cache")
        .next()
        .expect("one record")
        .expect("record entry")
        .path();
    let mut bytes = fs::read(&record).expect("read record");
    let payload_byte = 16;
    bytes[payload_byte] ^= 0xff;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&record, fs::Permissions::from_mode(0o644))
            .expect("reopen record for corruption");
    }
    fs::write(&record, &bytes).expect("corrupt record");

    let capture = cache.capture(&root).expect("capture after corruption");
    assert_eq!(capture.outcome(), CheckedSourceCacheOutcome::Cold);
    assert!(capture.record_stored());
    // The corrupt record was discarded and a verified one published in its
    // place: the next capture is a warm hit on the rebuilt record, not on
    // the poisoned bytes.
    let replayed = cache.capture(&root).expect("capture after republish");
    assert_eq!(replayed.outcome(), CheckedSourceCacheOutcome::Warm);
    assert_eq!(
        capture.canonical_source_metadata(),
        replayed.canonical_source_metadata()
    );
}

#[test]
fn a_full_cache_declares_no_new_records() {
    let tree = TempTree::new();
    let root = tree.path("package");
    sealed_fixture(&root);
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_with_limits(
        &cache_dir,
        CheckedSourceCacheLimits {
            max_records: 0,
            ..CheckedSourceCacheLimits::default()
        },
    )
    .expect("open bounded cache");
    let capture = cache.capture(&root).expect("capture against full cache");
    assert_eq!(capture.outcome(), CheckedSourceCacheOutcome::Cold);
    assert!(!capture.record_stored());
    // A declined store leaves the index itself intact — the cache never
    // reports a warm hit it cannot verify.
    let again = cache.capture(&root).expect("repeat capture");
    assert_eq!(again.outcome(), CheckedSourceCacheOutcome::Cold);
}

#[cfg(unix)]
#[test]
fn a_root_with_noncanonical_mode_rejects() {
    let tree = TempTree::new();
    let root = tree.path("package");
    fs::write(root.join("main.omg"), "data Main { value: u8; }\n").expect("write main");
    // The fixture remains world-writable: the capture must enforce the
    // canonical sealed modes before any record is consulted.
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");
    let error = cache.capture(&root).expect_err("unsealed root rejects");
    assert!(error.contains("noncanonical mode"), "{error}");
}

#[test]
fn a_missing_root_rejects() {
    let tree = TempTree::new();
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");
    let absent = tree.0.join("absent");
    let error = cache.capture(&absent).expect_err("absent root rejects");
    assert!(error.contains("cannot inspect source root"), "{error}");
}

#[test]
fn separate_roots_own_disjoint_records() {
    let tree = TempTree::new();
    let first = tree.path("first");
    let second = tree.path("second");
    sealed_fixture(&first);
    sealed_fixture(&second);
    let cache_dir = tree.path("cache");
    let cache = CheckedSourceCache::open_or_create(&cache_dir).expect("open cache");

    let first_cold = cache.capture(&first).expect("first cold capture");
    let second_cold = cache.capture(&second).expect("second cold capture");
    assert_eq!(
        2,
        fs::read_dir(&cache_dir).expect("enumerate cache").count()
    );
    let first_warm = cache.capture(&first).expect("first warm capture");
    let second_warm = cache.capture(&second).expect("second warm capture");
    assert_eq!(first_warm.outcome(), CheckedSourceCacheOutcome::Warm);
    assert_eq!(second_warm.outcome(), CheckedSourceCacheOutcome::Warm);
    assert_eq!(
        first_warm.canonical_source_metadata(),
        first_cold.canonical_source_metadata()
    );
    assert_eq!(
        second_warm.canonical_source_metadata(),
        second_cold.canonical_source_metadata()
    );
}
