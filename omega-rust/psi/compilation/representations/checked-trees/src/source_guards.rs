//! Source guards on checked-tree identity storage. Checking records each
//! program-owned identity once as a handle — a `CallableIdentityId`,
//! `TypeIdentityId`, `CheckedMemberIdentity`, or existing symbol handle — and
//! no checked plan or fact may carry a `String`-typed identity field: text
//! identities are a wire payload rendered at the Terminal Psi boundary, not
//! internal identity. Split needles keep the guards' own source from matching
//! forbidden spellings.

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

/// Checked plans and facts carry program-owned identities as handles; any
/// `*identity`/`identities` field declared as text is a checker decision
/// recorded as a wire payload instead of an interned key.
#[test]
fn checked_trees_carry_no_string_identity_fields() {
    let violations = violations(&[
        concat!("identity", ": String"),
        concat!("identity", ": Option<String>"),
        concat!("identity", ": Vec<String>"),
        concat!("identities", ": Vec<String>"),
        concat!("identities", ": HashSet<String>"),
        concat!("identities", ": BTreeSet<String>"),
    ]);
    assert!(
        violations.is_empty(),
        "checked trees must carry identity handles, not text identities: {violations:?}"
    );
}
