//! Source guards on provider-planning's layering boundary.
//!
//! Provider planning is an Omega realization consumer: typed vocabulary
//! reaches it only through checked custody (`checked_trees::typed_trees` and
//! the facade's module re-exports), never through the source-input crates
//! `typed-trees`, `typed-trees-to-checked-trees` or `validation`. The removed
//! manifest edges were the boundary's only enforcement until these tests:
//! the manifest guard fails the build if a banned dependency line returns,
//! and the source guard fails if typed vocabulary is spelled without the
//! `checked_trees` facade on the same line. Split needles keep the guards'
//! own source from matching forbidden spellings.

use std::fs;
use std::path::Path;

/// Every `(file, line number, line)` of this crate's non-test sources, outside
/// line comments.
fn source_lines() -> Vec<(String, usize, String)> {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut directories = vec![source_root.clone()];
    let mut lines = Vec::new();
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
                .path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if path.is_dir() {
                if !name.starts_with('.') && !matches!(name, "tests" | "examples" | "target") {
                    directories.push(path);
                }
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                || name == "tests.rs"
                || name.ends_with("_tests.rs")
            {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let file = path
                .strip_prefix(&source_root)
                .expect("scanned source stays within the crate")
                .display()
                .to_string();
            for (line_number, line) in source.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                lines.push((file.clone(), line_number + 1, line.to_owned()));
            }
        }
    }
    lines
}

/// Every `file:line: name` in this crate's non-test sources, outside line
/// comments, that names one of `forbidden`.
fn violations(forbidden: &[&str]) -> Vec<String> {
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines() {
        for name in forbidden {
            if line.contains(name) {
                violations.push(format!("{file}:{line_number}: {name}"));
            }
        }
    }
    violations.sort();
    violations
}

/// The crate may not re-declare the removed source-input dependency edges:
/// `typed-trees`, `typed-trees-to-checked-trees` and `validation` are
/// frontend-stage crates whose authority terminates at the checked product.
#[test]
fn manifest_declares_no_source_input_dependency_edges() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("manifest is readable beside the crate sources");
    let banned = [
        concat!("typed", "-trees"),
        concat!("typed", "-trees", "-to", "-checked", "-trees"),
        concat!("valida", "tion"),
    ];
    let mut violations = Vec::new();
    for (line_number, line) in manifest.lines().enumerate() {
        let entry = line.trim_start();
        if entry.starts_with('#') {
            continue;
        }
        for name in banned {
            if entry.starts_with(name) {
                violations.push(format!("Cargo.toml:{}: {name}", line_number + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "provider-planning must not depend on frontend source-input crates: {violations:?}"
    );
}

/// Typed vocabulary arrives through checked custody only: a source line
/// naming `typed_trees` must carry the `checked_trees` facade prefix on the
/// same line, so a returning direct dependency cannot hide behind a
/// partially-qualified path.
#[test]
fn typed_vocabulary_spells_through_checked_custody() {
    let typed = concat!("typed", "_trees");
    let facade = concat!("checked", "_trees");
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines() {
        if line.contains(typed) && !line.contains(facade) {
            violations.push(format!("{file}:{line_number}: {typed}"));
        }
    }
    violations.sort();
    assert!(
        violations.is_empty(),
        "typed vocabulary must arrive through the checked_trees facade: {violations:?}"
    );
}

/// The conversion stage between typed and checked trees is upstream of this
/// crate in both directions: no source may name it, and no manifest edge may
/// reintroduce it under its crate-name spelling.
#[test]
fn sources_never_name_the_conversion_stage() {
    let violations = violations(&[
        concat!("typed", "_trees", "_to", "_checked", "_trees"),
        concat!("typed", "-trees", "-to", "-checked", "-trees"),
    ]);
    assert!(
        violations.is_empty(),
        "provider-planning consumes the checked product, not the conversion stage: {violations:?}"
    );
}

#[test]
fn provider_realization_does_not_decode_source_selection_calls() {
    let violations = violations(&[
        concat!("fn ", "harvest_provider_selections("),
        concat!("fn ", "collect_executed_provider_selections("),
        concat!("fn ", "retain_provider_defaults("),
    ]);
    assert!(
        violations.is_empty(),
        "source decoding belongs to Psi: {violations:?}"
    );
}
