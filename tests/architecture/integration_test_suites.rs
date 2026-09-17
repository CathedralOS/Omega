//! Crates whose integration tests link as one binary keep that binary complete.
//!
//! A crate with `autotests = false` and a `tests/suite.rs` root promises that
//! every `tests/<topic>.rs` file is a module of that root: a topic file that no
//! `mod` line names would silently never compile or run.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.expect("read directory entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                pending.push(path);
            } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn suite_root(manifest: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(manifest).expect("read manifest");
    if !text.lines().any(|line| line.trim() == "autotests = false") {
        return None;
    }
    let suite = manifest
        .parent()
        .expect("manifest directory")
        .join("tests/suite.rs");
    suite.exists().then_some(suite)
}

fn declared_modules(suite: &Path) -> BTreeSet<String> {
    fs::read_to_string(suite)
        .expect("read suite root")
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("pub ").unwrap_or(line);
            rest.strip_prefix("mod ")
                .and_then(|name| name.strip_suffix(';'))
                .map(str::to_owned)
        })
        .collect()
}

fn explicit_test_paths(manifest: &Path) -> BTreeSet<String> {
    fs::read_to_string(manifest)
        .expect("read manifest")
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("path = \"tests/")
                .and_then(|rest| rest.strip_suffix(".rs\""))
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn every_topic_file_of_a_single_binary_test_suite_is_a_module_of_its_root() {
    let mut missing = Vec::new();
    let mut suites = 0;
    for manifest in manifests(&workspace_root().join("omega-rust")) {
        let Some(suite) = suite_root(&manifest) else {
            continue;
        };
        suites += 1;
        let declared = declared_modules(&suite);
        let explicit = explicit_test_paths(&manifest);
        let tests = suite.parent().expect("tests directory");
        for entry in fs::read_dir(tests).expect("read tests directory") {
            let path = entry.expect("read tests entry").path();
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let stem = path
                .file_stem()
                .expect("file stem")
                .to_string_lossy()
                .into_owned();
            if stem == "suite" || explicit.contains(&stem) || declared.contains(&stem) {
                continue;
            }
            missing.push(format!(
                "{} is not a `mod` of {}",
                path.display(),
                suite.display()
            ));
        }
    }
    assert!(
        suites > 0,
        "no single-binary test suites found; the guard is misaimed"
    );
    assert!(
        missing.is_empty(),
        "topic files outside their suite root:\n{}",
        missing.join("\n")
    );
}
