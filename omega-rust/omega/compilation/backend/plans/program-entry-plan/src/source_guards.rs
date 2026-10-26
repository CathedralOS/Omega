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

/// Boundary declarations where identity text is legitimate: constructor
/// parameters that intern into a caller-supplied `TerminalIdentityTableBuilder`
/// on arrival.
const ALLOWED: &[(&str, &str)] = &[
    (
        "program_entry_physical/mod.rs",
        "requirement_identity: String,",
    ),
    (
        "program_entry_physical/mod.rs",
        "parameter_type_identities: Vec<String>,",
    ),
    (
        "program_entry_physical/mod.rs",
        "result_type_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "attachment_type_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "field_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "carrier_type_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "carrier_base_identity: String,",
    ),
    (
        "selected_entry/service_establishment.rs",
        "requirement_identity: String,",
    ),
    ("selected_entry.rs", "requirement_identity: String,"),
    // Validate-to-construct scratch carries the canonical requirement text
    // once; construction mints it into the contract record's table.
    (
        "optimized_semantic_entry/validation.rs",
        "pub(super) requirement_identity: String,",
    ),
];

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
