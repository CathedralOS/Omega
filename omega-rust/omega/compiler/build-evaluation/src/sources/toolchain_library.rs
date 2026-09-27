//! Locate the toolchain's own library, never a requester-local substitute.
//!
//! Releases carry a library digest and no checkout fallback. Development builds
//! retain their source tree. Embedders can configure an absolute library root
//! before first use; OMEGA_LIBRARY_ROOT supplies the same explicit process-level
//! choice for command-line use. Neither bypasses a release's content check.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static LIBRARY: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Set the process's toolchain library before any compilation or lookup.
pub fn configure(root: &Path) -> Result<(), String> {
    let root = validate(root, option_env!("OMEGA_RELEASE_LIBRARY_SHA256"))?;
    LIBRARY
        .set(Ok(root))
        .map_err(|_| "toolchain library was already selected".to_owned())
}

pub fn root() -> Result<PathBuf, String> {
    LIBRARY
        .get_or_init(|| {
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            let executable = executable
                .canonicalize()
                .map_err(|error| error.to_string())?;
            let root = select_root(
                &executable,
                std::env::var_os("OMEGA_LIBRARY_ROOT").map(PathBuf::from),
                if option_env!("OMEGA_RELEASE_LIBRARY_SHA256").is_some() {
                    None
                } else {
                    recover_development_root(
                        &executable,
                        option_env!("OMEGA_DEVELOPMENT_LIBRARY_ROOT").map(PathBuf::from),
                    )
                },
            )?;
            validate(&root, option_env!("OMEGA_RELEASE_LIBRARY_SHA256"))
        })
        .clone()
}

/// The development library root to offer, recovering one the recorded path lost.
///
/// `build.rs` bakes an absolute `source/library` path from the checkout that
/// compiled the binary, and asks Cargo to rerun only when the release digest
/// changes. A binary therefore keeps naming its original checkout after that
/// tree is moved or deleted, and every compilation fails on a path the current
/// checkout never had. Prefer the recorded root while it still holds a library,
/// then fall back to the tree this executable actually sits in, and finally to
/// the recorded root so its absence is reported as before.
///
/// Release builds never reach this: their caller passes no development root,
/// keeping "a release carries a digest and no checkout fallback" intact.
fn recover_development_root(executable: &Path, recorded: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(recorded) = &recorded {
        if recorded.join("core").is_dir() {
            return Some(recorded.clone());
        }
    }
    executable
        .ancestors()
        .map(|ancestor| ancestor.join("source/library"))
        .find(|candidate| candidate.join("core").is_dir())
        .or(recorded)
}

fn select_root(
    executable: &Path,
    explicit: Option<PathBuf>,
    development: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(root) = explicit {
        if !root.is_absolute() {
            return Err("OMEGA_LIBRARY_ROOT must be an absolute path".to_owned());
        }
        return Ok(root);
    }
    let installation = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "cannot determine Omega installation from executable path".to_owned())?;
    let installed = installation.join("lib/omega/library");
    if installed.exists() || installation.join("manifest.json").exists() {
        return Ok(installed);
    }
    Ok(development.unwrap_or(installed))
}

fn validate(root: &Path, expected: Option<&str>) -> Result<PathBuf, String> {
    if !root.is_absolute() {
        return Err("toolchain library root must be absolute".to_owned());
    }
    let root = root.canonicalize().map_err(|error| format!(
        "missing Omega toolchain library at {}: {error}; reinstall the complete bundle or set OMEGA_LIBRARY_ROOT", root.display()))?;
    if !root.join("core").is_dir() || !root.join("std/build.omg").is_file() {
        return Err(format!(
            "incomplete Omega toolchain library at {}",
            root.display()
        ));
    }
    if let Some(expected) = expected {
        let actual = library_digest(&root)?;
        if actual != expected {
            return Err(format!(
                "Omega toolchain library does not match this compiler: expected {expected}, found {actual}; reinstall the complete bundle"
            ));
        }
    }
    Ok(root)
}

// The packager uses the same versioned framing: sorted UTF-8 relative paths,
// each prefixed by its u64 byte length, then file length and exact file bytes.
fn library_digest(root: &Path) -> Result<String, String> {
    fn collect(root: &Path, directory: &Path, files: &mut Vec<String>) -> Result<(), String> {
        for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_symlink() {
                return Err(format!(
                    "toolchain library contains a symbolic link: {}",
                    entry.path().display()
                ));
            }
            if kind.is_dir() {
                collect(root, &entry.path(), files)?;
            } else if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "omg")
            {
                let path = entry.path();
                let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
                files.push(
                    relative
                        .to_str()
                        .ok_or("non-UTF-8 toolchain library path")?
                        .replace('\\', "/"),
                );
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    digest.update(b"omega-toolchain-library-v1\0");
    for relative in files {
        let contents = std::fs::read(root.join(&relative)).map_err(|error| error.to_string())?;
        digest.update((relative.len() as u64).to_le_bytes());
        digest.update(relative.as_bytes());
        digest.update((contents.len() as u64).to_le_bytes());
        digest.update(contents);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LibraryFixture(PathBuf);

    impl LibraryFixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let identity = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "omega-library-{}-{identity}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(root.join("core")).unwrap();
            std::fs::create_dir_all(root.join("std")).unwrap();
            std::fs::write(root.join("core/a.omg"), b"data A {}\n").unwrap();
            std::fs::write(root.join("std/build.omg"), b"machine build() {}\n").unwrap();
            Self(root)
        }
    }

    impl Drop for LibraryFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn digest_matches_packager_and_rejects_modified_library() {
        let fixture = LibraryFixture::new();
        let expected = "fb0cd4651037cc6a991c017ff861bcf92c050498945c49a286f887dbb1fd9541";
        assert_eq!(library_digest(&fixture.0).unwrap(), expected);
        assert!(validate(&fixture.0, Some(expected)).is_ok());
        std::fs::write(fixture.0.join("core/a.omg"), b"data B {}\n").unwrap();
        assert!(
            validate(&fixture.0, Some(expected))
                .unwrap_err()
                .contains("does not match")
        );
    }

    #[test]
    fn release_has_no_development_fallback_and_explicit_paths_are_absolute() {
        let fixture = LibraryFixture::new();
        let executable = fixture.0.join("bin/omega");
        let installed = fixture.0.join("lib/omega/library");
        assert_eq!(select_root(&executable, None, None).unwrap(), installed);
        assert!(validate(&installed, None).is_err());
        assert!(select_root(&executable, Some(PathBuf::from("relative")), None).is_err());
        assert_eq!(
            select_root(&executable, Some(fixture.0.clone()), None).unwrap(),
            fixture.0
        );
    }

    #[test]
    fn installed_bundle_takes_precedence_over_development_tree() {
        let fixture = LibraryFixture::new();
        std::fs::write(fixture.0.join("manifest.json"), "{}").unwrap();
        let selected =
            select_root(&fixture.0.join("bin/omega"), None, Some(fixture.0.clone())).unwrap();
        assert_eq!(selected, fixture.0.join("lib/omega/library"));
        assert!(validate(&selected, None).is_err());
    }

    #[test]
    fn a_recorded_development_root_that_vanished_recovers_from_the_executable_tree() {
        let checkout = LibraryFixture::new();
        let library = checkout.0.join("source/library");
        std::fs::create_dir_all(library.join("core")).unwrap();
        let executable = checkout.0.join("target/debug/omega");
        let vanished = checkout.0.join("worktrees/removed/source/library");

        assert_eq!(
            recover_development_root(&executable, Some(vanished.clone())).unwrap(),
            library
        );
        assert_eq!(
            recover_development_root(&executable, Some(checkout.0.clone())).unwrap(),
            checkout.0
        );

        let outside = std::env::temp_dir().join("omega-absent-checkout/target/debug/omega");
        assert_eq!(
            recover_development_root(&outside, Some(vanished.clone())),
            Some(vanished)
        );
    }
}
