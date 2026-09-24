//! Production modules declared with `#[path = "..."]` only ever get fewer.
//!
//! A reader following `mod x;` expects `x.rs` or `x/mod.rs` beside the
//! declaration; a `#[path]` attribute sends them somewhere else, and the
//! redirect is invisible from every other file that names `x`. Tests are
//! fixtures, so a test module reached through a path attribute costs less,
//! but it still hides where the file lives; this ratchet records, per crate,
//! how many production source files (under `src/`, outside `tests`
//! directories and test files) still carry one and fails when that number
//! grows. An entry records the exact count its crate has right now. Growing
//! past it fails as a regression; shrinking fails as a stale entry, and the
//! entry is lowered or deleted.

use std::fs;
use std::path::{Path, PathBuf};

/// Exact no-growth ratchets: (crate directory, `#[path = ...]` attributes
/// in production source under its `src/` tree).
const PATH_ATTRIBUTE_CEILINGS: &[(&str, usize)] = &[(
    "omega-rust/omega/pipeline/selected-form-encoding-to-resolved-layout",
    1,
)];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn is_path_attribute(line: &str) -> bool {
    let line = line.trim();
    line.strip_prefix("#[path")
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}

/// A path attribute that reaches into a `tests` directory, or that sits under
/// `#[cfg(test)]`, declares a test fixture (a support module shared between a
/// crate's unit tests and its integration tests); it is not a production
/// module and is not counted.
fn is_fixture_path_attribute(previous: Option<&str>, line: &str) -> bool {
    line.contains("/tests/")
        || line.contains("\"tests/")
        || previous.is_some_and(|previous| previous.trim() == "#[cfg(test)]")
}

fn count_path_attribute_lines(path: &Path) -> usize {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let mut previous = None;
    let mut count = 0;
    for line in source.lines() {
        if is_path_attribute(line) && !is_fixture_path_attribute(previous, line) {
            count += 1;
        }
        previous = Some(line);
    }
    count
}

/// Path attributes in production files under one crate's `src/` tree.
/// Nested crates own their own entries; `tests` directories and test files
/// are fixtures; both are skipped.
fn count_path_attributes(directory: &Path) -> usize {
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
            count += count_path_attributes(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !is_test_file(&path)
        {
            count += count_path_attribute_lines(&path);
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
fn path_attributes_never_grow_per_crate() {
    let root = workspace_root();
    let mut regressions = Vec::new();
    for crate_directory in crate_directories(&root) {
        let count = count_path_attributes(&root.join(&crate_directory).join("src"));
        let ceiling = PATH_ATTRIBUTE_CEILINGS
            .iter()
            .find_map(|(directory, ceiling)| (*directory == crate_directory).then_some(*ceiling))
            .unwrap_or(0);
        if count > ceiling {
            regressions.push(format!(
                "{crate_directory}: {count} attributes, ceiling {ceiling}"
            ));
        }
    }
    assert!(
        regressions.is_empty(),
        "production `#[path = ...]` module attributes grew; put the file where `mod name;` \
         looks for it instead of raising the ceiling:\n{}",
        regressions.join("\n")
    );
}

#[test]
fn path_attribute_ceilings_are_still_exact() {
    let root = workspace_root();
    let crates = crate_directories(&root);
    let mut stale = Vec::new();
    for (crate_directory, ceiling) in PATH_ATTRIBUTE_CEILINGS {
        if !crates.iter().any(|directory| directory == crate_directory) {
            stale.push(format!(
                "{crate_directory}: crate no longer exists; delete the entry"
            ));
            continue;
        }
        let count = count_path_attributes(&root.join(crate_directory).join("src"));
        if count < *ceiling {
            stale.push(format!(
                "{crate_directory}: {count} attributes, entry says {ceiling}; lower it (or delete it at zero)"
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "path-attribute ceilings must track the tree exactly so the list only moves down:\n{}",
        stale.join("\n")
    );
}

#[test]
fn path_attribute_detection_matches_the_documented_forms() {
    for line in [
        "#[path = \"tests.rs\"]",
        "    #[path=\"machine/lower_machine.rs\"]",
    ] {
        assert!(is_path_attribute(line), "{line}");
    }
    for line in [
        "// #[path = \"x.rs\"] is refused",
        "#[cfg(test)]",
        "let path = 1;",
    ] {
        assert!(!is_path_attribute(line), "{line}");
    }
    assert!(is_fixture_path_attribute(
        None,
        "#[path = \"../tests/support/front_end.rs\"]"
    ));
    assert!(is_fixture_path_attribute(
        Some("#[cfg(test)]"),
        "#[path = \"support/front_end.rs\"]"
    ));
    assert!(!is_fixture_path_attribute(
        Some("use std::fs;"),
        "#[path = \"machine/lower_machine.rs\"]"
    ));
}
