// Toolchain library root discovery and release content-digest verification (`configure`, `root`, `identity`).
//
// Locates the compiler's own bundled `source/library` (`core`, `system`, etc.), never a
// requester-local substitute. Release builds enforce `OMEGA_RELEASE_LIBRARY_SHA256`
// with no checkout fallback. Development builds prefer `OMEGA_DEVELOPMENT_LIBRARY_ROOT`
// while it still holds `core/`, and otherwise recover `source/library` by walking
// the running executable's ancestors so moved or secondary worktrees do not fail on
// a stale compile-time checkout path. `identity()` re-hashes the library on demand
// (the complete frozen source-tree digest) so edits to a development library are never masked
// by the cached path in `LIBRARY`.

use source_acquisition::LocalSourceLimits;
use source_acquisition::local::resolution_observations::CapturedLocalSource;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const PACKAGE_CATALOG: &str = "packages.tsv";

static LIBRARY: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Selects the process's toolchain library. Several callers in one process
/// (parallel tests, for example) may select the same library; only a request
/// for a different library, or one after discovery failed, is refused.
pub fn configure(root: &Path) -> Result<(), String> {
    let root = validate(root, option_env!("OMEGA_RELEASE_LIBRARY_SHA256"))?;
    select(&LIBRARY, root)
}

fn select(library: &OnceLock<Result<PathBuf, String>>, root: PathBuf) -> Result<(), String> {
    match library.get_or_init(|| Ok(root.clone())) {
        Ok(selected) if *selected == root => Ok(()),
        Ok(selected) => Err(format!(
            "toolchain library {} was already selected; cannot select {}",
            selected.display(),
            root.display()
        )),
        Err(error) => Err(format!(
            "toolchain library discovery already failed: {error}"
        )),
    }
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
    if !root.join("core").is_dir() {
        return Err(format!(
            "Omega toolchain library at {} is missing its core directory",
            root.display()
        ));
    }
    if expected.is_some() {
        capture(&root, expected)?;
    }
    Ok(root)
}

pub fn identity() -> Result<String, String> {
    Ok(snapshot()?.normalized().content_identity.clone())
}

/// Package lookup and the source bytes returned to its caller share this capture.
pub fn snapshot() -> Result<CapturedLocalSource, String> {
    capture(&root()?, option_env!("OMEGA_RELEASE_LIBRARY_SHA256"))
}

fn capture(root: &Path, expected: Option<&str>) -> Result<CapturedLocalSource, String> {
    let capture = source_acquisition::local::operations::capture_local_source_input(
        root,
        LocalSourceLimits::default(),
    )
    .map_err(|error| error.to_string())?;
    if let Some((path, _)) = capture
        .tree()
        .entries()
        .find(|(_, node)| matches!(node, source_capture::CapturedSourceNode::Symlink { .. }))
    {
        return Err(format!(
            "toolchain library contains a symbolic link: {}",
            path.display()
        ));
    }
    let actual = &capture.normalized().content_identity;
    if let Some(expected) = expected
        && actual != expected
    {
        return Err(format!(
            "Omega toolchain library does not match this compiler: expected {expected}, found {actual}; reinstall the complete bundle"
        ));
    }
    Ok(capture)
}

#[cfg(test)]
mod tests {
    use super::{capture, select, validate};
    use platform_custody::temporary_directory::TemporaryDirectory;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    #[test]
    fn discovery_requires_core_and_verifies_the_components_the_distribution_shipped() {
        let directory = TemporaryDirectory::create("library-discovery", false).unwrap();
        let root = directory.path().canonicalize().unwrap();
        assert!(
            validate(&root, None)
                .unwrap_err()
                .contains("missing its core")
        );

        fs::create_dir(root.join("core")).unwrap();
        fs::write(root.join("core/main.omg"), b"public data Word {}").unwrap();
        let core_only = capture(&root, None)
            .unwrap()
            .normalized()
            .content_identity
            .clone();
        assert_eq!(validate(&root, None).unwrap(), root);
        assert_eq!(validate(&root, Some(&core_only)).unwrap(), root);

        fs::create_dir(root.join("system")).unwrap();
        let service = root.join("system/build.omg");
        fs::write(&service, b"machine build(builder: @mutable Build) {}").unwrap();
        let with_system = capture(&root, None)
            .unwrap()
            .normalized()
            .content_identity
            .clone();
        assert_eq!(validate(&root, Some(&with_system)).unwrap(), root);
        fs::remove_file(service).unwrap();
        let refusal = validate(&root, Some(&with_system)).unwrap_err();
        assert!(
            refusal.contains("does not match this compiler"),
            "{refusal}"
        );
    }

    #[test]
    fn bundle_identity_covers_the_catalog_and_bytes_visible_to_package_builds() {
        let directory = TemporaryDirectory::create("bundled-catalog", false).unwrap();
        let root = directory.path();
        fs::create_dir(root.join("core")).unwrap();
        fs::create_dir(root.join("part")).unwrap();
        fs::write(root.join(super::PACKAGE_CATALOG), "answers\tpart\n").unwrap();
        fs::write(root.join("part/input.bin"), [4, 7]).unwrap();
        let selected = capture(root, None).unwrap();
        let digest = &selected.normalized().content_identity;
        fs::write(root.join("part/input.bin"), [9, 7]).unwrap();
        assert!(capture(root, Some(digest)).is_err());
        fs::write(root.join("part/input.bin"), [4, 7]).unwrap();
        fs::write(root.join(super::PACKAGE_CATALOG), "alias\tpart\n").unwrap();
        assert!(capture(root, Some(digest)).is_err());
        let member = selected
            .workspace_member(
                std::path::Path::new("part"),
                &source_acquisition::SourceCaptureRequest::entire_tree(),
            )
            .unwrap();
        assert_eq!(
            member
                .tree()
                .read_to_string(std::path::Path::new("input.bin"))
                .unwrap()
                .as_bytes(),
            &[4, 7]
        );
    }

    #[test]
    fn selecting_the_same_library_twice_succeeds_and_a_different_one_is_refused() {
        let library = OnceLock::new();
        let first = PathBuf::from("/omega/source/library");
        assert_eq!(select(&library, first.clone()), Ok(()));
        assert_eq!(select(&library, first), Ok(()));
        let refusal = select(&library, PathBuf::from("/elsewhere/library")).unwrap_err();
        assert!(refusal.contains("was already selected"), "{refusal}");
    }

    #[test]
    fn selecting_after_failed_discovery_is_refused() {
        let library = OnceLock::new();
        library.set(Err("missing".to_owned())).unwrap();
        let refusal = select(&library, PathBuf::from("/omega/source/library")).unwrap_err();
        assert!(refusal.contains("missing"), "{refusal}");
    }
}
