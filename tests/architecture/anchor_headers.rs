//! Every crate anchor carries a module header, and the list of anchors that do
//! not only ever gets shorter.
//!
//! The bound is four `//!` lines. That is a floor, not a quality measure: it
//! catches a new crate landing with no header at all, and it keeps the anchors
//! that predate it visible and countable. It cannot tell a header that states
//! something the code cannot from four lines that paraphrase the crate name,
//! and nothing here should be read as claiming otherwise. Raising the bound to
//! nine is the follow-up once the exception list drains -- 66 of the 109
//! anchors already clear nine today, so the bound is set where it separates
//! "documented badly" from "not documented", which is the distinction a line
//! count can actually make.
//!
//! An exception records the exact header length its anchor has right now.
//! Growing past the bound fails as a stale exception, and the entry is deleted;
//! shrinking fails as a regression. The list moves one direction.

use std::fs;
use std::path::{Path, PathBuf};

const MINIMUM_ANCHOR_HEADER_LINES: usize = 4;

/// Exact no-growth ratchets for crate anchors that predate the bound.
const ANCHOR_HEADER_EXCEPTIONS: &[(&str, usize)] = &[
    (
        "omega-rust/omega/backend/instruction_set_architectures/omega-x86-encoding/src/lib.rs",
        2,
    ),
    ("omega-rust/omega/backend/omega-layout/src/lib.rs", 0),
    (
        "omega-rust/omega/backend/omega-machine-emission/src/lib.rs",
        2,
    ),
    (
        "omega-rust/omega/backend/plans/omega-program-entry-plan/src/lib.rs",
        3,
    ),
    ("omega-rust/omega/build/omega-build-output/src/lib.rs", 1),
    (
        "omega-rust/omega/build/omega-package-compilation/src/lib.rs",
        1,
    ),
    ("omega-rust/omega/compiler/omega-compiler/src/lib.rs", 1),
    (
        "omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lib.rs",
        1,
    ),
    (
        "omega-rust/omega/representations/omega-calling-conventions/src/lib.rs",
        0,
    ),
    (
        "omega-rust/omega/representations/omega-machine-code/src/lib.rs",
        2,
    ),
    (
        "omega-rust/omega/representations/omega-target-operations/src/lib.rs",
        2,
    ),
    (
        "omega-rust/omega/representations/omega-target/src/lib.rs",
        0,
    ),
    ("omega-rust/psi/foundation/psi-arena/src/lib.rs", 1),
    ("omega-rust/psi/foundation/psi-diagnostics/src/lib.rs", 1),
    ("omega-rust/psi/foundation/psi-language-core/src/lib.rs", 1),
    ("omega-rust/psi/foundation/psi-numerics/src/lib.rs", 1),
    ("omega-rust/psi/foundation/psi-source/src/lib.rs", 1),
    ("omega-rust/psi/foundation/psi-symbols/src/lib.rs", 1),
    (
        "omega-rust/psi/pipeline/psi-source-files-to-tokens/src/lib.rs",
        1,
    ),
    (
        "omega-rust/psi/pipeline/psi-symbol-resolved-trees-to-typed-trees/src/lib.rs",
        1,
    ),
    (
        "omega-rust/psi/pipeline/psi-syntax-trees-to-symbol-resolved-trees/src/lib.rs",
        1,
    ),
    (
        "omega-rust/psi/pipeline/psi-tokens-to-syntax-trees/src/lib.rs",
        1,
    ),
    (
        "omega-rust/psi/pipeline/psi-typed-trees-to-checked-trees/src/lib.rs",
        0,
    ),
    (
        "omega-rust/psi/representations/psi-checked-trees/src/lib.rs",
        0,
    ),
    ("omega-rust/psi/representations/psi-effects/src/lib.rs", 1),
    ("omega-rust/psi/representations/psi-facts/src/lib.rs", 1),
    (
        "omega-rust/psi/representations/psi-symbol-resolved-trees/src/lib.rs",
        1,
    ),
    (
        "omega-rust/psi/representations/psi-syntax-trees/src/lib.rs",
        1,
    ),
    ("omega-rust/psi/representations/psi-tokens/src/lib.rs", 1),
    (
        "omega-rust/psi/representations/psi-typed-trees/src/lib.rs",
        1,
    ),
    (
        "omega-rust/psi/semantics/psi-build-time-evaluation/src/lib.rs",
        1,
    ),
    ("omega-rust/psi/semantics/psi-proof/src/lib.rs", 1),
    ("omega-rust/psi/semantics/psi-validation/src/lib.rs", 0),
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

/// Length of the leading `//!` run, skipping the inner attributes and ordinary
/// comments that sit above it.
///
/// The scan tracks bracket depth rather than stopping at the first line that is
/// not a comment, because `#![allow(...)]` spans several lines throughout this
/// tree and an earlier version of this count reported three real headers as
/// absent. Counting every `//!` in the file would instead pick up the ones
/// inside an inline `mod { }`.
fn anchor_header_lines(source: &str) -> usize {
    let mut header_lines = 0;
    let mut depth: i64 = 0;
    let mut inside_attribute = false;

    for line in source.lines() {
        let line = line.trim();

        if inside_attribute {
            depth += bracket_balance(line);
            inside_attribute = depth > 0;
            continue;
        }

        if line.starts_with("//!") {
            header_lines += 1;
        } else if line.is_empty() || line.starts_with("//") {
            continue;
        } else if line.starts_with("#!") {
            depth = bracket_balance(line);
            inside_attribute = depth > 0;
        } else {
            break;
        }
    }

    header_lines
}

fn bracket_balance(line: &str) -> i64 {
    line.chars().fold(0, |balance, character| match character {
        '(' | '[' => balance + 1,
        ')' | ']' => balance - 1,
        _ => balance,
    })
}

fn crate_anchors(root: &Path) -> Vec<PathBuf> {
    let mut anchors = Vec::new();
    collect_crate_anchors(&root.join("omega-rust"), &mut anchors);
    anchors.sort();
    anchors
}

fn collect_crate_anchors(directory: &Path, anchors: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));

    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read entry under {}: {error}", directory.display()))
            .path();

        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_crate_anchors(&path, anchors);
        } else if path.ends_with("src/lib.rs") {
            anchors.push(path);
        }
    }
}

fn relative_anchor(root: &Path, anchor: &Path) -> String {
    anchor
        .strip_prefix(root)
        .expect("anchor lives beneath the workspace root")
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn every_crate_anchor_carries_a_header() {
    let root = workspace_root();
    let mut undocumented = Vec::new();
    let mut regressed = Vec::new();

    for anchor in crate_anchors(&root) {
        let relative = relative_anchor(&root, &anchor);
        let source = fs::read_to_string(&anchor)
            .unwrap_or_else(|error| panic!("read {}: {error}", anchor.display()));
        let header_lines = anchor_header_lines(&source);

        match ANCHOR_HEADER_EXCEPTIONS
            .iter()
            .find_map(|(exception, recorded)| (*exception == relative).then_some(*recorded))
        {
            Some(recorded) if header_lines < recorded => regressed.push(format!(
                "{relative} fell from {recorded} to {header_lines} header lines"
            )),
            Some(_) => {}
            None if header_lines < MINIMUM_ANCHOR_HEADER_LINES => undocumented.push(format!(
                "{relative} has {header_lines} header lines, below {MINIMUM_ANCHOR_HEADER_LINES}"
            )),
            None => {}
        }
    }

    assert!(
        regressed.is_empty(),
        "an exempt anchor lost header lines instead of gaining them:\n{}",
        regressed.join("\n")
    );
    assert!(
        undocumented.is_empty(),
        "a crate anchor must open with at least {MINIMUM_ANCHOR_HEADER_LINES} `//!` lines stating something the code cannot; add one rather than adding an exception:\n{}",
        undocumented.join("\n")
    );
}

#[test]
fn anchor_header_exceptions_are_still_needed() {
    let root = workspace_root();
    let mut stale = Vec::new();

    for (relative, recorded) in ANCHOR_HEADER_EXCEPTIONS {
        assert!(
            *recorded < MINIMUM_ANCHOR_HEADER_LINES,
            "{relative} records {recorded} header lines, which already meets the bound"
        );

        let anchor = root.join(relative);
        assert!(
            anchor.exists(),
            "exception names an anchor that no longer exists: {relative}"
        );

        let source = fs::read_to_string(&anchor)
            .unwrap_or_else(|error| panic!("read exception {relative}: {error}"));
        let header_lines = anchor_header_lines(&source);

        if header_lines >= MINIMUM_ANCHOR_HEADER_LINES {
            stale.push(format!("{relative} now has {header_lines} header lines"));
        }
    }

    assert!(
        stale.is_empty(),
        "documented anchors must leave the exception list:\n{}",
        stale.join("\n")
    );
}
