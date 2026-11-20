// Toolchain library root discovery and release content-digest verification (`configure`, `root`, `identity`).
//
// Locates the compiler's own bundled `source/library` (`core`, `std`, etc.), never a
// requester-local substitute. Release builds enforce `OMEGA_RELEASE_LIBRARY_SHA256`
// with no checkout fallback. Development builds prefer `OMEGA_DEVELOPMENT_LIBRARY_ROOT`
// while it still holds `core/`, and otherwise recover `source/library` by walking
// the running executable's ancestors so moved or secondary worktrees do not fail on
// a stale compile-time checkout path. `identity()` re-hashes the library on demand
// (`omega-toolchain-library-v1`) so edits to a development library are never masked
// by the cached path in `LIBRARY`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static LIBRARY: OnceLock<Result<PathBuf, String>> = OnceLock::new();

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

pub fn identity() -> Result<String, String> {
    library_digest(&root()?)
}

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
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-toolchain-library-v1\0");
    for relative in files {
        let contents = std::fs::read(root.join(&relative)).map_err(|error| error.to_string())?;
        digest.u64(relative.len() as u64);
        digest.raw(relative.as_bytes());
        digest.u64(contents.len() as u64);
        digest.raw(&contents);
    }
    Ok(crate::digest_encoder::hex(&digest.finish()))
}
