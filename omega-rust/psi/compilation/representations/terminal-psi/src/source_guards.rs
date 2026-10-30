//! Source guards on terminal-module identity storage. Terminal Psi stores
//! program-owned identities once in `TerminalIdentityTable`; durable rows
//! carry `TerminalIdentityIndex` handles and render text through
//! `identities.text(..)` only at report boundaries. The scan itself is shared
//! (`source_audit`); this file owns only its needles and its boundary
//! exemptions.
#![cfg(test)]

use source_audit::violations;

/// The crate whose sources these guards pin.
const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// No boundary declarations: every identity in this crate is interned.
const ALLOWED: &[(&str, &str)] = &[];

/// Terminal-module rows carry interned `TerminalIdentityIndex` handles; a
/// `*identity`/`identities` field declared as text bypasses the table and
/// breaks the single-storage contract the codec relies on.
#[test]
fn terminal_module_carries_no_string_identity_fields() {
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
        "terminal rows must carry interned identity indices, not text: {violations:?}"
    );
}
