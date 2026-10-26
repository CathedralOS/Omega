//! Source guards on identity storage. Durable rows in this crate carry
//! `TerminalIdentityIndex` handles into a `TerminalIdentityTable`; text exists
//! only at declared boundaries. `ALLOWED` pins those boundary declarations by
//! exact trimmed line so a stored `pub` field can never share a parameter's
//! exemption. The scan itself is shared (`source_audit`); this file owns only
//! its needles and its boundary exemptions.
#![cfg(test)]

use source_audit::violations;

/// The crate whose sources these guards pin.
const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// No boundary declarations: every identity in this crate is interned.
const ALLOWED: &[(&str, &str)] = &[];

/// Stored `*identity`/`identities` declarations carry interned
/// `TerminalIdentityIndex` handles; text identity survives only on the
/// boundary declarations pinned in `ALLOWED`.
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
