//! Sealed source staging: the test harness's stand-in for package acquisition.
//!
//! A real `omega` invocation never compiles a package out of the working tree.
//! Acquisition resolves the declared source, publishes an immutable snapshot
//! into its cache lane, and seals that snapshot read-only (`0o555`
//! directories, `0o444` files, `0o555` executables) before any compilation
//! reads it. The compiler relies on that: a package-aware build activation
//! captures compiler-validated canonical Source metadata from the physical
//! root, and the capture admits only those canonical modes, because a root
//! that is still writable cannot commit the bytes a build's `Source` reads.
//!
//! Tests that compile repository fixtures skip acquisition, so they would bind
//! the root package to the checked-out fixture directory, whose mode is an
//! ordinary writable `0o755`. Such a compilation has no canonical Source
//! metadata, and the harness, not the rule under test, would refuse any fixture
//! whose build machine reached the build filesystem. This module stages a sealed
//! private snapshot of the fixture's directory and binds the root package to
//! it, so the harness supplies the custody the compiler demands instead of the
//! compiler relaxing the demand.
//!
//! Two properties of the staged layout matter:
//!
//! - A snapshot keeps the fixture's repository-relative path under one private
//!   session root (`<session>/tests/fixtures/fail/build/<name>/`). Diagnostics
//!   show positions with the path they were compiled from, and the test
//!   sanitizer strips a known root prefix to stay checkout-portable. Because the
//!   relative path is mirrored, stripping the session root yields the text
//!   stripping the repository root yielded, so no pinned diagnostic changes
//!   because a fixture is now compiled out of a snapshot.
//! - Sealing happens in a private copy, never in place. Fixture directories
//!   are shared: several test runs use one checkout at once and an editor may
//!   hold a file open, so making the working tree read-only during a compile
//!   would let one run's restore unseal another run's source. The session root
//!   is created exclusively, so even processes with the same id in separate
//!   PID namespaces cannot share it.
//!
//! Dependencies are deliberately not staged for the root's compilation. A
//! fixture resolves `Source::Path` locations against its authored project
//! (fixtures reach the bundled standard library through
//! `../../../../../source/library/std`), so those rows are still read and
//! resolved from the authored directory; only the root package's binding moves
//! to the snapshot. Nested members keep their existing staging. A dependency
//! whose own Build runs is that Build's root, so it is sealed the same way,
//! once per process, separately from fixtures.

use std::fs;
use std::path::{Path, PathBuf};

/// The exclusively owned root for this process's snapshots and working directories.
///
/// Process ids are not unique when separate PID namespaces share a temporary
/// directory. Creating a candidate atomically, rather than accepting an existing
/// directory, makes the filesystem establish ownership; occupied candidates are
/// left untouched, including directories retained after a crashed runner.
pub fn temporary_session_root() -> &'static Path {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let temporary = std::env::temp_dir();
        for sequence in 0_u64.. {
            let root = temporary.join(format!(
                "omega-corpus-session-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return fs::canonicalize(&root).unwrap_or(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "fixture staging session root {} could not be created: {error}",
                    root.display()
                ),
            }
        }
        unreachable!("fixture staging session sequence exhausted")
    })
    .as_path()
}

/// The private directory every staged snapshot lands under.
///
/// Canonicalized once: a bound source root is canonicalized by package
/// construction, and the sanitizer strips this prefix from rendered
/// diagnostics, so both must see the same spelling. On macOS the host's
/// temporary directory is itself reached through a symlink, which would
/// otherwise make the two disagree.
pub fn staging_session_root() -> &'static Path {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let root = temporary_session_root().join("source");
        fs::create_dir_all(&root).unwrap_or_else(|error| {
            panic!(
                "fixture source root {} could not be created: {error}",
                root.display()
            )
        });
        fs::canonicalize(&root).unwrap_or(root)
    })
    .as_path()
}

/// A fresh writable staging directory for one fixture's compilation.
///
/// A compilation that names no staging directory defaults to a `build`
/// directory beside its root source. That default is now inside the sealed
/// snapshot, which admits no writes at all, and before staging it was inside
/// the checkout, where a test may not write either; both only ever worked
/// because no fixture's build machine had reached the build filesystem. Every
/// compilation this harness starts therefore names its staging explicitly.
///
/// It lives outside [`staging_session_root`] on purpose: that root is stripped
/// out of recorded diagnostics, and a staging path underneath it would be
/// rewritten into something that reads like a repository-relative path.
pub fn fresh_staging_build_dir(repository_root: &Path, authored_root: &Path) -> PathBuf {
    let staging = temporary_session_root()
        .join("fixture-build")
        .join(relative_fixture_path(repository_root, authored_root));
    fs::create_dir_all(&staging).expect("fixture staging parent");
    // Separate Product legs can retain overlapping source lifetimes. Starting
    // one must never dispose another leg's still-live package session.
    for sequence in 0_u64.. {
        let directory = staging.join(sequence.to_string());
        match fs::create_dir(&directory) {
            Ok(()) => return directory,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!(
                "cannot reserve fixture staging {}: {error}",
                directory.display()
            ),
        }
    }
    unreachable!("fixture staging sequence exhausted")
}

/// Stage `authored_root` as a sealed snapshot at the same path relative to
/// `repository_root`, and answer the snapshot's root directory.
///
/// The snapshot is rebuilt on every call rather than reused: a fixture's
/// authored bytes are what the compilation under test must read, and a
/// retained copy would answer for an edit made since.
pub fn stage_sealed_snapshot(repository_root: &Path, authored_root: &Path) -> PathBuf {
    let staged = staging_session_root().join(relative_fixture_path(repository_root, authored_root));
    discard_snapshot(&staged);
    if let Some(parent) = staged.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!(
                "fixture snapshot parent {} could not be created: {error}",
                parent.display()
            )
        });
    }
    copy_tree(authored_root, &staged).unwrap_or_else(|error| {
        panic!(
            "fixture {} could not be staged at {}: {error}",
            authored_root.display(),
            staged.display()
        )
    });
    seal_tree(&staged).unwrap_or_else(|error| {
        panic!(
            "fixture snapshot {} could not be sealed: {error}",
            staged.display()
        )
    });
    staged
}

/// The private directory every staged dependency snapshot lands under,
/// canonicalized for the same reason as [`staging_session_root`].
///
/// A dependency is staged apart from fixture roots because a dependency
/// directory may sit inside another fixture (`pass/targets/.../kit`) whose
/// own snapshot is discarded and restaged while a concurrent worker reads the
/// dependency. The runner strips this root from recorded diagnostics too.
pub fn dependency_staging_session_root() -> &'static Path {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let root = temporary_session_root().join("dependency-source");
        fs::create_dir_all(&root).unwrap_or_else(|error| {
            panic!(
                "dependency source root {} could not be created: {error}",
                root.display()
            )
        });
        fs::canonicalize(&root).unwrap_or(root)
    })
    .as_path()
}

/// Stage a dependency the way acquisition seals it into its cache lane, at
/// its repository-relative path under [`dependency_staging_session_root`],
/// and answer the snapshot's root directory.
///
/// A dependency Build occurrence is a package-aware build activation rooted
/// at that dependency, so it needs the same canonical Source metadata a
/// fixture root does. Unlike a fixture root, one dependency (the bundled
/// standard library above all) serves many fixtures at once, so it is staged
/// once per process and then only read: restaging it per fixture would
/// discard a snapshot a concurrent worker is compiling.
pub fn stage_sealed_dependency_snapshot(repository_root: &Path, authored_root: &Path) -> PathBuf {
    static STAGED: std::sync::Mutex<Vec<(PathBuf, PathBuf)>> = std::sync::Mutex::new(Vec::new());
    let mut staged = STAGED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, snapshot)) = staged
        .iter()
        .find(|(authored, _)| authored == authored_root)
    {
        return snapshot.clone();
    }
    let snapshot = dependency_staging_session_root()
        .join(relative_fixture_path(repository_root, authored_root));
    discard_snapshot(&snapshot);
    if let Some(parent) = snapshot.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!(
                "dependency snapshot parent {} could not be created: {error}",
                parent.display()
            )
        });
    }
    copy_tree(authored_root, &snapshot).unwrap_or_else(|error| {
        panic!(
            "dependency {} could not be staged at {}: {error}",
            authored_root.display(),
            snapshot.display()
        )
    });
    seal_tree(&snapshot).unwrap_or_else(|error| {
        panic!(
            "dependency snapshot {} could not be sealed: {error}",
            snapshot.display()
        )
    });
    staged.push((authored_root.to_path_buf(), snapshot.clone()));
    snapshot
}

/// Release this process's snapshots and working directories.
///
/// Called once the last record is emitted, not after each fixture. A detached
/// compile may still read, write or remove paths beneath this root. Keep its
/// reservation until the process exits: releasing it early would let another
/// same-PID session reuse the path before the old compile's cleanup finishes.
/// The caller's count must cover every remaining staging access, including
/// diagnostic rendering after a caught compiler panic.
pub fn discard_staged_snapshots(still_live: usize) {
    if still_live == 0 {
        discard_snapshot(temporary_session_root());
    }
}

/// Where a fixture sits inside the repository: the path both a snapshot and a
/// staging directory are keyed by, so one fixture's two private directories
/// are named the same way and a concurrent worker can never collide with it.
fn relative_fixture_path(repository_root: &Path, authored_root: &Path) -> PathBuf {
    // Dependency acquisition canonicalizes its roots. Compare both physical
    // roots in that same namespace, including Windows verbatim prefixes and
    // symlink resolution, before deriving a private staging location.
    let repository_root = fs::canonicalize(repository_root).unwrap_or_else(|error| {
        panic!(
            "repository root {} cannot be resolved: {error}",
            repository_root.display()
        )
    });
    let authored_root = fs::canonicalize(authored_root).unwrap_or_else(|error| {
        panic!(
            "fixture root {} cannot be resolved: {error}",
            authored_root.display()
        )
    });
    authored_root
        .strip_prefix(&repository_root)
        .unwrap_or_else(|_| {
            panic!(
                "fixture root {} is not inside the repository at {}",
                authored_root.display(),
                repository_root.display()
            )
        })
        .to_path_buf()
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Apply the canonical snapshot modes bottom-up: a directory loses the write
/// permission its own children were created through, so its contents are
/// sealed first.
#[cfg(unix)]
fn seal_tree(root: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            seal_tree(&path)?;
            continue;
        }
        // Acquisition commits whether a member is executable, and the
        // capture reads that distinction back out of the mode, so the
        // authored bit decides between the two canonical file modes.
        let executable = fs::symlink_metadata(&path)?.permissions().mode() & 0o111 != 0;
        let mode = if executable { 0o555 } else { 0o444 };
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o555))
}

/// Windows carries no canonical-mode contract: the capture's mode checks are
/// Unix-only, so a copied tree is already the snapshot the compiler admits.
#[cfg(not(unix))]
fn seal_tree(_root: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Remove a staged tree, restoring the write permission on each directory
/// first: a sealed directory admits no removal of its own entries.
fn discard_snapshot(root: &Path) {
    if !root.exists() {
        return;
    }
    unseal_tree(root);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
fn unseal_tree(root: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = fs::set_permissions(root, fs::Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            unseal_tree(&entry.path());
        }
    }
}

#[cfg(not(unix))]
fn unseal_tree(_root: &Path) {}
