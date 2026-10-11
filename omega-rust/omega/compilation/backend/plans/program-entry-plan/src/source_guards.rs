//! Source guards on identity storage. Durable rows in this crate carry
//! `TerminalIdentityIndex` handles into a `TerminalIdentityTable`; text exists
//! only at declared boundaries. `ALLOWED` pins those boundary declarations by
//! exact trimmed line so a stored `pub` field can never share a parameter's
//! exemption.
#![cfg(test)]

use std::fs;
use std::path::Path;

/// Boundary declarations where identity text is legitimate: constructor
/// parameters that intern into a caller-supplied `TerminalIdentityTableBuilder`
/// on arrival.
const ALLOWED: &[(&str, &str)] = &[
    (
        "program_entry_physical/mod.rs",
        "requirement_identity: String,",
    ),
    (
        "program_entry_physical/mod.rs",
        "parameter_type_identities: Vec<String>,",
    ),
    (
        "program_entry_physical/mod.rs",
        "result_type_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "attachment_type_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "field_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "carrier_type_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "carrier_base_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "requirement_identity: String,",
    ),
    ("selected_entry.rs", "requirement_identity: String,"),
    // Validate-to-construct scratch carries the canonical requirement text
    // once; construction mints it into the contract record's table.
    (
        "optimized_semantic_entry/validation.rs",
        "pub(super) requirement_identity: String,",
    ),
];

/// Every `(file, line number, line)` of this crate's non-test sources, outside
/// line comments (this guard file is test-only and pins text needles).
fn source_lines() -> Vec<(String, usize, String)> {
    let mut directories = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
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
                || name == "source_guards.rs"
                || name == "tests.rs"
                || name.ends_with("_tests.rs")
            {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let file = path.display().to_string();
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
/// comments, that declares a text identity. `allowed` pairs are
/// `(file suffix, exact trimmed line)` so only the pinned declaration is
/// exempt — a `pub` field of the same name still violates.
fn violations(forbidden: &[&str], allowed: &[(&str, &str)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines() {
        for name in forbidden {
            if line.contains(name)
                && !allowed.iter().any(|(suffix, declaration)| {
                    file.ends_with(suffix) && line.trim() == *declaration
                })
            {
                violations.push(format!("{file}:{line_number}: {name}"));
            }
        }
    }
    violations.sort();
    violations
}

/// Stored `*identity`/`identities` declarations carry interned
/// `TerminalIdentityIndex` handles; text identity survives only on the
/// boundary declarations pinned in `ALLOWED`.
#[test]
fn crate_carries_no_string_identity_fields() {
    let violations = violations(
        &[
            concat!("identity", ": String"),
            concat!("identity", ": Option<String>"),
            concat!("identity", ": Vec<String>"),
            concat!("identities", ": Vec<String>"),
            concat!("identities", ": HashSet<String>"),
            concat!("identities", ": BTreeSet<String>"),
        ],
        ALLOWED,
    );
    assert!(
        violations.is_empty(),
        "durable rows must carry interned identity indices, not text: {violations:?}"
    );
}
