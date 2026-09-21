//! Files that import their own crate or parent module with a glob only ever
//! get fewer.
//!
//! `use super::*;` and `use crate::*;` turn a parent module into a hidden
//! namespace: a reader following a call from an entrance into a leaf cannot
//! see where a name comes from without a repository-wide search, which is the
//! navigation failure the source-organization audits exist to prevent. The
//! repository once held more than two thousand such files; the residual is
//! small enough to repair file by file, and this ratchet keeps it shrinking.
//! It records, per crate, how many files carry a glob self-import today and
//! fails when that number grows.
//!
//! An entry records the exact count its crate has right now. Growing past it
//! fails as a regression; shrinking fails as a stale entry, and the entry is
//! lowered or deleted. The list moves one direction. Writing a new file with
//! explicit imports is always allowed; writing one with `use super::*;` is
//! allowed only by removing the glob from another file in the same crate.

use std::fs;
use std::path::{Path, PathBuf};

/// Exact no-growth ratchets: (crate directory, files carrying a glob
/// self-import anywhere under it: `src/`, `tests/`, `examples/`, `benches/`).
const GLOB_SELF_IMPORT_CEILINGS: &[(&str, usize)] = &[];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

/// Whether one line is a glob import of the file's own crate or an ancestor
/// module: `use super::*;`, `use super::super::*;`, `use crate::*;`, with any
/// `pub` qualifier in front.
fn is_glob_self_import(line: &str) -> bool {
    let line = line.trim();
    let Some(rest) = line
        .strip_prefix("pub(crate) use ")
        .or_else(|| line.strip_prefix("pub(super) use "))
        .or_else(|| line.strip_prefix("pub use "))
        .or_else(|| line.strip_prefix("use "))
    else {
        return false;
    };
    let Some(path) = rest.strip_suffix("*;") else {
        return false;
    };
    let mut segments = path.split("::").filter(|segment| !segment.is_empty());
    let Some(first) = segments.next() else {
        return false;
    };
    (first == "crate" || first == "super") && segments.all(|segment| segment == "super")
}

fn file_carries_glob_self_import(path: &Path) -> bool {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    source.lines().any(is_glob_self_import)
}

fn count_glob_self_import_files(directory: &Path) -> usize {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    let mut count = 0;
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read entry under {}: {error}", directory.display()))
            .path();
        if path.is_dir() {
            let nested_crate = path.join("Cargo.toml").is_file();
            if nested_crate || path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            count += count_glob_self_import_files(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && file_carries_glob_self_import(&path)
        {
            count += 1;
        }
    }
    count
}

/// Every crate directory under `omega-rust/`, `tests/`, or `tools/` that has
/// a `Cargo.toml` and a `src/` tree, as workspace-relative paths.
fn crate_directories(root: &Path) -> Vec<String> {
    let mut crates = Vec::new();
    for tree in ["omega-rust", "tests", "tools"] {
        collect_crate_directories(root, &root.join(tree), &mut crates);
    }
    crates.sort();
    crates
}

fn collect_crate_directories(root: &Path, directory: &Path, crates: &mut Vec<String>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read entry under {}: {error}", directory.display()))
            .path();
        if !path.is_dir() || path.file_name().is_some_and(|name| name == "target") {
            continue;
        }
        if path.join("Cargo.toml").is_file() && path.join("src").is_dir() {
            crates.push(
                path.strip_prefix(root)
                    .expect("crate lives beneath the workspace root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
        collect_crate_directories(root, &path, crates);
    }
}

#[test]
fn glob_self_imports_never_grow_per_crate() {
    let root = workspace_root();
    let mut regressions = Vec::new();
    for crate_directory in crate_directories(&root) {
        let count = count_glob_self_import_files(&root.join(&crate_directory));
        let ceiling = GLOB_SELF_IMPORT_CEILINGS
            .iter()
            .find_map(|(directory, ceiling)| (*directory == crate_directory).then_some(*ceiling))
            .unwrap_or(0);
        if count > ceiling {
            regressions.push(format!(
                "{crate_directory}: {count} files, ceiling {ceiling}"
            ));
        }
    }
    assert!(
        regressions.is_empty(),
        "files importing their own crate or parent module with a glob grew; import the names \
         the file uses instead of raising the ceiling:\n{}",
        regressions.join("\n")
    );
}

#[test]
fn glob_self_import_ceilings_are_still_exact() {
    let root = workspace_root();
    let crates = crate_directories(&root);
    let mut stale = Vec::new();
    for (crate_directory, ceiling) in GLOB_SELF_IMPORT_CEILINGS {
        if !crates.iter().any(|directory| directory == crate_directory) {
            stale.push(format!(
                "{crate_directory}: crate no longer exists; delete the entry"
            ));
            continue;
        }
        let count = count_glob_self_import_files(&root.join(crate_directory));
        if count < *ceiling {
            stale.push(format!(
                "{crate_directory}: {count} files, entry says {ceiling}; lower it (or delete it at zero)"
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "glob self-import ceilings must track the tree exactly so the list only moves down:\n{}",
        stale.join("\n")
    );
}

#[test]
fn glob_self_import_detection_matches_the_documented_forms() {
    for line in [
        "use super::*;",
        "    use super::*;",
        "use super::super::*;",
        "use crate::*;",
        "pub(crate) use super::*;",
        "pub use crate::*;",
    ] {
        assert!(is_glob_self_import(line), "{line}");
    }
    for line in [
        "use super::shared::*;",
        "use crate::model::*;",
        "use std::collections::*;",
        "use super::{A, B};",
        "pub use model::*;",
        "// use super::*;",
    ] {
        assert!(!is_glob_self_import(line), "{line}");
    }
}
