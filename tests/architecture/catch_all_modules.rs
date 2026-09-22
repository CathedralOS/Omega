//! Modules named for what they are not (`shared`, `helpers`, `common`,
//! `util`, `prelude`, ...) only ever get fewer.
//!
//! A module named `shared.rs` or `prelude.rs` that re-exports a pile of
//! `pub(super) use` lines is a forwarding staircase: every sibling starts with
//! `use super::shared::*;` and a reader following a name from an entrance
//! into a leaf cannot see where it comes from without opening the hub. A
//! `helpers.rs` is the same failure for functions: the name says nothing
//! about the responsibility, so the file collects whatever had no owner.
//! AGENTS.md names these catch-alls as the shapes discoverability
//! architecture refuses; this ratchet records, per crate, how many
//! production source files still carry one of the names and fails when the
//! number grows.
//!
//! An entry records the exact count its crate has right now. Growing past it
//! fails as a regression; shrinking fails as a stale entry, and the entry is
//! lowered or deleted. Test trees (`src/tests/`, `tests/`) are not counted:
//! a test prelude is a fixture, not a route.

use std::fs;
use std::path::{Path, PathBuf};

/// File names that describe no responsibility. `model.rs` has its own
/// refusal in `entrypoint_module_layout`.
const CATCH_ALL_NAMES: &[&str] = &[
    "shared.rs",
    "helpers.rs",
    "helper.rs",
    "common.rs",
    "util.rs",
    "utils.rs",
    "misc.rs",
    "prelude.rs",
];

/// Exact no-growth ratchets: (crate directory, production source files under
/// its `src/` tree whose name is one of [`CATCH_ALL_NAMES`]).
const CATCH_ALL_MODULE_CEILINGS: &[(&str, usize)] = &[
    ("omega-rust/omega/backend/machine-emission", 1),
    ("omega-rust/psi/pipeline/typed-trees-to-checked-trees", 1),
    ("omega-rust/psi/representations/checked-trees", 1),
    ("omega-rust/psi/semantics/terminal-verifier", 1),
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn is_catch_all_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| CATCH_ALL_NAMES.contains(&name))
}

/// Catch-all-named files under one crate's `src/` tree. Nested crates own
/// their own entries and `tests` directories are fixtures, so both are
/// skipped.
fn count_catch_all_modules(directory: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    let mut found = Vec::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read entry under {}: {error}", directory.display()))
            .path();
        if path.is_dir() {
            if path.join("Cargo.toml").is_file()
                || path.file_name().is_some_and(|name| name == "tests")
            {
                continue;
            }
            found.extend(count_catch_all_modules(&path));
        } else if is_catch_all_name(&path) {
            found.push(path);
        }
    }
    found
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
        if !path.is_dir() {
            continue;
        }
        let is_crate = path.join("Cargo.toml").is_file() && path.join("src").is_dir();
        if path.file_name().is_some_and(|name| name == "target") && !is_crate {
            continue;
        }
        if is_crate {
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
fn catch_all_modules_never_grow_per_crate() {
    let root = workspace_root();
    let mut regressions = Vec::new();
    for crate_directory in crate_directories(&root) {
        let found = count_catch_all_modules(&root.join(&crate_directory).join("src"));
        let ceiling = CATCH_ALL_MODULE_CEILINGS
            .iter()
            .find_map(|(directory, ceiling)| (*directory == crate_directory).then_some(*ceiling))
            .unwrap_or(0);
        if found.len() > ceiling {
            regressions.push(format!(
                "{crate_directory}: {} files, ceiling {ceiling}:\n  {}",
                found.len(),
                found
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ));
        }
    }
    assert!(
        regressions.is_empty(),
        "catch-all module names (shared/helpers/common/util/prelude) grew; name the module \
         for the responsibility it owns and import names from their owners:\n{}",
        regressions.join("\n")
    );
}

#[test]
fn catch_all_module_ceilings_are_still_exact() {
    let root = workspace_root();
    let crates = crate_directories(&root);
    let mut stale = Vec::new();
    for (crate_directory, ceiling) in CATCH_ALL_MODULE_CEILINGS {
        if !crates.iter().any(|directory| directory == crate_directory) {
            stale.push(format!(
                "{crate_directory}: crate no longer exists; delete the entry"
            ));
            continue;
        }
        let count = count_catch_all_modules(&root.join(crate_directory).join("src")).len();
        if count < *ceiling {
            stale.push(format!(
                "{crate_directory}: {count} files, entry says {ceiling}; lower it (or delete it at zero)"
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "catch-all module ceilings must track the tree exactly so the list only moves down:\n{}",
        stale.join("\n")
    );
}

#[test]
fn catch_all_detection_matches_the_documented_names() {
    for name in ["shared.rs", "helpers.rs", "prelude.rs", "util.rs"] {
        assert!(is_catch_all_name(
            Path::new("src/lowering").join(name).as_path()
        ));
    }
    for name in ["instruction_order.rs", "shared_types.rs", "mod.rs"] {
        assert!(!is_catch_all_name(
            Path::new("src/lowering").join(name).as_path()
        ));
    }
}
