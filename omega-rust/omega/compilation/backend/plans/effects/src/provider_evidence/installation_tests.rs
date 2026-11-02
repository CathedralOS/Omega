use super::*;

#[test]
fn installation_dependency_selection_preserves_plan_and_family_custody() {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let requirement = identities.intern("Requirement::visit");
    let head = identities.intern("Family::head");
    let member = identities.intern("Family::member");
    let mut selected = SelectedProviderPlanFacts {
        identities: identities.build(),
        installation_reach_resolutions: [1, 2]
            .map(|plan| InstallationReachResolution {
                requirement_identity: requirement,
                provider_plan_report_identity: plan,
                upper_bound: vec!["X".into(), "Y".into()],
                resolved_row: vec![if plan == 1 { "X" } else { "Y" }.into()],
            })
            .to_vec(),
        ..SelectedProviderPlanFacts::default()
    };
    assert_eq!(
        selected
            .resolve_installation_requirement("Requirement::visit", 1)
            .unwrap()
            .resolved_row,
        ["X"]
    );
    assert!(
        selected
            .resolve_installation_requirement("Requirement::visit", 3)
            .unwrap_err()
            .contains("more than one")
    );
    assert!(
        selected
            .resolve_installation_requirement("Missing", 1)
            .unwrap_err()
            .contains("unresolved")
    );
    selected
        .family_realization_rows
        .push(SelectedTargetFamilyRealization {
            terminal: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
            },
            target: "macos_arm64".into(),
            head: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
            member: semantic_vocabulary::BoundaryMachineId::new(2).unwrap(),
            head_identity: head,
            member_identity: member,
            selected_requirement_identity: requirement,
            provider_plan_report_identity: 2,
            provider_plan_digest: crate::provider_plan::ProviderPlanDigest::from_digest([2; 32]),
        });
    assert_eq!(
        selected
            .resolve_installation_requirement("Family::head", 1)
            .unwrap()
            .provider_plan_report_identity,
        2
    );
    assert_eq!(
        selected
            .resolve_installation_reach_for_plan(&[], &["Family::member".into()], 1)
            .unwrap(),
        ["Y"]
    );
    selected.installation_reach_resolutions.remove(0);
    assert_eq!(
        selected
            .resolve_installation_requirement("Requirement::visit", 3)
            .unwrap()
            .resolved_row,
        ["Y"]
    );
}
