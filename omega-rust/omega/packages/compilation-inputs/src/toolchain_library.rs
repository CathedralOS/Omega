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
    if let Some(recorded) = &recorded
        && recorded.join("core").is_dir()
    {
        return Some(recorded.clone());
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

/// Identify the selected library's current source bytes, including injected core
/// modules absent from a project's package graph. Do not cache this with the
/// selected path: a development library may have changed since the prior use.
pub fn identity() -> Result<String, String> {
    library_digest(&root()?)
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
