/*
We enforce that durable records in this crate store interned
`TerminalIdentityIndex` handles rather than raw `String` identity fields,
pinning the exact constructor parameter declarations where incoming text is
interned on arrival.
*/

#![cfg(test)]

use source_audit::violations;

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

macro_rules! row {
    ($a:expr => $b:expr) => {
        ($a, $b)
    };
}

const ALLOWED: &[(&str, &str)] = &[
    row!("program_entry_physical.rs" => "requirement_identity: String,"),
    row!("program_entry_physical.rs" => "parameter_type_identities: Vec<String>,"),
    row!("program_entry_physical.rs" => "result_type_identity: String,"),
    row!("selected_entry/service_establishment.rs" => "attachment_type_identity: String,"),
    row!("selected_entry/service_establishment.rs" => "field_identity: String,"),
    row!("selected_entry/service_establishment.rs" => "carrier_type_identity: String,"),
    row!("selected_entry/service_establishment.rs" => "carrier_base_identity: String,"),
    row!("selected_entry/service_establishment.rs" => "requirement_identity: String,"),
    ("selected_entry.rs", "requirement_identity: String,"),
];

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
