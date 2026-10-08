//! Source guards on what lowering may read. Lowering consumes stage 04's
//! plans and checked facts; it never re-reads the authored program or
//! re-enters checking to reconstruct a decision checking already recorded.
//! Split needles keep the guards' own source from matching forbidden names.

use std::fs;
use std::path::Path;

/// Every `file:line: name` in this crate's non-test sources, outside line
/// comments, that names one of `forbidden`.
fn violations(forbidden: &[&str]) -> Vec<String> {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut directories = vec![source_root.clone()];
    let mut violations = Vec::new();
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
            for (line_number, line) in source.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for name in forbidden {
                    if line.contains(name) {
                        violations.push(format!(
                            "{}:{}: {name}",
                            path.strip_prefix(&source_root)
                                .expect("scanned source stays within the crate")
                                .display(),
                            line_number + 1,
                        ));
                    }
                }
            }
        }
    }
    violations.sort();
    violations
}

#[test]
fn lowering_reads_checked_type_facts_without_reconstruction() {
    let violations = violations(&[
        concat!("declared_place_", "type_raw"),
        concat!("expression_result_", "type_reference"),
        concat!("unwrapped_", "type_reference"),
    ]);
    assert!(
        violations.is_empty(),
        "lowering must read checked place/result/carrier facts: {violations:?}"
    );
}

/// A value lowering needs from the authored program is recorded in a stage
/// 04 plan row or checked fact (a state's `let` locals are
/// `CheckFacts::state_locals`); reference custody is the recorded
/// `CheckFacts::reference_custody`, read through `crate::reference_custody`.
#[test]
fn lowering_reads_no_authored_statements_or_expressions() {
    let violations = violations(&[
        concat!("statement_", "table"),
        concat!("expression_", "table"),
        concat!("Statement", "Node::"),
        concat!("Expression", "Node::"),
        concat!("reference_result_", "custody"),
    ]);
    assert!(
        violations.is_empty(),
        "lowering must read plan rows and checked facts, not authored source: {violations:?}"
    );
}
