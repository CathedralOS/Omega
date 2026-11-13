// Source-selection decoding stays in Psi and no conversion-stage implementation
// belongs in provider metadata. Typed metadata uses its actual owner directly.
use source_audit::violations;
const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[test]
fn sources_never_name_the_conversion_stage() {
    let violations = violations(
        CRATE_ROOT,
        "",
        &[
            concat!("typed", "_trees", "_to", "_checked", "_trees"),
            concat!("typed", "-trees", "-to", "-checked", "-trees"),
        ],
        &[],
    );
    assert!(
        violations.is_empty(),
        "provider-planning consumes the checked product, not the conversion stage: {violations:?}"
    );
}

#[test]
fn provider_realization_does_not_decode_source_selection_calls() {
    let violations = violations(
        CRATE_ROOT,
        "",
        &[
            concat!("fn ", "harvest_provider_selections("),
            concat!("fn ", "collect_executed_provider_selections("),
            concat!("fn ", "retain_provider_defaults("),
        ],
        &[],
    );
    assert!(
        violations.is_empty(),
        "source decoding belongs to Psi: {violations:?}"
    );
}
