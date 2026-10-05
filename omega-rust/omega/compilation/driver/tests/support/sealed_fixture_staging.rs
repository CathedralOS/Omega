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
//! Test targets that compile repository fixtures bypassed acquisition and
//! bound the root package straight to the checked-out fixture directory, whose
//! mode is an ordinary writable `0o755`. Those compilations therefore had no
//! canonical Source metadata to offer, and any fixture whose build machine
//! reached the build filesystem was refused by the harness rather than by the
//! rule it meant to exercise. This module closes that gap: it stages a sealed
//! private snapshot of a fixture's directory and the binding is made against
//! the snapshot, so the harness supplies the custody the compiler demands
//! instead of the compiler relaxing the demand.
//!
//! Two properties of the staged layout are load-bearing:
//!
//! - A snapshot keeps the fixture's repository-relative path under one private
//!   session root (`<session>/tests/omega/fail/build/<name>/`). Diagnostics
//!   render positions with the path they were compiled from, and the corpus
//!   record's sanitizer strips a known root prefix to stay checkout-portable;
//!   mirroring the relative path means stripping the session root yields the
//!   exact text stripping the repository root yielded, so no recorded golden
//!   row moves because a fixture is now compiled out of a snapshot.
//! - Sealing happens in a private copy, never in place. Fixture directories
//!   are shared: several corpus runs execute against one checkout at once and
//!   an editor may hold a file open, so making the working tree read-only for
//!   the duration of a compile would let one run's restore unseal another
//!   run's source. The session root carries the process id for the same
//!   reason.
//!
//! Dependency rows are deliberately not staged here. A fixture resolves
//! `Source::Path` locations against its authored project (849 fixtures reach
//! the bundled standard library through `../../../../../source/library/std`),
//! so the authored directory stays the place those rows are read and resolved
//! from, and only the root package's binding moves to the snapshot. Nested
//! members keep their own existing staging.

use std::fs;
use std::path::{Path, PathBuf};

/// The private per-process directory every staged snapshot lands under.
///
/// Canonicalized once: a bound source root is canonicalized by package
/// construction, and the sanitizer strips this prefix from rendered
/// diagnostics, so both must see the same spelling. On macOS the host's
/// temporary directory is itself reached through a symlink, which would
/// otherwise make the two disagree.
pub fn staging_session_root() -> &'static Path {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let root =
            std::env::temp_dir().join(format!("omega-fixture-source-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap_or_else(|error| {
            panic!(
                "fixture staging session root {} could not be created: {error}",
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
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let root = ROOT.get_or_init(|| {
        std::env::temp_dir().join(format!("omega-fixture-staging-{}", std::process::id()))
    });
    let staging = root.join(relative_fixture_path(repository_root, authored_root));
    let _ = fs::remove_dir_all(&staging);
    staging
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

/// Release every snapshot this process staged.
///
/// Called once the last record is emitted, not after each fixture: a compile
/// that outlived its budget is detached rather than joined, and it may still
/// be reading the snapshot it was given. Its record is already discarded, so
/// what it reads no longer matters, but the directory must outlive it.
pub fn discard_staged_snapshots() {
    discard_snapshot(staging_session_root());
    let _ = fs::remove_dir_all(
        std::env::temp_dir().join(format!("omega-fixture-staging-{}", std::process::id())),
    );
}

/// Where a fixture sits inside the repository: the path both a snapshot and a
/// staging directory are keyed by, so one fixture's two private directories
/// are named the same way and a concurrent worker can never collide with it.
fn relative_fixture_path<'root>(repository_root: &Path, authored_root: &'root Path) -> &'root Path {
    authored_root
        .strip_prefix(repository_root)
        .unwrap_or_else(|_| {
            panic!(
                "fixture root {} is not inside the repository at {}",
                authored_root.display(),
                repository_root.display()
            )
        })
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
