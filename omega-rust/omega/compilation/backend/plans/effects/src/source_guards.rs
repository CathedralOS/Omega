//! Source guards on provider-plan identity storage. Provider plans and
//! schemas store program-owned identities once in `ProviderIdentityTable`;
//! interned rows carry `ProviderIdentityId` handles and render text through
//! `identities.text(..)` / `identity_text(..)` only at report and codec
//! boundaries. Neutral declaration storage in Psi follows the same contract.
//! The forbidden needles are assembled at runtime so this file never contains
//! them verbatim. The scan itself is shared (`source_audit`); this file owns
//! only its needles, its roots and its boundary exemptions.

use source_audit::{source_lines, violations};

/// The crate whose sources these guards pin.
const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// The neutral declaration moved to Psi; its storage guards must follow it.
fn service_surface_owner() -> String {
    std::path::Path::new(CRATE_ROOT)
        .join("../../../../../psi/foundation/language-semantics")
        .to_string_lossy()
        .into_owned()
}

/// The `field: <shape>` declaration needles `field` must never carry.
fn text_shapes(field: &str) -> Vec<String> {
    [": String", ": Option<String>", ": Vec<String>"]
        .iter()
        .map(|shape| format!("{field}{shape}"))
        .collect()
}

/// The interned fields must be declared with `ProviderIdentityId` handles;
/// the guard requires each declaration so a revert to text cannot pass
/// silently.
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

/// No provider type may redeclare `trait_name` or `machine_identity` as
/// text; those names are interned identities.
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

/// `requirement_identity` is interned on `ServiceMethod` and
/// `ServiceProgressEstablishmentRoute`. Scan their new Psi owner as well as
/// Omega so moving a declaration cannot make the guard pass vacuously.
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

/// Boundary declarations where identity text is legitimate: constructor
/// parameters interning on arrival and the authored-exclusion `Service`
/// payload, which carries the selected declaration's hermetic identity as
/// text until a Terminal module's catalog resolves it.
const ALLOWED: &[(&str, &str)] = &[(
    "behavior_exclusions/authored.rs",
    "Service { identity: String, name: String },",
)];

/// Stored `*identity`/`identities` declarations carry interned
/// `TerminalIdentityIndex` handles; text identity survives only on the
/// boundary declarations pinned in `ALLOWED`.
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
