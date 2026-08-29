//! Repository guard for the package subsystem's source-navigation contract.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PACKAGE_CRATES: &[&str] = &[
    "omega-package-advisory",
    "omega-package-manager",
    "omega-package-review",
    "omega-package-source",
    "omega-resolver-execution",
];
const MAX_PRODUCTION_LEAF_LINES: usize = 800;
const MAX_TEST_LEAF_LINES: usize = 900;
const MAX_ENTRANCE_LINES: usize = 250;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn package_root() -> PathBuf {
    workspace_root().join("source/omega-rust/omega/packages")
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap_or_else(|error| {
            panic!("read package directory {}: {error}", directory.display())
        }) {
            let entry = entry.expect("read package source entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn package_top_level_is_the_exact_advertised_crate_map() {
    let packages = package_root();
    let actual = fs::read_dir(&packages)
        .expect("read package subsystem entrance")
        .map(|entry| {
            entry
                .expect("read package subsystem entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    let expected = std::iter::once("README.md")
        .chain(PACKAGE_CRATES.iter().copied())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "packages/ must remain an exact, documented map of package responsibilities"
    );

    let package_readme = fs::read_to_string(packages.join("README.md"))
        .expect("read package subsystem README entrance");
    assert!(package_readme.starts_with("# "));
    for crate_name in PACKAGE_CRATES {
        assert!(
            package_readme.contains(&format!("{crate_name}/")),
            "packages/README.md must advertise {crate_name}"
        );
    }
}

#[test]
fn every_advertised_package_has_readme_cargo_and_library_entrances() {
    let packages = package_root();
    for crate_name in PACKAGE_CRATES {
        let crate_root = packages.join(crate_name);
        let readme_path = crate_root.join("README.md");
        let cargo_path = crate_root.join("Cargo.toml");
        let library_path = crate_root.join("src/lib.rs");
        for entrance in [&readme_path, &cargo_path, &library_path] {
            assert!(
                entrance.is_file(),
                "advertised package entrance is missing: {}",
                entrance.display()
            );
        }

        let readme = fs::read_to_string(&readme_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", readme_path.display()));
        assert!(
            readme.lines().take(12).any(|line| line.starts_with('#')),
            "package README must introduce its responsibility: {}",
            readme_path.display()
        );

        let cargo = fs::read_to_string(&cargo_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", cargo_path.display()));
        assert!(
            cargo.contains("[package]") && cargo.contains(&format!("name = \"{crate_name}\"")),
            "package Cargo entrance must declare its advertised crate name: {}",
            cargo_path.display()
        );

        let library = fs::read_to_string(&library_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", library_path.display()));
        assert!(
            library.lines().take(12).any(|line| line.starts_with("//!")),
            "package library entrance must guide readers into its owners: {}",
            library_path.display()
        );
    }
}

#[test]
fn package_sources_keep_bounded_entrances_and_named_leaves() {
    let packages = package_root();
    for path in rust_files(&packages) {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("Rust source has a UTF-8 file name");
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read package source {}: {error}", path.display()));
        let lines = source.lines().count();
        let is_entrance = matches!(file_name, "lib.rs" | "mod.rs");
        let is_test = file_name == "tests.rs"
            || path
                .components()
                .any(|component| component.as_os_str() == "tests");
        let (limit, kind) = if is_entrance {
            (MAX_ENTRANCE_LINES, "entrance")
        } else if is_test {
            (MAX_TEST_LEAF_LINES, "test leaf")
        } else {
            (MAX_PRODUCTION_LEAF_LINES, "production leaf")
        };
        assert!(
            lines <= limit,
            "package {kind} exceeds its {limit}-line source-navigation limit (found {lines}): {}",
            path.display()
        );
    }
}
