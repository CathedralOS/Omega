/*
We audit our crate source files to ensure durable rows store interned
`TerminalIdentityIndex` handles rather than raw `String` identity fields. We
keep `ALLOWED` empty because every identity in this crate is interned.
*/
#![cfg(test)]

use source_audit::violations;

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

const ALLOWED: &[(&str, &str)] = &[];

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
