//! Production files that import any module with a glob only ever get fewer.
//!
//! `glob_self_imports` retired `use super::*;` and `use crate::*;`, and
//! `glob_reexports` ratchets `pub use module::*;`. This gate covers the rest
//! of the same failure: `use super::shared::*;`, `use crate::model::*;`,
//! `use semantic_vocabulary::*;`. Whatever the path, a glob import turns the
//! target into a hidden namespace — a reader following a name from an
//! entrance into a leaf cannot see where it comes from without opening the
//! globbed module, which is the navigation failure the source-organization
//! audits exist to prevent. It records, per crate, how many production source
//! files still carry a glob import and fails when that number grows.
//!
//! An entry records the exact count its crate has right now. Growing past it
//! fails as a regression; shrinking fails as a stale entry, and the entry is
//! lowered or deleted. Test trees (`tests/` directories, `tests.rs`,
//! `*_tests.rs`) are fixtures, not routes, and are not counted.

use std::fs;
use std::path::{Path, PathBuf};

/// Exact no-growth ratchets: (crate directory, production files under its
/// `src/` tree carrying a glob import).
const GLOB_IMPORT_CEILINGS: &[(&str, usize)] = &[
    ("omega-rust/omega/backend/artifacts/native-artifact", 3),
    (
        "omega-rust/omega/backend/instruction_set_architectures/isa-aarch64",
        2,
    ),
    ("omega-rust/omega/backend/machine-emission", 23),
    ("omega-rust/omega/backend/plans/program-entry-plan", 3),
    ("omega-rust/omega/backend/register-environment", 1),
    (
        "omega-rust/omega/backend/runtime/executable-installation",
        1,
    ),
    ("omega-rust/omega/packages/manager", 3),
    ("omega-rust/omega/packages/review/evidence", 52),
    (
        "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations",
        1,
    ),
    (
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations",
        1,
    ),
    (
        "omega-rust/omega/pipeline/post-allocation-machine-to-selected-form-encoding",
        2,
    ),
    (
        "omega-rust/omega/pipeline/register-homes-to-post-allocation-machine",
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
        11,
    ),
    (
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions",
        18,
    ),
    (
        "omega-rust/omega/pipeline/target-operations-to-selected-instructions",
        1,
    ),
    ("omega-rust/omega/representations/register-model", 1),
    (
        "omega-rust/omega/representations/representation-selections",
        1,
    ),
    ("omega-rust/omega/semantics/optimization-unit-semantics", 16),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees",
        3,
    ),
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

/// Whether one line is a module glob import: `use path::*;` whose last
/// segment names a module (snake case). `use SelectedInstructionKind::*;`
/// opens an enum's variants for a match and names the enum on the same
/// line, so it hides nothing and is not counted. Re-exports
/// (`pub use path::*;`) belong to `glob_reexports`, and the self-import
/// forms belong to `glob_self_imports`, which already holds them at zero;
/// both are counted here too, since either is also a glob import.
fn is_glob_import(line: &str) -> bool {
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
    let Some(last) = path
        .split("::")
        .filter(|segment| !segment.is_empty())
        .last()
    else {
        return false;
    };
    !last.starts_with(|first: char| first.is_ascii_uppercase())
}

fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}

fn file_carries_glob_import(path: &Path) -> bool {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    source.lines().any(is_glob_import)
}

/// Production files under one crate's `src/` tree carrying a glob import.
/// Nested crates own their own entries; `tests` directories and test files
/// are fixtures; both are skipped.
fn count_glob_import_files(directory: &Path) -> usize {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    let mut count = 0;
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
            count += count_glob_import_files(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !is_test_file(&path)
            && file_carries_glob_import(&path)
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
fn glob_imports_never_grow_per_crate() {
    let root = workspace_root();
    let mut regressions = Vec::new();
    for crate_directory in crate_directories(&root) {
        let count = count_glob_import_files(&root.join(&crate_directory).join("src"));
        let ceiling = GLOB_IMPORT_CEILINGS
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
        "production files importing a module with a glob grew; import the names the file uses \
         from their owner instead of raising the ceiling:\n{}",
        regressions.join("\n")
    );
}

#[test]
fn glob_import_ceilings_are_still_exact() {
    let root = workspace_root();
    let crates = crate_directories(&root);
    let mut stale = Vec::new();
    for (crate_directory, ceiling) in GLOB_IMPORT_CEILINGS {
        if !crates.iter().any(|directory| directory == crate_directory) {
            stale.push(format!(
                "{crate_directory}: crate no longer exists; delete the entry"
            ));
            continue;
        }
        let count = count_glob_import_files(&root.join(crate_directory).join("src"));
        if count < *ceiling {
            stale.push(format!(
                "{crate_directory}: {count} files, entry says {ceiling}; lower it (or delete it at zero)"
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "glob import ceilings must track the tree exactly so the list only moves down:\n{}",
        stale.join("\n")
    );
}

#[test]
fn glob_import_detection_matches_the_documented_forms() {
    for line in [
        "use super::shared::*;",
        "    use crate::model::*;",
        "use semantic_vocabulary::*;",
        "use ::selected_instructions::*;",
        "use super::*;",
        "pub use model::*;",
    ] {
        assert!(is_glob_import(line), "{line}");
    }
    for line in [
        "use SelectedInstructionKind::*;",
        "    use selected_instructions::SelectedInstructionKind::*;",
        "use super::{A, B};",
        "use semantic_vocabulary::ValueId;",
        "// use super::*;",
        "let star = a * b;",
    ] {
        assert!(!is_glob_import(line), "{line}");
    }
}
