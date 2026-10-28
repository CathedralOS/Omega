//! Source guards on checked-tree identity storage. Checking records each
//! program-owned identity once as a handle — a `CallableIdentityId`,
//! `TypeIdentityId`, `CheckedMemberIdentity`, or existing symbol handle — and
//! no checked plan or fact may carry a `String`-typed identity field: text
//! identities are a wire payload rendered at the Terminal Psi boundary, not
//! internal identity. The scan itself is shared (`source_audit`); this file
//! owns only its needles and its boundary exemptions.
#![cfg(test)]

use source_audit::violations;

/// The crate whose sources these guards pin.
const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// No boundary declarations: every identity in this crate is interned.
const ALLOWED: &[(&str, &str)] = &[];

/// Checked plans and facts carry program-owned identities as handles; any
/// `*identity`/`identities` field declared as text is a checker decision
/// recorded as a wire payload instead of an interned key.
#[test]
fn checked_trees_carry_no_string_identity_fields() {
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
        "checked trees must carry identity handles, not text identities: {violations:?}"
    );
}
