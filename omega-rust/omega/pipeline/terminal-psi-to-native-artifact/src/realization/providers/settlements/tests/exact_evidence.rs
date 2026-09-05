use super::*;

#[test]
fn compact_report_claim_cannot_substitute_different_exact_import_plan() {
    let selected_plan = import_plan(b"selected_leaf", target::TargetProfile::LinuxX64);
    let substituted = import_plan(b"substituted_leaf", target::TargetProfile::LinuxX64);
    let report_identity = selected_plan.report_fingerprint();
    let selected =
        effects::SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan.clone()])
            .unwrap();
    assert!(
        selected_plan_from_exact_evidence(
            &selected,
            report_identity,
            &substituted,
            "omega::test::Foreign::leaf()",
        )
        .is_err()
    );
    assert_eq!(
        selected_plan_from_exact_evidence(
            &selected,
            report_identity,
            &selected_plan,
            "omega::test::Foreign::leaf()",
        )
        .unwrap(),
        &selected_plan
    );
}
