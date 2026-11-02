//! Read the authored public call through executed Build and ordinary Check.
//! The invalid twin changes only its runtime index; setup failure cannot stand
//! in for the fact-sensitive refusal.

use super::fixture_package_inputs;

#[test]
fn selected_requirement_preserves_runtime_index_and_explicit_evidence() {
    let target = "macos_arm64";
    let mut failures = Vec::new();
    for (fixture, rejection) in [
        (
            "tests/omega/pass/facts/selected_value_application_through_target_body_exit",
            None,
        ),
        (
            "tests/omega/fail/facts/selected_value_application_through_target_body_rejected",
            Some("local `difference` may overflow `u32`"),
        ),
        (
            "samples/cli/facts/selected_value_application/probes/two_evidence",
            None,
        ),
        (
            "samples/cli/facts/selected_value_application/probes/unused_evidence",
            None,
        ),
        (
            "samples/cli/facts/selected_value_application/probes/default_evidence",
            None,
        ),
        (
            "samples/cli/facts/selected_value_application/probes/two_evidence/invalid_first",
            Some("expected a complete `Choose` implementation"),
        ),
        (
            "samples/cli/facts/selected_value_application/probes/two_evidence/missing_second",
            Some("static"),
        ),
        (
            "samples/cli/facts/selected_value_application/probes/unused_evidence/invalid_provider_bound",
            Some("does not refine requirement"),
        ),
        (
            "samples/cli/facts/selected_value_application/probes/wrong_evidence_argument",
            Some("expected a complete `Choose` implementation"),
        ),
        (
            "samples/cli/facts/selected_value_application/probes/wrong_evidence_trait",
            Some("expected a complete `Choose` implementation"),
        ),
    ] {
        let source = fixture_package_inputs::repo_root()
            .join(fixture)
            .join("main.omg");
        let result = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &source,
            Some(target),
        )
        .and_then(|mut prepared| {
            fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        });
        match result {
            Ok(_) if rejection.is_none() => eprintln!("SOURCE_CHECK {fixture}: accepted"),
            Err(diagnostics)
                if rejection.is_some_and(|fragment| {
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains(fragment))
                }) =>
            {
                eprintln!("SOURCE_CHECK {fixture}: intended refusal {diagnostics:?}");
            }
            Ok(_) => failures.push(format!(
                "{fixture}: invalid static or runtime obligation accepted"
            )),
            Err(diagnostics) => failures.push(format!("{fixture}: {diagnostics:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn public_requirement_evidence_roundtrips_with_its_exact_bound() {
    let target = "macos_arm64";
    let fixture = fixture_package_inputs::repo_root()
        .join("samples/cli/facts/selected_value_application/probes/public_evidence/main.omg");
    let mut prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("public evidence source admission");
    let checked = fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        .expect("checked public evidence source");
    let profile = target::TargetProfile::from_canonical_target_name(target).unwrap();
    let package = checked.sources().package_identity().expect("root package");
    let policy = package_evidence::project_checked_package_policy(&checked, profile, package)
        .expect("public evidence policy");
    let selected = policy
        .public_traits()
        .iter()
        .find(|declaration| declaration.identity().path().ends_with("SelectedValue"))
        .expect("public SelectedValue");
    let requirement = &selected.requirements()[0];
    assert_eq!(requirement.type_parameters().len(), 2);
    assert_eq!(requirement.conformance_bounds().len(), 1);
    let bound = &requirement.conformance_bounds()[0];
    assert_eq!(bound.binder_ordinal(), 0);
    assert!(bound.trait_identity().path().ends_with("Choose"));
    assert_eq!(bound.arguments().len(), 1);
    assert!(bound.arguments()[0].canonical().contains("Cell"));
    let bytes = policy.canonical_bytes().expect("encode evidence policy");
    let recovered = package_evidence::record::PackagePolicyBaseline::recover_canonical(
        &bytes,
        package_evidence::encoding::PackagePolicyRecoveryLimits::default(),
    )
    .expect("recover source-free evidence policy");
    let recovered_requirement = &recovered
        .public_traits()
        .iter()
        .find(|declaration| declaration.identity().path().ends_with("SelectedValue"))
        .expect("recovered SelectedValue")
        .requirements()[0];
    assert_eq!(
        recovered_requirement.conformance_bounds(),
        requirement.conformance_bounds()
    );
    assert_eq!(
        recovered
            .canonical_bytes()
            .expect("re-encode evidence policy"),
        bytes
    );
}
