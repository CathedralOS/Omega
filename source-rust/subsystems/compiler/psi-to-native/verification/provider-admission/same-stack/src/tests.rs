use super::*;
use terminal_psi::TerminalIdentityIndex;

fn candidate() -> SameStackContributionAdmissionCandidate {
    SameStackContributionAdmissionCandidate {
        provider_plan_report_identity: 17,
        provider_plan_commitment: SameStackProviderPlanCommitment::from_digest([7; 32]),
        requirement_identity: TerminalIdentityIndex(1),
        receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(19).unwrap(),
        bytes: 0,
        alignment: 16,
    }
}

#[test]
fn a_zero_byte_provider_leaf_keeps_its_exact_contribution() {
    let candidate = candidate();
    let admitted = admit_same_stack_contribution(
        candidate.clone(),
        candidate.provider_plan_report_identity,
        candidate.provider_plan_commitment,
        candidate.requirement_identity,
    )
    .unwrap();
    assert_eq!(admitted.bytes(), 0);
    assert_eq!(admitted.alignment(), 16);
    assert_eq!(admitted.receipt(), candidate.receipt);
    assert_eq!(
        admitted.provider_plan_commitment(),
        candidate.provider_plan_commitment
    );
    assert_eq!(
        admitted.requirement_identity(),
        candidate.requirement_identity
    );
    assert_eq!(
        admitted.report_identity().normalized_identity(),
        0x2c7194780071e676
    );
    assert_eq!(
        admitted.commitment().as_bytes(),
        [
            0xca, 0xc9, 0xa7, 0xaa, 0x58, 0x53, 0x89, 0x0e, 0x07, 0x08, 0x0f, 0x9e, 0x02, 0x3c,
            0x8f, 0xd5, 0xe3, 0xa9, 0xef, 0x96, 0x76, 0x47, 0xf6, 0x21, 0xe5, 0xc3, 0x71, 0x94,
            0xce, 0xf9, 0xe9, 0x0d
        ]
    );
}

#[test]
fn a_changed_plan_or_requirement_cannot_issue_the_contribution() {
    let candidate = candidate();
    for (report, commitment, requirement) in [
        (0, [7; 32], 1),
        (18, [7; 32], 1),
        (17, [0; 32], 1),
        (17, [9; 32], 1),
        (17, [7; 32], 0),
        (17, [7; 32], 2),
    ] {
        assert!(
            admit_same_stack_contribution(
                candidate.clone(),
                report,
                SameStackProviderPlanCommitment::from_digest(commitment),
                TerminalIdentityIndex(requirement),
            )
            .is_err()
        );
    }
}

#[test]
fn zero_receipts_and_invalid_alignment_are_refused() {
    assert!(SameStackContributionAdmissionReceiptId::from_normalized_identity(0).is_err());
    let candidate = candidate();
    for alignment in [0, 3] {
        let invalid = SameStackContributionAdmissionCandidate {
            alignment,
            ..candidate.clone()
        };
        assert!(
            admit_same_stack_contribution(
                invalid,
                17,
                candidate.provider_plan_commitment,
                candidate.requirement_identity,
            )
            .is_err()
        );
    }
}
