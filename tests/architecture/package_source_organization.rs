//! Positive source-navigation contract for the package subsystem.
//!
//! This guard names current owners and reader entrances. It deliberately does
//! not preserve a blacklist of historical paths: architecture is proved by the
//! tree that exists, its documented dependency direction, and bounded leaves.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PACKAGE_ROOT_ENTRIES: &[&str] = &[
    "README.md",
    "manager",
    "review",
    "sources",
];
const PACKAGE_CRATES: &[&str] = &[
    "manager",
    "review/advisory",
    "review/evidence",
    "sources/acquisition",
    "sources/execution",
];
const MANAGER_OWNERS: &[&str] = &["declarations", "operations", "resolution", "review"];
const EVIDENCE_OWNERS: &[&str] = &["capture", "encoding", "ledger", "record"];
const MAX_PRODUCTION_LEAF_LINES: usize = 525;
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
fn packages_exposes_one_reader_route_per_subsystem_responsibility() {
    let packages = package_root();
    let expected = PACKAGE_ROOT_ENTRIES
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(directory_entries(&packages), expected);

    let readme = fs::read_to_string(packages.join("README.md"))
        .expect("read package subsystem README entrance");
    assert!(readme.starts_with("# Omega Package Subsystem"));
    for area_name in ["manager", "review", "sources"] {
        assert!(
            readme.contains(&format!("{area_name}/")),
            "packages/README.md must advertise {area_name}"
        );
    }
    for group in ["review", "sources"] {
        assert!(
            packages.join(group).join("README.md").is_file(),
            "package responsibility group `{group}` needs a reader entrance"
        );
    }
    for crate_name in PACKAGE_CRATES {
        let crate_root = packages.join(crate_name);
        assert!(crate_root.join("README.md").is_file());
        assert!(crate_root.join("Cargo.toml").is_file());
        assert!(crate_root.join("src/lib.rs").is_file());
    }
}

#[test]
fn manager_and_compiler_evidence_have_exact_reader_entrances() {
    let packages = package_root();
    assert_documented_owners(&packages.join("manager"), MANAGER_OWNERS);
    assert_documented_owners(&packages.join("review/evidence"), EVIDENCE_OWNERS);
}

#[test]
fn ordinary_compilation_handoff_belongs_to_resolution_not_review() {
    let packages = package_root();
    let manager = packages.join("manager");
    let compiler_input = manager.join("src/resolution/compiler_input.rs");
    assert!(compiler_input.is_file());
    assert!(
        !manager.join("src/review/candidate/inputs.rs").exists(),
        "ordinary compiler input construction must not be hidden under package review",
    );

    for path in [
        manager.join("src/operations/prepare_project.rs"),
        compiler_input,
    ] {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            !source.contains("crate::review"),
            "ordinary package compilation must not route through review ownership: {}",
            path.display(),
        );
    }
}

#[test]
fn stable_package_evidence_excludes_compiler_private_projection_handles() {
    let evidence = package_root().join("review/evidence/src/record");
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
    let package = package_root().join("review/evidence/src");
    for owner in ["record", "encoding"] {
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
    let encoding = package_root().join("review/evidence/src/encoding");
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
    let source = package_root().join("sources/acquisition/src");
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
fn package_source_has_shared_owners_and_one_way_adapter_dependencies() {
    let crate_root = package_root().join("sources/acquisition");
    let source = crate_root.join("src");
    let readme = fs::read_to_string(crate_root.join("README.md"))
        .expect("read package-source README entrance");
    let library = fs::read_to_string(source.join("lib.rs"))
        .expect("read package-source library entrance");

    for owner in [
        "identity",
        "tree",
        "snapshot",
        "custody",
        "local",
        "git",
        "observations",
    ] {
        assert!(
            readme.contains(&format!("{owner}/")),
            "package-source README must advertise `{owner}/`",
        );
        assert!(
            library.contains(&format!("mod {owner};")),
            "package-source library must name `{owner}`",
        );
    }

    for (adapter, forbidden_adapter) in [("local", "git"), ("git", "local")] {
        for path in rust_files(&source.join(adapter)) {
            if path
                .components()
                .any(|component| component.as_os_str() == "tests")
            {
                continue;
            }
            let text = fs::read_to_string(&path).expect("read package-source adapter file");
            assert!(
                !text.contains(&format!("crate::{forbidden_adapter}::")),
                "`{adapter}` must not depend on `{forbidden_adapter}`: {}",
                path.display(),
            );
        }
    }

    for shared_owner in ["identity", "tree", "snapshot", "custody"] {
        for path in rust_files(&source.join(shared_owner)) {
            if path
                .components()
                .any(|component| component.as_os_str() == "tests")
            {
                continue;
            }
            let text = fs::read_to_string(&path).expect("read shared package-source owner file");
            for adapter in ["local", "git"] {
                assert!(
                    !text.contains(&format!("crate::{adapter}::")),
                    "shared `{shared_owner}` must not depend on `{adapter}`: {}",
                    path.display(),
                );
            }
        }
    }

    assert!(source.join("tree/capture/mod.rs").is_file());
    assert!(!source.join("tree/capture.rs").exists());
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
