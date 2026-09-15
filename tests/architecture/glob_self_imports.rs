//! Files that import their own crate or parent module with a glob only ever
//! get fewer.
//!
//! `use super::*;` and `use crate::*;` turn a parent module into a hidden
//! namespace: a reader following a call from an entrance into a leaf cannot
//! see where a name comes from without a repository-wide search, which is the
//! navigation failure the source-organization audits exist to prevent. The
//! repository already holds more than two thousand such files, so this gate
//! does not demand they vanish at once. It records, per crate, how many files
//! carry a glob self-import today and fails when that number grows.
//!
//! An entry records the exact count its crate has right now. Growing past it
//! fails as a regression; shrinking fails as a stale entry, and the entry is
//! lowered or deleted. The list moves one direction. Writing a new file with
//! explicit imports is always allowed; writing one with `use super::*;` is
//! allowed only by removing the glob from another file in the same crate.

use std::fs;
use std::path::{Path, PathBuf};

/// Exact no-growth ratchets: (crate directory, files carrying a glob
/// self-import under its `src/`).
const GLOB_SELF_IMPORT_CEILINGS: &[(&str, usize)] = &[
    ("omega-rust/omega", 11),
    ("omega-rust/omega/backend/artifacts/component-candidate", 1),
    ("omega-rust/omega/backend/artifacts/native-artifact", 11),
    ("omega-rust/omega/backend/images/image", 4),
    ("omega-rust/omega/backend/images/image-emission", 34),
    ("omega-rust/omega/backend/images/image-macho", 1),
    (
        "omega-rust/omega/backend/instruction_set_architectures/isa-aarch64",
        32,
    ),
    (
        "omega-rust/omega/backend/instruction_set_architectures/isa-x86_64",
        39,
    ),
    (
        "omega-rust/omega/backend/instruction_set_architectures/x86-encoding",
        1,
    ),
    ("omega-rust/omega/backend/layout", 2),
    ("omega-rust/omega/backend/machine-emission", 9),
    ("omega-rust/omega/backend/plans/backend-plan", 2),
    ("omega-rust/omega/backend/plans/program-entry-plan", 9),
    ("omega-rust/omega/backend/register-environment", 4),
    ("omega-rust/omega/backend/runtime/component-publication", 1),
    (
        "omega-rust/omega/backend/runtime/executable-installation",
        6,
    ),
    ("omega-rust/omega/backend/runtime/external-roots", 15),
    ("omega-rust/omega/backend/runtime/runtime-abi", 1),
    ("omega-rust/omega/build/build-declarations", 1),
    ("omega-rust/omega/build/build-evaluation", 14),
    ("omega-rust/omega/build/build-output", 3),
    ("omega-rust/omega/build/package-compilation", 2),
    ("omega-rust/omega/build/provider-planning", 12),
    ("omega-rust/omega/build/selected-dispatch", 21),
    ("omega-rust/omega/build/trust-ledger", 1),
    ("omega-rust/omega/build/trust-model", 3),
    ("omega-rust/omega/compiler/compilation-report", 2),
    ("omega-rust/omega/compiler/compiler", 6),
    ("omega-rust/omega/compiler/native-realization", 21),
    ("omega-rust/omega/packages/manager", 58),
    ("omega-rust/omega/packages/review/evidence", 90),
    ("omega-rust/omega/packages/sources/acquisition", 31),
    ("omega-rust/omega/packages/sources/execution", 2),
    ("omega-rust/omega/packages/topology", 3),
    (
        "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations",
        147,
    ),
    (
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations",
        41,
    ),
    (
        "omega-rust/omega/pipeline/post-allocation-machine-to-selected-form-encoding",
        3,
    ),
    (
        "omega-rust/omega/pipeline/resolved-layout-to-resolved-layout",
        2,
    ),
    (
        "omega-rust/omega/pipeline/selected-form-encoding-to-resolved-layout",
        1,
    ),
    (
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes",
        14,
    ),
    (
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions",
        53,
    ),
    (
        "omega-rust/omega/pipeline/target-operations-to-selected-instructions",
        182,
    ),
    (
        "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations",
        3,
    ),
    ("omega-rust/omega/representations/boundary-applications", 1),
    ("omega-rust/omega/representations/calling-conventions", 4),
    ("omega-rust/omega/representations/effects", 12),
    ("omega-rust/omega/representations/legalized-operations", 13),
    ("omega-rust/omega/representations/machine-code", 7),
    ("omega-rust/omega/representations/optimization-core", 9),
    ("omega-rust/omega/representations/optimization-unit", 54),
    ("omega-rust/omega/representations/register-homes", 7),
    ("omega-rust/omega/representations/register-model", 2),
    (
        "omega-rust/omega/representations/representation-selections",
        1,
    ),
    ("omega-rust/omega/representations/selected-instructions", 11),
    ("omega-rust/omega/representations/task-plans", 2),
    (
        "omega-rust/omega/semantics/optimization-unit-semantics",
        158,
    ),
    ("omega-rust/omega/tooling/artifacts", 1),
    ("omega-rust/omega/tooling/platform-custody", 4),
    ("omega-rust/omega/tooling/visualizations", 16),
    ("omega-rust/psi/compiler/terminal-production", 1),
    ("omega-rust/psi/foundation/access-plans", 3),
    ("omega-rust/psi/foundation/extents", 2),
    ("omega-rust/psi/foundation/language-semantics", 3),
    ("omega-rust/psi/foundation/layout-plans", 1),
    ("omega-rust/psi/foundation/numerics", 3),
    ("omega-rust/psi/foundation/semantic-vocabulary", 5),
    ("omega-rust/psi/foundation/symbols", 3),
    ("omega-rust/psi/pipeline/checked-trees-to-lowered-psi", 252),
    ("omega-rust/psi/pipeline/lowered-psi-to-lowered-psi", 4),
    (
        "omega-rust/psi/pipeline/symbol-resolved-trees-to-typed-trees",
        7,
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees",
        37,
    ),
    ("omega-rust/psi/pipeline/tokens-to-syntax-trees", 4),
    ("omega-rust/psi/pipeline/typed-trees-to-checked-trees", 451),
    ("omega-rust/psi/representations/checked-trees", 16),
    ("omega-rust/psi/representations/facts", 3),
    ("omega-rust/psi/representations/flow-effects", 1),
    ("omega-rust/psi/representations/optimization", 1),
    ("omega-rust/psi/representations/terminal-psi", 3),
    ("omega-rust/psi/representations/typed-trees", 6),
    ("omega-rust/psi/semantics/build-time-evaluation", 21),
    ("omega-rust/psi/semantics/checked-interpreter", 38),
    ("omega-rust/psi/semantics/proof", 5),
    ("omega-rust/psi/semantics/proof-admission", 17),
    ("omega-rust/psi/semantics/terminal-codec", 16),
    ("omega-rust/psi/semantics/terminal-fixed-fuel", 1),
    ("omega-rust/psi/semantics/terminal-interpreter", 20),
    ("omega-rust/psi/semantics/terminal-semantics", 8),
    ("omega-rust/psi/semantics/terminal-verifier", 56),
    ("omega-rust/psi/semantics/validation", 110),
];

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
            count += count_glob_self_import_files(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && file_carries_glob_self_import(&path)
        {
            count += 1;
        }
    }
    count
}

/// Every crate directory under `omega-rust/` that has a `Cargo.toml` and a
/// `src/` tree, as workspace-relative paths.
fn crate_directories(root: &Path) -> Vec<String> {
    let mut crates = Vec::new();
    collect_crate_directories(root, &root.join("omega-rust"), &mut crates);
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
        let count = count_glob_self_import_files(&root.join(&crate_directory).join("src"));
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
        let count = count_glob_self_import_files(&root.join(crate_directory).join("src"));
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
