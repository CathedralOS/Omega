use super::*;

fn candidate(bytes: u64) -> SameStackContributionAdmissionCandidate {
    SameStackContributionAdmissionCandidate {
        provider_plan_report_identity: 17,
        provider_plan_commitment: SameStackProviderPlanCommitment::from_digest([3; 32]),
        requirement_identity: terminal_psi::TerminalIdentityIndex(9),
        receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(21).unwrap(),
        bytes,
        alignment: 16,
    }
}

fn admit(
    candidate: SameStackContributionAdmissionCandidate,
) -> Result<AdmittedSameStackContribution, TaskPlanDiagnostic> {
    admit_same_stack_contribution(
        candidate,
        17,
        SameStackProviderPlanCommitment::from_digest([3; 32]),
        terminal_psi::TerminalIdentityIndex(9),
    )
}

#[test]
fn zero_component_stack_demand_retains_exact_edge_and_alignment() {
    let contribution = admit(candidate(0)).unwrap();
    let root = TaskStackFrameId::from_normalized_identity(1).unwrap();
    let summary = validate_task_stack_frame_summary(TaskStackFrameSummary {
        frame: root,
        local_bytes: 40,
        alignment: 8,
        validation: TaskStackFrameValidationId::from_normalized_identity(2).unwrap(),
        calls: vec![StackCallContribution::AdmittedSameStack(
            contribution.clone(),
        )],
        unresolved_calls: Vec::new(),
    })
    .unwrap();
    let demand = compose_task_stack_demand(root, [summary]).unwrap();
    assert_eq!(demand.bytes(), 48);
    assert_eq!(demand.alignment(), 16);
    assert!(demand.unresolved_calls().is_empty());
    assert_eq!(
        demand.admitted_contribution_report_identities(),
        &BTreeSet::from([contribution.report_identity()])
    );
    assert_eq!(
        demand.admitted_contribution_commitments(),
        &BTreeSet::from([contribution.commitment()])
    );
    assert_ne!(
        contribution.commitment(),
        admit(candidate(16)).unwrap().commitment()
    );
}

#[test]
fn zero_bytes_never_replace_provider_identity_or_alignment_evidence() {
    for mutation in 0..4 {
        let mut changed = candidate(0);
        match mutation {
            0 => changed.provider_plan_report_identity = 18,
            1 => {
                changed.provider_plan_commitment =
                    SameStackProviderPlanCommitment::from_digest([4; 32])
            }
            2 => changed.requirement_identity = terminal_psi::TerminalIdentityIndex(10),
            3 => changed.alignment = 0,
            _ => unreachable!(),
        }
        assert!(admit(changed).is_err(), "mutation {mutation}");
    }
}
