/*
We guard provider-plan identity storage so provider plans and service schemas
store program-owned identities once in `ProviderIdentityTable` and reference
them via `ProviderIdentityId` or `TerminalIdentityIndex` handles rather than
raw strings. We assemble forbidden declaration needles at runtime so this test
file never contains them verbatim.
*/

use source_audit::{source_lines, violations};

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn service_surface_owner() -> String {
    std::path::Path::new(CRATE_ROOT)
        .join("../../../../../psi/foundation/language-semantics")
        .to_string_lossy()
        .into_owned()
}

fn text_shapes(field: &str) -> Vec<String> {
    [": String", ": Option<String>", ": Vec<String>"]
        .iter()
        .map(|shape| format!("{field}{shape}"))
        .collect()
}

#[test]
fn interned_fields_hold_provider_identity_ids() {
    let mut missing = Vec::new();
    let mut declarations = source_lines(CRATE_ROOT, "provider_evidence/providers");
    declarations.extend(source_lines(&service_surface_owner(), "service_surface.rs"));
    for needle in [
        format!("trait_name: {}", "ProviderIdentityId"),
        format!("requirement_identity: {}", "ProviderIdentityId"),
        format!("machine_identity: {}", "ProviderIdentityId"),
    ] {
        if !declarations
            .iter()
            .any(|(_, _, line)| line.contains(needle.as_str()))
        {
            missing.push(needle);
        }
    }
    assert!(
        missing.is_empty(),
        "provider rows lost their interned identity declarations: {missing:?}"
    );
}

#[test]
fn provider_types_carry_no_text_trait_or_machine_identities() {
    let mut forbidden = text_shapes("trait_name");
    forbidden.extend(text_shapes("machine_identity"));
    let mut findings = violations(CRATE_ROOT, "provider_evidence/providers", &forbidden, &[]);
    findings.extend(violations(
        &service_surface_owner(),
        "service_surface.rs",
        &forbidden,
        &[],
    ));
    let violations = findings;
    assert!(
        violations.is_empty(),
        "provider types must carry interned identity ids, not text: {violations:?}"
    );
}

#[test]
fn service_schemas_carry_no_text_requirement_identities() {
    let mut findings = violations(
        CRATE_ROOT,
        "provider_evidence/providers/provider_plan/service_schemas.rs",
        &text_shapes("requirement_identity"),
        &[],
    );
    findings.extend(violations(
        &service_surface_owner(),
        "service_surface.rs",
        &text_shapes("requirement_identity"),
        &[],
    ));
    let violations = findings;
    assert!(
        violations.is_empty(),
        "service schemas must carry interned identity ids, not text: {violations:?}"
    );
}

const ALLOWED: &[(&str, &str)] = &[(
    "provider_evidence/executable_scopes.rs",
    "Service { identity: String, name: String },",
)];

#[test]
fn crate_carries_no_string_identity_fields() {
    let forbidden = [
        concat!("identity", ": String"),
        concat!("identity", ": Option<String>"),
        concat!("identity", ": Vec<String>"),
        concat!("identities", ": Vec<String>"),
        concat!("identities", ": HashSet<String>"),
        concat!("identities", ": BTreeSet<String>"),
    ];
    let mut findings = violations(CRATE_ROOT, "", &forbidden, ALLOWED);
    findings.extend(violations(
        &service_surface_owner(),
        "service_surface.rs",
        &forbidden,
        &[],
    ));
    let violations = findings;
    assert!(
        violations.is_empty(),
        "durable rows must carry interned identity indices, not text: {violations:?}"
    );
}
