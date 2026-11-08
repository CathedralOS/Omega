// Layering boundary guards for `provider-planning`. Verifies that `Cargo.toml`
// declares no direct frontend source-input dependency edges (`typed-trees`,
// `typed-trees-to-checked-trees`, `validation`), that every source line naming
// `checked_trees::typed_trees` spells it through the `checked_trees` facade on
// the same line, and that source selection decoding stays in Psi.

use std::fs;
use std::path::Path;

use source_audit::{source_lines, violations};

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

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

#[test]
fn typed_vocabulary_spells_through_checked_custody() {
    let typed = concat!("typed", "_trees");
    let facade = concat!("checked", "_trees");
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines(CRATE_ROOT, "") {
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

#[test]
fn sources_never_name_the_conversion_stage() {
    let violations = violations(
        CRATE_ROOT,
        "",
        &[
            concat!("typed", "_trees", "_to", "_checked", "_trees"),
            concat!("typed", "-trees", "-to", "-checked", "-trees"),
        ],
        &[],
    );
    assert!(
        violations.is_empty(),
        "provider-planning consumes the checked product, not the conversion stage: {violations:?}"
    );
}

#[test]
fn provider_realization_does_not_decode_source_selection_calls() {
    let violations = violations(
        CRATE_ROOT,
        "",
        &[
            concat!("fn ", "harvest_provider_selections("),
            concat!("fn ", "collect_executed_provider_selections("),
            concat!("fn ", "retain_provider_defaults("),
        ],
        &[],
    );
    assert!(
        violations.is_empty(),
        "source decoding belongs to Psi: {violations:?}"
    );
}
