//! Positive source-navigation contract for the package subsystem.
//!
//! This guard names current owners and reader entrances. It deliberately does
//! not preserve a blacklist of historical paths: architecture is proved by the
//! tree that exists, its documented dependency direction, and bounded leaves.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PACKAGE_CRATES: &[&str] = &[
    "advisory",
    "evidence",
    "manager",
    "resolver-execution",
    "source",
];
const MANAGER_OWNERS: &[&str] = &[
    "graph",
    "package",
    "project",
    "review",
    "sources",
    "workflows",
];
const EVIDENCE_OWNERS: &[&str] = &["encoding", "evidence", "obligations", "projection"];
const MAX_PRODUCTION_LEAF_LINES: usize = 600;
const MAX_TEST_LEAF_LINES: usize = 800;
const MAX_ENTRANCE_LINES: usize = 160;
const MAX_SOURCE_DIRECTORY_DEPTH: usize = 5;

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

fn directory_entries(root: &Path) -> BTreeSet<String> {
    fs::read_dir(root)
        .unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
        .map(|entry| {
            entry
                .expect("read directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.expect("read package source entry").path();
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

fn assert_documented_owners(crate_root: &Path, owners: &[&str]) {
    let source_root = crate_root.join("src");
    let expected = std::iter::once("lib.rs".to_owned())
        .chain(owners.iter().map(|owner| (*owner).to_owned()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        directory_entries(&source_root),
        expected,
        "{} must expose one exact, readable owner map",
        source_root.display()
    );

    let readme = fs::read_to_string(crate_root.join("README.md"))
        .unwrap_or_else(|error| panic!("read {} README: {error}", crate_root.display()));
    let library = fs::read_to_string(source_root.join("lib.rs"))
        .unwrap_or_else(|error| panic!("read {} library entrance: {error}", crate_root.display()));
    for owner in owners {
        assert!(
            readme.contains(&format!("{owner}/")) && library.contains(&format!("mod {owner};")),
            "{} must advertise `{owner}` in both human and Rust entrances",
            crate_root.display()
        );
        let entrance = source_root.join(owner).join("mod.rs");
        let source = fs::read_to_string(&entrance)
            .unwrap_or_else(|error| panic!("read {}: {error}", entrance.display()));
        assert!(
            source.lines().take(12).any(|line| line.starts_with("//!")),
            "owner entrance must explain where curiosity leads next: {}",
            entrance.display()
        );
    }
}

#[test]
fn package_top_level_is_the_exact_advertised_crate_map() {
    let packages = package_root();
    let expected = std::iter::once("README.md".to_owned())
        .chain(PACKAGE_CRATES.iter().map(|name| (*name).to_owned()))
        .collect::<BTreeSet<_>>();
    assert_eq!(directory_entries(&packages), expected);

    let readme = fs::read_to_string(packages.join("README.md"))
        .expect("read package subsystem README entrance");
    assert!(readme.starts_with("# "));
    for crate_name in PACKAGE_CRATES {
        let crate_root = packages.join(crate_name);
        assert!(crate_root.join("README.md").is_file());
        assert!(crate_root.join("Cargo.toml").is_file());
        assert!(crate_root.join("src/lib.rs").is_file());
        assert!(
            readme.contains(&format!("{crate_name}/")),
            "packages/README.md must advertise {crate_name}"
        );
    }
}

#[test]
fn manager_and_compiler_evidence_have_exact_reader_entrances() {
    let packages = package_root();
    assert_documented_owners(&packages.join("manager"), MANAGER_OWNERS);
    assert_documented_owners(&packages.join("evidence"), EVIDENCE_OWNERS);
}

#[test]
fn stable_package_evidence_excludes_compiler_private_projection_handles() {
    let evidence = package_root().join("evidence/src/evidence");
    for path in rust_files(&evidence) {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        for forbidden in ["SymbolHandle", "psi_source::SourceSpan"] {
            assert!(
                !source.contains(forbidden),
                "stable package evidence retains compiler-private `{forbidden}` in {}",
                path.display()
            );
        }
    }
}

#[test]
fn stable_evidence_and_encoding_exclude_compiler_representations() {
    let package = package_root().join("evidence/src");
    for owner in ["evidence", "encoding"] {
        for path in rust_files(&package.join(owner)) {
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            for forbidden in ["psi_typed_trees", "psi_checked_trees", "psi_facts"] {
                assert!(
                    !source.contains(forbidden),
                    "stable {owner} retains compiler representation `{forbidden}` in {}",
                    path.display(),
                );
            }
        }
    }
}

#[test]
fn package_evidence_encoding_has_one_canonical_encoder_owner() {
    let encoding = package_root().join("evidence/src/encoding");
    assert_eq!(
        directory_entries(&encoding),
        BTreeSet::from([
            "encode".to_owned(),
            "mod.rs".to_owned(),
            "recovery".to_owned(),
        ]),
        "package-evidence encoding must expose canonical encoding and recovery as its only owners",
    );

    let encoder = encoding.join("encode");
    assert!(
        encoder.join("values/mod.rs").is_file(),
        "semantic value encoders must remain subordinate to canonical encoding",
    );
    let encoder_entrance =
        fs::read_to_string(encoder.join("mod.rs")).expect("read package-evidence encoder entrance");
    assert!(
        encoder_entrance.lines().any(|line| line == "mod values;"),
        "the canonical encoder entrance must own its semantic value encoders",
    );

    let encoding_entrance = fs::read_to_string(encoding.join("mod.rs"))
        .expect("read package-evidence encoding entrance");
    for forbidden_peer in ["mod values;", "mod decode;"] {
        assert!(
            !encoding_entrance.lines().any(|line| line == forbidden_peer),
            "encoding must not restore the former sibling owner `{forbidden_peer}`",
        );
    }
}

#[test]
fn source_tests_live_with_their_owners() {
    let source = package_root().join("source/src");
    assert!(
        !source.join("tests").exists(),
        "package source must not recover a crate-wide wildcard test hub"
    );
    for owner in ["custody", "git", "local"] {
        assert!(
            source.join(owner).join("tests").is_dir(),
            "private source invariants must live beside `{owner}`"
        );
    }
    for path in rust_files(&source) {
        let text = fs::read_to_string(&path).expect("read package-source Rust file");
        assert!(
            !text.contains("#[allow(unused_imports)]"),
            "owner-local tests must not depend on wildcard forwarding: {}",
            path.display()
        );
    }
}

#[test]
fn package_sources_are_bounded_and_shallow_enough_to_navigate() {
    let packages = package_root();
    for crate_name in PACKAGE_CRATES {
        let source_root = packages.join(crate_name).join("src");
        for path in rust_files(&source_root) {
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let file_name = path.file_name().and_then(|name| name.to_str()).unwrap();
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
            let lines = source.lines().count();
            assert!(
                lines <= limit,
                "package {kind} exceeds its {limit}-line navigation limit (found {lines}): {}",
                path.display()
            );

            let depth = path
                .strip_prefix(&source_root)
                .expect("package source belongs to its crate")
                .parent()
                .map(|parent| parent.components().count())
                .unwrap_or(0);
            assert!(
                depth <= MAX_SOURCE_DIRECTORY_DEPTH,
                "package source exceeds its {MAX_SOURCE_DIRECTORY_DEPTH}-directory navigation depth (found {depth}): {}",
                path.display()
            );
        }
    }
}
