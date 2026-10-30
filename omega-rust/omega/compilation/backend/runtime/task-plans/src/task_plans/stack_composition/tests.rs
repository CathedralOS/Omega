use super::*;

#[test]
fn zero_frame_component_retains_exact_admission_and_call_coverage() {
    let commitment = SameStackProviderPlanCommitment::from_digest([7; 32]);
    let requirement = terminal_psi::TerminalIdentityIndex(1);
    let candidate = SameStackContributionAdmissionCandidate {
        provider_plan_report_identity: 17,
        provider_plan_commitment: commitment,
        requirement_identity: requirement,
        receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(19).unwrap(),
        bytes: 0,
        alignment: 16,
    };
    let contribution =
        admit_same_stack_contribution(candidate.clone(), 17, commitment, requirement)
            .expect("a leaf may allocate no additional frame");
    assert_eq!(contribution.bytes(), 0);
    let summary = validate_task_stack_frame_summary(TaskStackFrameSummary {
        frame: TaskStackFrameId::from_normalized_identity(23).unwrap(),
        local_bytes: 16,
        alignment: 16,
        validation: TaskStackFrameValidationId::from_normalized_identity(29).unwrap(),
        calls: vec![StackCallContribution::AdmittedSameStack(contribution)],
        unresolved_calls: Vec::new(),
    })
    .unwrap();
    assert_eq!(summary.summary().calls.len(), 1);
    let composed = compose_task_stack_demand(summary.summary().frame, [summary]).unwrap();
    assert_eq!(composed.bytes(), 16);
    assert_eq!(composed.admitted_contribution_report_identities().len(), 1);
    assert!(composed.unresolved_calls().is_empty());
    assert!(admit_same_stack_contribution(candidate.clone(), 18, commitment, requirement).is_err());
    let invalid = SameStackContributionAdmissionCandidate {
        alignment: 3,
        ..candidate
    };
    assert!(admit_same_stack_contribution(invalid, 17, commitment, requirement).is_err());
}
