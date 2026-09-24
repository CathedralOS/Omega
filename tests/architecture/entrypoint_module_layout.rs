//! Entrypoint and module-layout contract for workspace crates.
//!
//! Codifies the discoverability rules of `AGENTS.md` and `omega-rust/README.md`
//! as a gate: every crate keeps a readable entrypoint (`src/lib.rs` or
//! `src/main.rs`); each program representation keeps exactly one named root
//! file beside `lib.rs` that defines the current program; `model/` grab-bag
//! directories stay out of crate source trees; and test families live in
//! named modules rather than inline bodies inside the entrypoint file.
//!
//! The pinned rosters name the pre-existing exceptions the contract still
//! tolerates. A new violation fails, and a rostered path that heals or drifts
//! also fails, so the roster can only move deliberately.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::workspace_root;

/// Representation crates whose `src/` does not keep exactly one named root
/// file beside `lib.rs` (AGENTS.md: "Each program representation has one
/// named root file beside `lib.rs`"): the two `lib.rs`-owned leaves that
/// `crate_root_responsibility.rs` lists in `LIB_RS_OWNED`.
const REPRESENTATION_ROOT_EXCEPTIONS: &[&str] = &[
    "omega-rust/omega/representations/function-identity",
    "omega-rust/omega/representations/installation-evidence",
];

/// `model/` grab-bag directories retained inside crate source trees
/// (AGENTS.md: "do not ... collect unrelated types in `model/`").
const MODEL_DIRECTORY_EXCEPTIONS: &[&str] =
    &["omega-rust/omega/representations/optimization-unit/src/optimization_unit/rewrite/model"];

/// Entrypoint files carrying an inline `#[cfg(test)] mod <name> { .. }`
/// body instead of a named test module file (`mod <name>_tests;`).
const INLINE_TEST_ENTRYPOINTS: &[&str] = &[
    "omega-rust/omega/backend/artifacts/component-candidate/src/lib.rs",
    "omega-rust/omega/backend/instruction_set_architectures/x86-encoding/src/lib.rs",
    "omega-rust/omega/representations/function-identity/src/lib.rs",
    "omega-rust/psi/semantics/terminal-codec/src/lib.rs",
];

fn workspace_crates() -> Vec<PathBuf> {
    let mut crates = Vec::new();
    let mut pending = vec![workspace_root().join("omega-rust")];
    while let Some(directory) = pending.pop() {
        // A manifest marks a crate root, not a descent boundary: crates like
        // `omega-rust/omega` carry their own `src/` yet still hold member
        // crates in sibling directories beside it.
        if directory.join("Cargo.toml").is_file() {
            crates.push(directory.clone());
        }
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.expect("read workspace entry").path();
            // `src/` is the crate's own source tree and `tests/` its harness;
            // a `target/` name is only a build directory when it carries
            // cargo's markers — `representations/target` is a real crate.
            let is_build_target = path.file_name().is_some_and(|name| name == "target")
                && (path.join("CACHEDIR.TAG").is_file()
                    || path.join("debug").is_dir()
                    || path.join("release").is_dir());
            if path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name != "src" && name != "tests")
                && !is_build_target
            {
                pending.push(path);
            }
        }
    }
    crates.sort();
    crates
}

fn representation_source_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for family in [
        "omega-rust/psi/representations",
        "omega-rust/omega/representations",
    ] {
        for entry in fs::read_dir(workspace_root().join(family))
            .unwrap_or_else(|error| panic!("read {family}: {error}"))
        {
            let src = entry.expect("read representation entry").path().join("src");
            if src.is_dir() {
                roots.push(src);
            }
        }
    }
    roots.sort();
    roots
}

fn repository_relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .expect("crate path stays inside the workspace")
        .to_string_lossy()
        .replace('\\', "/")
}

/// Non-test Rust files beside `lib.rs` at one `src/` root: the named roots a
/// representation is built from. A `tests.rs` module is test wiring, not a
/// program root.
fn sibling_roots(src: &Path) -> BTreeSet<String> {
    fs::read_dir(src)
        .unwrap_or_else(|error| panic!("read {}: {error}", src.display()))
        .filter_map(|entry| {
            let name = entry.expect("read src entry").file_name();
            let name = name.to_string_lossy();
            (name.ends_with(".rs") && name != "lib.rs" && name != "tests.rs")
                .then(|| name.into_owned())
        })
        .collect()
}

/// True when `source` carries an inline test-module body: a `#[cfg(test)]`
/// attribute directly above a `mod <name> {` block. File-backed declarations
/// (`mod name_tests;`) are the named-module pattern this gate requires.
fn carries_inline_test_body(source: &str) -> bool {
    let mut cfg_test_pending = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[") {
            cfg_test_pending |= trimmed == "#[cfg(test)]";
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let is_inline_mod =
            trimmed.starts_with("mod ") && trimmed.ends_with('{') && !trimmed.ends_with(';');
        if cfg_test_pending && is_inline_mod {
            return true;
        }
        cfg_test_pending = false;
    }
    false
}

#[test]
fn every_workspace_crate_keeps_a_discoverable_entrypoint() {
    let missing = workspace_crates()
        .iter()
        .filter(|crate_root| {
            !crate_root.join("src/lib.rs").is_file() && !crate_root.join("src/main.rs").is_file()
        })
        .map(|crate_root| repository_relative(crate_root))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "every crate needs a readable entrypoint (src/lib.rs or src/main.rs); missing: {missing:?}"
    );
}

#[test]
fn representations_keep_one_named_root_beside_the_entrypoint() {
    let observed = representation_source_roots()
        .iter()
        .filter(|src| sibling_roots(src).len() != 1)
        .map(|src| repository_relative(src.parent().expect("src has a parent")))
        .collect::<BTreeSet<_>>();
    let pinned = REPRESENTATION_ROOT_EXCEPTIONS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed, pinned,
        "a program representation keeps exactly one named root file beside lib.rs"
    );
}

#[test]
fn crate_sources_do_not_collect_model_directories() {
    let mut observed = BTreeSet::new();
    for crate_root in workspace_crates() {
        let src = crate_root.join("src");
        let mut pending = vec![src];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
            {
                let path = entry.expect("read src entry").path();
                if !path.is_dir() {
                    continue;
                }
                if path.file_name().is_some_and(|name| name == "model") {
                    observed.insert(repository_relative(&path));
                } else {
                    pending.push(path);
                }
            }
        }
    }
    let pinned = MODEL_DIRECTORY_EXCEPTIONS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed, pinned,
        "crate sources organize concept-owned areas; `model/` collects unrelated types"
    );
}

#[test]
fn test_families_stay_out_of_entrypoint_files() {
    let mut observed = BTreeSet::new();
    for crate_root in workspace_crates() {
        for entrypoint in ["lib.rs", "main.rs"] {
            let path = crate_root.join("src").join(entrypoint);
            if !path.is_file() {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            if carries_inline_test_body(&source) {
                observed.insert(repository_relative(&path));
            }
        }
    }
    let pinned = INLINE_TEST_ENTRYPOINTS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed, pinned,
        "test families live in named modules beside the entrypoint, not inline in it"
    );
}
