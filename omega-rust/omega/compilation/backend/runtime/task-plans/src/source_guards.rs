/*
We verify that durable rows in `task-plans` never import source/typed/checked
IR carriers and always store interned `TerminalIdentityIndex` handles rather
than raw `String` identities.
*/

#![cfg(test)]

use source_audit::violations;

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

const ALLOWED: &[(&str, &str)] = &[];

#[test]
fn activation_join_consumes_no_source_ir() {
    let found = violations(
        CRATE_ROOT,
        "",
        &[
            concat!("checked", "_trees::"),
            concat!("typed", "_trees::"),
            concat!("TypeReference", "Handle"),
            concat!("Symbol", "Handle"),
        ],
        &[],
    );
    assert!(
        found.is_empty(),
        "task realization reopened source: {found:?}"
    );
}

#[test]
fn crate_carries_no_string_identity_fields() {
    let violations = violations(
        CRATE_ROOT,
        "",
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
