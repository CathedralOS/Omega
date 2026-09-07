use super::fixture::ExactCompilerRowScenario;
use super::*;

pub(super) fn assert_encoding_and_recovery(
    scenario: &ExactCompilerRowScenario,
    conflicts: &package_manager::review::ReviewOnlyCapabilityConflictSet,
) {
    let [package] = conflicts.packages() else {
        panic!("one package has candidate-bound conflicts")
    };
    let [conflict, second_conflict] = package.conflicts() else {
        panic!("two added public proposition rows")
    };

    let first_accept = package
        .root_policy_decision(
            conflict,
            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        )
        .expect("bind first exact blocking row");
    let second_accept = package
        .root_policy_decision(
            second_conflict,
            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        )
        .expect("bind second exact blocking row");
    let accepted_resolution =
        resolve_review_only_root_policy_decisions(conflicts, &[second_accept, first_accept])
            .expect("resolve every blocking row");
    assert!(accepted_resolution.all_blocking_rows_accepted());
    assert_eq!(accepted_resolution.decisions().len(), 2);
    assert_eq!(
        accepted_resolution.candidate_closure(),
        package.candidate_closure()
    );
    assert_ne!(accepted_resolution.commitment().digest(), [0; 32]);

    let record_limits = ReviewOnlyRootPolicyRecordLimits::default();
    let accepted_record = accepted_resolution
        .encode_canonical(record_limits)
        .expect("encode accepted root policy");
    let accepted_text = std::str::from_utf8(&accepted_record).expect("canonical policy UTF-8");
    let [first_canonical, second_canonical] = accepted_resolution.decisions() else {
        panic!("accepted resolution has two canonical decisions")
    };
    assert_eq!(
        accepted_text,
        format!(
            "OMEGA_PACKAGE_ROOT_POLICY_RESOLUTION_V1\n\
candidate_closure {}\n\
decision_count 2\n\
decision {} accept_candidate_change\n\
decision {} accept_candidate_change\n\
resolution_commitment {}\n\
end_root_policy_resolution\n",
            hex_digest(accepted_resolution.candidate_closure().digest()),
            hex_digest(first_canonical.conflict().digest()),
            hex_digest(second_canonical.conflict().digest()),
            hex_digest(accepted_resolution.commitment().digest()),
        )
    );
    assert_eq!(
        recover_review_only_root_policy_resolution(conflicts, &accepted_record, record_limits)
            .expect("recover accepted root policy"),
        accepted_resolution
    );
    assert_eq!(
        resolve_review_only_root_policy_decisions(conflicts, &[first_accept, second_accept])
            .expect("repeat accepted root policy")
            .encode_canonical(record_limits)
            .expect("encode repeated policy"),
        accepted_record
    );

    let mut wrong_candidate_record = accepted_record.clone();
    let candidate_offset = "OMEGA_PACKAGE_ROOT_POLICY_RESOLUTION_V1\ncandidate_closure ".len();
    wrong_candidate_record[candidate_offset] = if wrong_candidate_record[candidate_offset] == b'0' {
        b'1'
    } else {
        b'0'
    };
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &wrong_candidate_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::CandidateClosureMismatch { .. })
    ));

    let mut reordered_lines = accepted_text.lines().collect::<Vec<_>>();
    reordered_lines.swap(3, 4);
    let reordered_record = format!("{}\n", reordered_lines.join("\n"));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            reordered_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::NonCanonicalEncoding)
    ));

    let mut bad_commitment_record = accepted_record.clone();
    let commitment_offset = accepted_text
        .find("resolution_commitment ")
        .expect("commitment row")
        + "resolution_commitment ".len();
    bad_commitment_record[commitment_offset] = if bad_commitment_record[commitment_offset] == b'0' {
        b'1'
    } else {
        b'0'
    };
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &bad_commitment_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::CommitmentMismatch { .. })
    ));

    let mut trailing_record = accepted_record.clone();
    trailing_record.extend_from_slice(b"unexpected\n");
    assert!(matches!(
        recover_review_only_root_policy_resolution(conflicts, &trailing_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::InvalidFraming)
    ));

    let mut bad_header_record = accepted_record.clone();
    bad_header_record[0] = b'X';
    assert!(matches!(
        recover_review_only_root_policy_resolution(conflicts, &bad_header_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::InvalidHeader)
    ));

    let mut invalid_utf8_record = accepted_record.clone();
    invalid_utf8_record[0] = 0xff;
    assert!(matches!(
        recover_review_only_root_policy_resolution(conflicts, &invalid_utf8_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::InvalidUtf8)
    ));
    let crlf_record = accepted_text.replacen('\n', "\r\n", 1);
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            crlf_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidFraming)
    ));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &accepted_record[..accepted_record.len() - 1],
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidFraming)
    ));

    let mut uppercase_fingerprint_record = accepted_record.clone();
    let first_fingerprint_offset =
        accepted_text.find("decision ").expect("first decision row") + "decision ".len();
    uppercase_fingerprint_record[first_fingerprint_offset] = b'A';
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &uppercase_fingerprint_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidFingerprint)
    ));

    let mut invalid_candidate_record = accepted_record.clone();
    invalid_candidate_record[candidate_offset] = b'g';
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &invalid_candidate_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidCandidateClosure)
    ));

    let mut invalid_commitment_record = accepted_record.clone();
    invalid_commitment_record[commitment_offset] = b'g';
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &invalid_commitment_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidCommitment)
    ));

    let noncanonical_count_record =
        accepted_text.replacen("decision_count 2", "decision_count 02", 1);
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            noncanonical_count_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidDecisionCount)
    ));
    let overflowing_count_record = accepted_text.replacen(
        "decision_count 2",
        "decision_count 999999999999999999999999999999999999",
        1,
    );
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            overflowing_count_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidDecisionCount)
    ));

    let invalid_disposition_record =
        accepted_text.replacen("accept_candidate_change", "allow_candidate_change", 1);
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            invalid_disposition_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidDisposition)
    ));

    let mut missing_lines = accepted_text.lines().map(str::to_owned).collect::<Vec<_>>();
    missing_lines[2] = "decision_count 1".to_owned();
    missing_lines.remove(4);
    let missing_record = format!("{}\n", missing_lines.join("\n"));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            missing_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::Resolution(
            ReviewOnlyRootPolicyResolutionError::MissingDecision { .. }
        ))
    ));

    let mut duplicate_lines = accepted_text.lines().map(str::to_owned).collect::<Vec<_>>();
    duplicate_lines[4] = duplicate_lines[3].clone();
    let duplicate_record = format!("{}\n", duplicate_lines.join("\n"));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            duplicate_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::Resolution(
            ReviewOnlyRootPolicyResolutionError::DuplicateDecision { .. }
        ))
    ));

    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &accepted_record,
            ReviewOnlyRootPolicyRecordLimits::new(accepted_record.len() - 1, 2, 2)
        ),
        Err(ReviewOnlyRootPolicyRecordError::ByteLimitExceeded { .. })
    ));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &accepted_record,
            ReviewOnlyRootPolicyRecordLimits::new(accepted_record.len(), 1, 2)
        ),
        Err(ReviewOnlyRootPolicyRecordError::DecisionLimitExceeded { .. })
    ));
    assert!(matches!(
        accepted_resolution.encode_canonical(ReviewOnlyRootPolicyRecordLimits::new(1, 2, 2)),
        Err(ReviewOnlyRootPolicyRecordError::ByteLimitExceeded { .. })
    ));
    assert!(matches!(
        accepted_resolution.encode_canonical(ReviewOnlyRootPolicyRecordLimits::new(
            record_limits.maximum_bytes(),
            1,
            2
        )),
        Err(ReviewOnlyRootPolicyRecordError::DecisionLimitExceeded { .. })
    ));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            conflicts,
            &accepted_record,
            ReviewOnlyRootPolicyRecordLimits::new(accepted_record.len(), 2, 1)
        ),
        Err(ReviewOnlyRootPolicyRecordError::ConflictLimitExceeded { .. })
    ));

    let stale_conflicts = compare_review_only_capabilities(
        &scenario.stale_baseline_reviews,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare same candidate against alternate baseline");
    let [stale_package] = stale_conflicts.packages() else {
        panic!("alternate baseline has one changed package")
    };
    let [stale_conflict] = stale_package.conflicts() else {
        panic!("alternate baseline has one changed proposition")
    };
    let stale_decision = stale_package
        .root_policy_decision(
            stale_conflict,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        )
        .expect("bind alternate-baseline decision");
    let stale_record =
        resolve_review_only_root_policy_decisions(&stale_conflicts, &[stale_decision])
            .expect("resolve alternate-baseline policy")
            .encode_canonical(record_limits)
            .expect("encode alternate-baseline policy");
    assert!(matches!(
        recover_review_only_root_policy_resolution(conflicts, &stale_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::UnknownConflictFingerprint { .. })
    ));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &stale_conflicts,
            &accepted_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidDecisionCount)
    ));
    assert_eq!(
        stale_decision.candidate_closure(),
        package.candidate_closure()
    );
    assert!(matches!(
        resolve_review_only_root_policy_decisions(conflicts, &[stale_decision, first_accept]),
        Err(ReviewOnlyRootPolicyResolutionError::StaleOrForeignConflict { .. })
    ));
    assert_eq!(
        resolve_review_only_root_policy_decisions(conflicts, &[first_accept, second_accept])
            .expect("decision input order is canonicalized")
            .commitment(),
        accepted_resolution.commitment()
    );

    let second_reject = package
        .root_policy_decision(
            second_conflict,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        )
        .expect("bind explicit candidate rejection");
    let rejected_resolution =
        resolve_review_only_root_policy_decisions(conflicts, &[first_accept, second_reject])
            .expect("a rejection is still a complete policy result");
    assert!(!rejected_resolution.all_blocking_rows_accepted());
    assert_ne!(
        rejected_resolution.commitment(),
        accepted_resolution.commitment()
    );
    let rejected_record = rejected_resolution
        .encode_canonical(record_limits)
        .expect("encode rejected root policy");
    assert!(
        std::str::from_utf8(&rejected_record)
            .expect("rejected policy UTF-8")
            .contains("reject_candidate_change")
    );
    assert_eq!(
        recover_review_only_root_policy_resolution(conflicts, &rejected_record, record_limits)
            .expect("recover rejected root policy"),
        rejected_resolution
    );
    assert!(matches!(
        resolve_review_only_root_policy_decisions(conflicts, &[]),
        Err(ReviewOnlyRootPolicyResolutionError::EmptyDecisionSet)
    ));
    assert!(matches!(
        resolve_review_only_root_policy_decisions(conflicts, &[first_accept]),
        Err(ReviewOnlyRootPolicyResolutionError::MissingDecision { .. })
    ));
    assert!(matches!(
        resolve_review_only_root_policy_decisions(conflicts, &[first_accept, first_accept]),
        Err(ReviewOnlyRootPolicyResolutionError::DuplicateDecision { .. })
    ));
    assert!(matches!(
        resolve_review_only_root_policy_decisions(
            conflicts,
            &[first_accept, first_accept, second_accept]
        ),
        Err(ReviewOnlyRootPolicyResolutionError::TooManyDecisions { maximum: 2 })
    ));
    assert_eq!(
        triage_review_update(
            &scenario.baseline_reviews,
            &scenario.candidate_reviews,
            &BTreeSet::new()
        )
        .disposition(),
        PackageTriageDisposition::BlockedCapabilityChange
    );
}
