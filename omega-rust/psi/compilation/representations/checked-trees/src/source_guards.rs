//! Source guards on checked-tree identity storage. Checking records each
//! program-owned identity once as a handle — a `CallableIdentityId`,
//! `TypeIdentityId`, `CheckedMemberIdentity`, or existing symbol handle — so a
//! checked plan or fact compares and selects on handles, never on text. Text
//! exists only at declared boundaries: fields whose text lowering only interns
//! into the Terminal identity table and that no checker or lowering step
//! compares or selects on. `ALLOWED` pins those boundary declarations by exact
//! trimmed line. The scan itself is shared (`source_audit`); this file owns
//! only its needles and its boundary exemptions.
#![cfg(test)]

use source_audit::violations;

/// The crate whose sources these guards pin.
const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Boundary declarations where identity text is a wire payload that lowering
/// only interns into the Terminal identity table.
const ALLOWED: &[(&str, &str)] = &[
    // Declaration ABI row: the text is copied from the typed declaration and
    // lowering interns it into `CallingAbiDeclaration`.
    (
        "terminal/calling_signatures.rs",
        "pub requirement_identity: String,",
    ),
    // Native layout provenance: lowering interns it into the
    // `NativeLayoutCatalog`. Plans are keyed and selected by `TypeIdentityId`;
    // the text takes part only in whole-plan equality, where
    // `retain_declarations` and `adopt_structural_types` reject a duplicate
    // plan that disagrees with the one already recorded.
    (
        "terminal/structural_type_plans.rs",
        "pub native_identity: Option<String>,",
    ),
    (
        "terminal/structural_type_plans.rs",
        "pub native_policy_identity: Option<String>,",
    ),
    (
        "terminal/structural_type_plans.rs",
        "pub native_semantic_identity: Option<String>,",
    ),
];

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
