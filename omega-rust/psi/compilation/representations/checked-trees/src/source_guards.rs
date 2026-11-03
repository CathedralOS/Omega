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

macro_rules! row {
    ($a:expr => $b:expr) => {
        ($a, $b)
    };
}

/// Boundary declarations where identity text is a wire payload: lowering only
/// interns it into the Terminal identity table, and no checker or lowering
/// step compares, keys or dispatches on the text itself.
const ALLOWED: &[(&str, &str)] = &[
    // Provider instance publication: checking renders the callable identities
    // once; lowering interns each into `RequiredProviderInstance` rows.
    row!("facts/provider_instance_publication.rs" => "pub template_identity: String,"),
    row!("facts/provider_instance_publication.rs" => "pub source_candidate_identity: String,"),
    row!("facts/provider_instance_publication.rs" => "pub requirement_identity: String,"),
    row!("facts/provider_instance_publication.rs" => "pub candidate_identity: String,"),
    // Declaration ABI row: lowering interns it into `CallingAbiDeclaration`.
    row!("terminal/calling_signatures.rs" => "pub requirement_identity: String,"),
    // Native layout provenance: lowering interns it into the
    // `NativeLayoutCatalog`; plans are keyed by `TypeIdentityId`, and the text
    // takes part only in whole-plan equality when duplicate plans must agree.
    row!("terminal/structural_type_plans.rs" => "pub native_identity: Option<String>,"),
    row!("terminal/structural_type_plans.rs" => "pub native_policy_identity: Option<String>,"),
    row!("terminal/structural_type_plans.rs" => "pub native_semantic_identity: Option<String>,"),
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
