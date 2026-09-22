//! Production modules that re-export a child with a glob only ever get fewer.
//!
//! `pub use child::*;` (with any visibility) makes a module's public surface
//! invisible at the module that publishes it: a reader at a crate root or an
//! area owner sees a list of children, not the names those children
//! contribute, and must open every child to learn what the owner exports.
//! That is the navigation failure the discoverability rules exist to prevent
//! ("`lib.rs` wiring, re-exports, and a prose file map do not substitute for
//! that orchestration"). Named re-exports say what an owner offers; globs say
//! only that something is offered.
//!
//! The repository still carries several hundred such lines under `src/`. This
//! ratchet records, per crate, how many production lines re-export with a
//! glob today and fails when that number grows. An entry records the exact
//! count its crate has right now: growing past it fails as a regression,
//! shrinking fails as a stale entry, and the entry is lowered or deleted. The
//! list moves one direction. Adding a named re-export is always allowed;
//! adding a glob re-export is allowed only by removing one elsewhere in the
//! same crate.

use std::fs;
use std::path::{Path, PathBuf};

/// Exact no-growth ratchets: (crate directory, production lines under its
/// `src/` that re-export a module with a glob).
const GLOB_REEXPORT_CEILINGS: &[(&str, usize)] = &[
    ("omega-rust/omega/backend/artifacts/native-artifact", 2),
    ("omega-rust/omega/backend/images/image", 7),
    (
        "omega-rust/omega/backend/instruction_set_architectures/isa-aarch64",
        7,
    ),
    (
        "omega-rust/omega/backend/instruction_set_architectures/isa-x86_64",
        8,
    ),
    ("omega-rust/omega/backend/layout", 1),
    ("omega-rust/omega/backend/machine-emission", 22),
    ("omega-rust/omega/backend/plans/program-entry-plan", 8),
    ("omega-rust/omega/backend/register-environment", 1),
    ("omega-rust/omega/backend/runtime/component-publication", 3),
    (
        "omega-rust/omega/backend/runtime/executable-installation",
        1,
    ),
    ("omega-rust/omega/backend/runtime/external-roots", 8),
    ("omega-rust/omega/backend/runtime/runtime-abi", 1),
    ("omega-rust/omega/build/provider-planning", 2),
    ("omega-rust/omega/packages/review/evidence", 13),
    (
        "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations",
        11,
    ),
    (
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations",
        1,
    ),
    (
        "omega-rust/omega/pipeline/post-allocation-machine-to-selected-form-encoding",
        3,
    ),
    (
        "omega-rust/omega/pipeline/register-homes-to-post-allocation-machine",
        8,
    ),
    (
        "omega-rust/omega/pipeline/resolved-layout-to-resolved-layout",
        4,
    ),
    (
        "omega-rust/omega/pipeline/selected-form-encoding-to-resolved-layout",
        1,
    ),
    (
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes",
        12,
    ),
    (
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions",
        29,
    ),
    (
        "omega-rust/omega/pipeline/target-operations-to-selected-instructions",
        2,
    ),
    ("omega-rust/omega/representations/abstract-operations", 13),
    ("omega-rust/omega/representations/boundary-applications", 1),
    ("omega-rust/omega/representations/legalized-operations", 10),
    ("omega-rust/omega/representations/machine-code", 48),
    ("omega-rust/omega/representations/optimization-unit", 17),
    ("omega-rust/omega/representations/physical-instructions", 7),
    ("omega-rust/omega/representations/register-homes", 20),
    ("omega-rust/omega/representations/register-model", 3),
    (
        "omega-rust/omega/representations/representation-selections",
        1,
    ),
    ("omega-rust/omega/representations/selected-instructions", 6),
    ("omega-rust/omega/representations/target-operations", 24),
    ("omega-rust/omega/semantics/optimization-unit-semantics", 50),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees",
        15,
    ),
    ("omega-rust/psi/representations/facts", 6),
    ("omega-rust/psi/representations/flow-effects", 1),
    ("omega-rust/psi/representations/lowered-psi", 2),
    ("omega-rust/psi/representations/optimization", 1),
    ("omega-rust/psi/representations/symbol-resolved-trees", 1),
    ("omega-rust/psi/semantics/terminal-verifier", 8),
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

/// Whether one line re-exports a module with a glob: `pub use path::*;` with
/// any `pub`, `pub(crate)`, `pub(super)`, or `pub(in ...)` visibility. A plain
/// `use path::*;` is an import, not a re-export, and belongs to the
/// self-import ratchet when it names an ancestor.
fn is_glob_reexport(line: &str) -> bool {
    let line = line.trim();
    let Some(rest) = line.strip_prefix("pub") else {
        return false;
    };
    let rest = if let Some(after_visibility) = rest.strip_prefix('(') {
        let Some(close) = after_visibility.find(')') else {
            return false;
        };
        &after_visibility[close + 1..]
    } else {
        rest
    };
    let Some(path) = rest.strip_prefix(" use ") else {
        return false;
    };
    path.strip_suffix("::*;")
        .is_some_and(|prefix| !prefix.is_empty() && !prefix.contains('{'))
}

fn count_glob_reexport_lines(path: &Path) -> usize {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    source.lines().filter(|line| is_glob_reexport(line)).count()
}

/// Glob re-export lines under one crate's `src/` tree. Nested crates own
/// their own entries and are skipped.
fn count_glob_reexports(directory: &Path) -> usize {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    let mut count = 0;
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read entry under {}: {error}", directory.display()))
            .path();
        if path.is_dir() {
            if path.join("Cargo.toml").is_file() {
                continue;
            }
            count += count_glob_reexports(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            count += count_glob_reexport_lines(&path);
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
fn glob_reexports_never_grow_per_crate() {
    let root = workspace_root();
    let mut regressions = Vec::new();
    for crate_directory in crate_directories(&root) {
        let count = count_glob_reexports(&root.join(&crate_directory).join("src"));
        let ceiling = GLOB_REEXPORT_CEILINGS
            .iter()
            .find_map(|(directory, ceiling)| (*directory == crate_directory).then_some(*ceiling))
            .unwrap_or(0);
        if count > ceiling {
            regressions.push(format!(
                "{crate_directory}: {count} lines, ceiling {ceiling}"
            ));
        }
    }
    assert!(
        regressions.is_empty(),
        "production glob re-exports (`pub use module::*;`) grew; name the items the owner \
         exports instead of raising the ceiling:\n{}",
        regressions.join("\n")
    );
}

#[test]
fn glob_reexport_ceilings_are_still_exact() {
    let root = workspace_root();
    let crates = crate_directories(&root);
    let mut stale = Vec::new();
    for (crate_directory, ceiling) in GLOB_REEXPORT_CEILINGS {
        if !crates.iter().any(|directory| directory == crate_directory) {
            stale.push(format!(
                "{crate_directory}: crate no longer exists; delete the entry"
            ));
            continue;
        }
        let count = count_glob_reexports(&root.join(crate_directory).join("src"));
        if count < *ceiling {
            stale.push(format!(
                "{crate_directory}: {count} lines, entry says {ceiling}; lower it (or delete it at zero)"
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "glob re-export ceilings must track the tree exactly so the list only moves down:\n{}",
        stale.join("\n")
    );
}

#[test]
fn glob_reexport_detection_matches_the_documented_forms() {
    for line in [
        "pub use rewrites::*;",
        "    pub(crate) use analyses::*;",
        "pub(super) use self::children::*;",
        "pub(in crate::capture) use signatures::*;",
    ] {
        assert!(is_glob_reexport(line), "{line}");
    }
    for line in [
        "use super::*;",
        "use crate::model::*;",
        "pub use rewrites::{Rewrite, apply};",
        "pub use rewrites::Rewrite;",
        "pub mod rewrites;",
        "pub(crate) use ::*;",
    ] {
        assert!(!is_glob_reexport(line), "{line}");
    }
}
