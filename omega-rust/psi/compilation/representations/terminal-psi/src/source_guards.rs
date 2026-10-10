//! Source guards on terminal-module identity storage. Terminal Psi stores
//! program-owned identities once in `TerminalIdentityTable`; durable rows
//! carry `TerminalIdentityIndex` handles and render text through
//! `identities.text(..)` only at report boundaries. Split needles keep the
//! guards' own source from matching forbidden declarations.

use std::fs;
use std::path::Path;

/// Every `(file, line number, line)` of this crate's non-test sources, outside
/// line comments.
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

/// Terminal-module rows carry interned `TerminalIdentityIndex` handles; a
/// `*identity`/`identities` field declared as text bypasses the table and
/// breaks the single-storage contract the codec relies on.
#[test]
fn terminal_module_carries_no_string_identity_fields() {
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
        "terminal rows must carry interned identity indices, not text: {violations:?}"
    );
}
