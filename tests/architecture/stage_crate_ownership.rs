//! Stage-crate ownership audit for both pipeline halves.
//!
//! `omega-rust/{psi,omega}/pipeline/` may only contain `X-to-Y` transform
//! crates and `X-to-X` selected-optimization crates. Each must expose a
//! non-empty `src/lib.rs`; its input must be produced by another transform or
//! be the `source-files` boundary; its output must be consumed by a follow-on
//! stage or be a documented hand-off terminal. `omega-rust/pipeline.md` is the
//! ownership table: it must link every stage crate through an entrypoint file
//! that exists, and no stage crate may go undocumented.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Stage outputs that leave `pipeline/` for a documented owner instead of a
/// follow-on `Y-to-*` stage. The terminal artifact hands off to
/// `native-realization`; the resolved layout hands off to `machine-emission`.
const HANDOFF_TERMINALS: [(&str, &str); 2] = [
    (
        "terminal-artifact",
        "omega-rust/omega/compiler/native-realization",
    ),
    (
        "resolved-layout",
        "omega-rust/omega/backend/machine-emission",
    ),
];

/// Inputs admitted without a producing stage: the pipeline's source boundary.
const SOURCE_BOUNDARIES: [&str; 1] = ["source-files"];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

fn stage_crates(root: &Path) -> Vec<(String, PathBuf)> {
    let mut crates = Vec::new();
    for half in ["psi", "omega"] {
        let directory = root.join("omega-rust").join(half).join("pipeline");
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.unwrap().path();
            if path.is_dir() && path.join("Cargo.toml").is_file() {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                crates.push((name, path));
            }
        }
    }
    crates.sort();
    crates
}

fn transform_pair(name: &str) -> (&str, &str) {
    let (from, to) = name
        .split_once("-to-")
        .unwrap_or_else(|| panic!("stage crate {name} does not name an X-to-Y transform"));
    assert!(
        !from.is_empty() && !to.is_empty(),
        "stage crate {name} has an empty transform side"
    );
    assert!(
        !to.contains("-to-"),
        "stage crate {name} encodes more than one transform"
    );
    (from, to)
}

fn markdown_links(document: &str) -> Vec<&str> {
    let mut links = Vec::new();
    let mut rest = document;
    while let Some(open) = rest.find("](") {
        rest = &rest[open + 2..];
        match rest.find(')') {
            Some(close) => {
                links.push(&rest[..close]);
                rest = &rest[close + 1..];
            }
            None => break,
        }
    }
    links
}

#[test]
fn stage_crates_name_unique_transforms_with_real_entrypoints() {
    let root = repository();
    let crates = stage_crates(&root);
    assert!(
        crates.len() >= 10,
        "stage-crate inventory collapsed to {}",
        crates.len()
    );
    let mut pairs = BTreeSet::new();
    for (name, path) in &crates {
        let pair = transform_pair(name);
        assert!(
            pairs.insert(pair),
            "competing stage owners for {} -> {}",
            pair.0,
            pair.1
        );
        let library = path.join("src/lib.rs");
        assert!(library.is_file(), "{name} has no src/lib.rs entrypoint");
        assert!(
            !std::fs::read_to_string(&library).unwrap().trim().is_empty(),
            "{name} lib.rs is an empty stub"
        );
    }
}

#[test]
fn stage_chain_has_no_orphan_inputs_or_outputs() {
    let root = repository();
    let crates = stage_crates(&root);
    let pairs: Vec<(&str, &str)> = crates
        .iter()
        .map(|(name, _)| transform_pair(name))
        .collect();
    let produced: BTreeSet<&str> = pairs
        .iter()
        .filter(|(from, to)| from != to)
        .map(|(_, to)| *to)
        .collect();
    let consumed: BTreeSet<&str> = pairs.iter().map(|(from, _)| *from).collect();
    for (from, to) in &pairs {
        assert!(
            produced.contains(from) || SOURCE_BOUNDARIES.contains(from),
            "stage input {from} of {from}-to-{to} has no producing transform and \
             is not a documented source boundary"
        );
        assert!(
            consumed.contains(to) || HANDOFF_TERMINALS.iter().any(|(t, _)| t == to),
            "stage output {to} of {from}-to-{to} is orphaned — no successor stage \
             and no documented hand-off"
        );
    }
    for (terminal, owner) in HANDOFF_TERMINALS {
        assert!(
            root.join(owner).join("Cargo.toml").is_file(),
            "hand-off owner {owner} for stage output {terminal} is missing"
        );
    }
}

#[test]
fn pipeline_ownership_document_links_every_stage_crate() {
    let root = repository();
    let crates = stage_crates(&root);
    let document = std::fs::read_to_string(root.join("omega-rust/pipeline.md")).unwrap();
    let mut linked = BTreeSet::new();
    for link in markdown_links(&document) {
        if !(link.starts_with("psi/pipeline/") || link.starts_with("omega/pipeline/")) {
            continue;
        }
        assert!(
            root.join("omega-rust").join(link).is_file(),
            "pipeline.md links a missing stage entrypoint: {link}"
        );
        linked.insert(link.split('/').nth(2).unwrap().to_owned());
    }
    let on_disk: BTreeSet<String> = crates.iter().map(|(name, _)| name.clone()).collect();
    assert_eq!(
        linked, on_disk,
        "pipeline.md stage-crate links differ from the on-disk pipeline crates"
    );
}
