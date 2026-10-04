//! Scoped-path confinement: what `resolve_for_check` authorizes when the leaf
//! of an authored path is a symlink. These exercise the host filesystem
//! directly and carry no Omega source input.

use super::{canonical_grants, resolve_for_check};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

/// A real host directory, removed with the test. Confinement is a property of
/// physical paths, so these cases cannot run against the virtual provider.
struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let identity = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-confinement-{label}-{}-{identity}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create confinement test directory");
        Self(path)
    }

    /// The canonical form, which is what a grant root compares against:
    /// `/tmp` is itself a symlink on macOS.
    fn canonical(&self) -> PathBuf {
        self.0.canonicalize().expect("test directory resolves")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_root(identity: u32, path: PathBuf) -> crate::FsGrants {
    crate::FsGrants {
        read_roots: Vec::new(),
        write_roots: vec![crate::FilesystemGrantRoot::new(
            crate::FilesystemGrantRootIdentity::new(identity).expect("nonzero grant identity"),
            path,
        )],
    }
}

/// A scoped run that authorizes only a LINK NAME must not reach through it.
/// `real_symlink` admits `out/x -> <outside>/new` because it authorizes the
/// link name and its target need not exist, and the open then operates on the
/// path the grant check resolved. Resolution therefore has to report the
/// target: re-attaching the authored leaf would name `out/x`, match the write
/// root, and let `open(O_CREAT)` follow the link and create a file outside
/// every grant. `RealScoped` without a `FilesystemSponsor` -- the
/// `CheckedCompileRequest::new` default -- has no mirror to catch that, so the
/// refusal has to come from resolution itself.
#[test]
fn a_dangling_link_leaf_resolves_to_its_target_and_leaves_the_write_root() {
    let inside = TestDirectory::new("dangling-inside");
    let outside = TestDirectory::new("dangling-outside");
    let link = inside.0.join("x");
    let escape = outside.0.join("new");
    symlink(&escape, &link).expect("authorize only the link name");
    assert!(!escape.exists(), "the link target starts absent");

    let grants =
        canonical_grants(write_root(1, inside.0.clone())).expect("the write root resolves");

    // The two steps `authorized_native_path` composes for a follow-resolved
    // write. A `None` root is the refusal: the open is never reached.
    let resolved = resolve_for_check(&link).expect("the target's parent exists");
    assert_eq!(
        resolved,
        outside.canonical().join("new"),
        "resolution reports the location the open would actually reach"
    );
    assert!(
        grants.matching_root(&resolved, true).is_none(),
        "the link target is outside every grant"
    );
    assert!(
        !escape.exists(),
        "nothing is created outside the write root"
    );
}

/// The refusal above is a property of the TARGET, not of symlinks. A dangling
/// link whose target is inside the write root still resolves inside it and
/// still opens, which refusing every symlink leaf outright would have broken.
#[test]
fn a_dangling_link_leaf_inside_the_write_root_stays_authorized() {
    let inside = TestDirectory::new("dangling-in-root");
    let link = inside.0.join("x");
    symlink("y", &link).expect("authorize only the link name");

    let grants =
        canonical_grants(write_root(1, inside.0.clone())).expect("the write root resolves");

    let resolved = resolve_for_check(&link).expect("the link's own parent exists");
    assert_eq!(resolved, inside.canonical().join("y"));
    assert_eq!(
        grants
            .matching_root(&resolved, true)
            .map(|root| root.identity.get()),
        Some(1)
    );
}

/// A cycle among dangling links is unresolvable rather than an unbounded walk,
/// and an unresolvable path is the existing ENOENT lane, not an authorization.
#[test]
fn a_dangling_link_cycle_is_unresolvable() {
    let directory = TestDirectory::new("dangling-cycle");
    symlink("second", directory.0.join("first")).expect("link one");
    symlink("first", directory.0.join("second")).expect("link two");

    assert_eq!(resolve_for_check(&directory.0.join("first")), None);
}
