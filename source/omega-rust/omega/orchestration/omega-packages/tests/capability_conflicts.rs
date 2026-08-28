use omega_package_review::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};
use omega_packages::{
    ExternalSourceContext, LocalSourceLimits, PackageSourceClosureLimits, PackageTriageDisposition,
    PackageTriageReason, ReviewOnlyBaselineCapsule, ReviewOnlyBaselineLimits,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDirectory,
    ReviewOnlyRootPolicyDisposition, ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName,
    ReviewOnlyRootPolicyNameError, ReviewOnlyRootPolicyRecordError,
    ReviewOnlyRootPolicyRecordLimits, ReviewOnlyRootPolicyResolutionError,
    compare_review_only_capabilities, compare_review_only_capabilities_from_baseline,
    compile_resolved_package_reviews, recover_review_only_root_policy_resolution,
    resolve_external_local_package_closure, resolve_review_only_root_policy_decisions,
    triage_review_update, triage_review_update_from_baseline,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-capability-conflict-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn hex_digest(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_package(root: &Path, main: &str) {
    std::fs::create_dir_all(root).expect("create test package");
    std::fs::write(
        root.join("build.omg"),
        r#"target windows_x64 { }

machine build(builder: &mut Build) {
    builder.package("conflict-probe");
}
"#,
    )
    .expect("write package declaration");
    std::fs::write(root.join("main.omg"), main).expect("write package source");
}

#[test]
fn exact_compiler_rows_become_candidate_bound_review_conflicts() {
    let live = temp_root("live");
    let baseline_cache = temp_root("baseline-cache");
    let stale_baseline_cache = temp_root("stale-baseline-cache");
    let candidate_cache = temp_root("candidate-cache");
    let representation_cache = temp_root("representation-cache");
    let dangerous_slack_cache = temp_root("dangerous-slack-cache");
    let accepted_claim_baseline_cache = temp_root("accepted-claim-baseline-cache");
    let accepted_claim_candidate_cache = temp_root("accepted-claim-candidate-cache");
    let build_root = temp_root("build");
    let context = ExternalSourceContext::derive(b"capability-conflict-test-lock");
    write_package(
        &live,
        r#"pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}
"#,
    );
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve baseline custody");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile baseline review");

    write_package(
        &live,
        r#"pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}

pub proposition ready();
"#,
    );
    let stale_baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &stale_baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve alternate baseline custody");
    let stale_baseline_reviews =
        compile_resolved_package_reviews(&stale_baseline_sources, "windows_x64", &build_root)
            .expect("compile alternate baseline review");

    write_package(
        &live,
        r#"pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}

pub proposition ready();
pub proposition settled();
"#,
    );
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve candidate custody");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile candidate review");

    assert_eq!(
        baseline_sources.graph().root(),
        candidate_sources.graph().root()
    );
    assert_ne!(
        baseline_sources
            .custody(baseline_sources.graph().root())
            .unwrap()
            .resolution(),
        candidate_sources
            .custody(candidate_sources.graph().root())
            .unwrap()
            .resolution()
    );

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare exact compiler rows");
    let baseline_capsule = ReviewOnlyBaselineCapsule::capture(
        &baseline_sources,
        &baseline_reviews,
        ReviewOnlyBaselineLimits::default(),
    )
    .expect("capture restart-stable review baseline");
    let baseline_bytes = baseline_capsule
        .encode(ReviewOnlyBaselineLimits::default())
        .expect("encode restart-stable review baseline");
    let recovered_baseline =
        ReviewOnlyBaselineCapsule::decode(&baseline_bytes, ReviewOnlyBaselineLimits::default())
            .expect("decode restart-stable review baseline");
    assert_eq!(
        recovered_baseline
            .encode(ReviewOnlyBaselineLimits::default())
            .expect("re-encode canonical review baseline"),
        baseline_bytes
    );
    let recovered_conflicts = compare_review_only_capabilities_from_baseline(
        &recovered_baseline,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare candidate with recovered baseline");
    assert_eq!(recovered_conflicts, conflicts);
    assert_eq!(
        triage_review_update_from_baseline(
            &recovered_baseline,
            &candidate_reviews,
            &BTreeSet::new(),
        ),
        triage_review_update(&baseline_reviews, &candidate_reviews, &BTreeSet::new())
    );
    let mut corrupted_baseline = baseline_bytes.clone();
    corrupted_baseline[0] ^= 1;
    assert!(
        ReviewOnlyBaselineCapsule::decode(
            &corrupted_baseline,
            ReviewOnlyBaselineLimits::default(),
        )
        .is_err(),
        "corrupted restart capsule must reject"
    );
    let mut wrong_version = baseline_bytes.clone();
    let version_offset = b"OMEGA-PACKAGE-REVIEW-BASELINE\0".len();
    wrong_version[version_offset..version_offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    let prefix_length = wrong_version.len() - 32;
    let mut checksum = Sha256::new();
    checksum.update(b"OMEGA-PACKAGE-REVIEW-BASELINE-CAPSULE\0");
    checksum.update((prefix_length as u64).to_le_bytes());
    checksum.update(&wrong_version[..prefix_length]);
    let checksum: [u8; 32] = checksum.finalize().into();
    wrong_version[prefix_length..].copy_from_slice(&checksum);
    assert_eq!(
        ReviewOnlyBaselineCapsule::decode(&wrong_version, ReviewOnlyBaselineLimits::default(),)
            .expect_err("checksummed stale capsule version must reject")
            .message(),
        "unsupported review baseline capsule header"
    );
    assert!(
        ReviewOnlyBaselineCapsule::decode(
            &baseline_bytes,
            ReviewOnlyBaselineLimits::new(
                baseline_bytes.len() - 1,
                1_024,
                16_384,
                128,
                4 * 1024,
                256,
                65_536,
                32 * 1024 * 1024,
            ),
        )
        .is_err(),
        "capsule byte ceiling must reject before parsing"
    );
    let tight_identity_limits = ReviewOnlyBaselineLimits::new(
        64 * 1024 * 1024,
        1_024,
        16_384,
        128,
        4,
        256,
        65_536,
        32 * 1024 * 1024,
    );
    assert!(
        baseline_capsule.encode(tight_identity_limits).is_err(),
        "encoding must enforce the same identity ceiling as decoding"
    );
    assert!(
        ReviewOnlyBaselineCapsule::decode(&baseline_bytes, tight_identity_limits).is_err(),
        "decoding must enforce the configured identity ceiling"
    );
    assert_eq!(conflicts.packages().len(), 1);
    assert_eq!(conflicts.conflict_count(), 2);
    let package = &conflicts.packages()[0];
    assert_eq!(package.key(), candidate_sources.graph().root());
    assert!(package.dependency_path().steps().is_empty());
    assert_ne!(package.candidate_closure().digest(), [0; 32]);
    let [conflict, second_conflict] = package.conflicts() else {
        panic!("two added public proposition rows")
    };
    assert_eq!(
        conflict.kind(),
        PackageReviewCanonicalRowKind::PublicProposition
    );
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(conflict.change(), ReviewOnlyCapabilityConflictChange::Added);
    assert!(conflict.baseline_row().is_none());
    assert!(conflict.candidate_row().is_some());
    assert!(conflict.baseline_source().is_none());
    let candidate_locations = conflict
        .candidate_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("added proposition has compiler-issued candidate source");
    assert_eq!(candidate_locations.len(), 1);
    assert_eq!(candidate_locations[0].relative_path(), "main.omg");
    assert!(conflict.is_blocking());
    assert_ne!(conflict.fingerprint().digest(), [0; 32]);
    assert_eq!(
        second_conflict.kind(),
        PackageReviewCanonicalRowKind::PublicProposition
    );
    assert!(second_conflict.is_blocking());

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
        resolve_review_only_root_policy_decisions(&conflicts, &[second_accept, first_accept])
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
        recover_review_only_root_policy_resolution(&conflicts, &accepted_record, record_limits)
            .expect("recover accepted root policy"),
        accepted_resolution
    );
    assert_eq!(
        resolve_review_only_root_policy_decisions(&conflicts, &[first_accept, second_accept])
            .expect("repeat accepted root policy")
            .encode_canonical(record_limits)
            .expect("encode repeated policy"),
        accepted_record
    );

    let policy_root = temp_root("capability-conflict-root-policy");
    let policy_outside = temp_root("capability-conflict-root-policy-outside");
    std::fs::create_dir_all(policy_root.join("policy")).expect("create root policy directory");
    std::fs::create_dir_all(&policy_outside).expect("create outside policy directory");
    let policy_directory_path = policy_root.join("policy");
    let policy_directory =
        cap_std::fs::Dir::open_ambient_dir(&policy_directory_path, cap_std::ambient_authority())
            .expect("open explicit root-owned policy directory");
    let policy_project =
        ReviewOnlyRootPolicyDirectory::from_capability(policy_directory, &policy_directory_path)
            .expect("bind root-owned policy directory capability");
    let accepted_policy_path =
        ReviewOnlyRootPolicyName::parse("candidate.policy").expect("canonical policy filename");
    policy_project
        .persist_new_resolution(&accepted_policy_path, &accepted_resolution, record_limits)
        .expect("persist accepted root policy beneath project root");
    assert_eq!(
        policy_project
            .recover_resolution(&accepted_policy_path, &conflicts, record_limits)
            .expect("recover root-project-custodied policy"),
        accepted_resolution
    );
    assert_eq!(
        std::fs::read(policy_directory_path.join(accepted_policy_path.as_str()))
            .expect("persisted policy bytes"),
        accepted_record
    );
    assert_eq!(
        std::fs::read_dir(policy_root.join("policy"))
            .expect("policy directory")
            .count(),
        1,
        "successful publication removes its exclusive stage"
    );
    assert!(matches!(
        policy_project.persist_new_resolution(
            &accepted_policy_path,
            &accepted_resolution,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyFileError::DestinationExists { .. })
    ));
    assert_eq!(
        std::fs::read(policy_directory_path.join(accepted_policy_path.as_str()))
            .expect("existing policy remains unchanged"),
        accepted_record
    );
    assert!(matches!(
        policy_project.recover_resolution(
            &accepted_policy_path,
            &conflicts,
            ReviewOnlyRootPolicyRecordLimits::new(accepted_record.len() - 1, 2, 2)
        ),
        Err(ReviewOnlyRootPolicyFileError::ByteLimitExceeded { .. })
    ));

    for invalid_path in [
        "",
        "/policy/candidate.policy",
        "policy/",
        "policy//candidate.policy",
        "./policy/candidate.policy",
        "../candidate.policy",
        "policy\\candidate.policy",
        "policy/candidate.",
        "Policy/candidate.policy",
        "policy/NUL.txt",
        "policy/COM1",
    ] {
        assert_eq!(
            ReviewOnlyRootPolicyName::parse(invalid_path),
            Err(ReviewOnlyRootPolicyNameError::InvalidName),
            "accepted noncanonical root policy path {invalid_path:?}"
        );
    }
    assert_eq!(
        ReviewOnlyRootPolicyName::parse(&"a".repeat(256)),
        Err(ReviewOnlyRootPolicyNameError::InvalidName)
    );

    let noncanonical_policy_path =
        ReviewOnlyRootPolicyName::parse("noncanonical.policy").expect("policy path");
    let mut noncanonical_policy_bytes = accepted_record.clone();
    noncanonical_policy_bytes.push(b'\n');
    std::fs::write(
        policy_directory_path.join(noncanonical_policy_path.as_str()),
        noncanonical_policy_bytes,
    )
    .expect("write authored noncanonical policy");
    assert!(matches!(
        policy_project.recover_resolution(&noncanonical_policy_path, &conflicts, record_limits),
        Err(ReviewOnlyRootPolicyFileError::Record(
            ReviewOnlyRootPolicyRecordError::InvalidFraming
        ))
    ));

    let directory_policy_path =
        ReviewOnlyRootPolicyName::parse("directory.policy").expect("policy path");
    std::fs::create_dir(policy_directory_path.join(directory_policy_path.as_str()))
        .expect("create non-regular policy leaf");
    assert!(matches!(
        policy_project.recover_resolution(&directory_policy_path, &conflicts, record_limits),
        Err(ReviewOnlyRootPolicyFileError::NotRegularFile { .. })
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};

        assert_eq!(
            std::fs::metadata(policy_directory_path.join(accepted_policy_path.as_str()))
                .expect("policy metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let outside_record = policy_outside.join("outside.policy");
        std::fs::write(&outside_record, &accepted_record).expect("write outside policy bytes");
        let leaf_link_path = ReviewOnlyRootPolicyName::parse("link.policy").expect("policy path");
        symlink(
            &outside_record,
            policy_directory_path.join(leaf_link_path.as_str()),
        )
        .expect("create policy leaf symlink");
        assert!(matches!(
            policy_project.recover_resolution(&leaf_link_path, &conflicts, record_limits),
            Err(ReviewOnlyRootPolicyFileError::NotRegularFile { .. })
        ));
        assert!(matches!(
            policy_project.persist_new_resolution(
                &leaf_link_path,
                &accepted_resolution,
                record_limits
            ),
            Err(ReviewOnlyRootPolicyFileError::DestinationExists { .. })
        ));
        assert_eq!(
            std::fs::read(&outside_record).expect("outside policy remains unchanged"),
            accepted_record
        );
    }

    let mut wrong_candidate_record = accepted_record.clone();
    let candidate_offset = "OMEGA_PACKAGE_ROOT_POLICY_RESOLUTION_V1\ncandidate_closure ".len();
    wrong_candidate_record[candidate_offset] = if wrong_candidate_record[candidate_offset] == b'0' {
        b'1'
    } else {
        b'0'
    };
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
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
            &conflicts,
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
            &conflicts,
            &bad_commitment_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::CommitmentMismatch { .. })
    ));

    let mut trailing_record = accepted_record.clone();
    trailing_record.extend_from_slice(b"unexpected\n");
    assert!(matches!(
        recover_review_only_root_policy_resolution(&conflicts, &trailing_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::InvalidFraming)
    ));

    let mut bad_header_record = accepted_record.clone();
    bad_header_record[0] = b'X';
    assert!(matches!(
        recover_review_only_root_policy_resolution(&conflicts, &bad_header_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::InvalidHeader)
    ));

    let mut invalid_utf8_record = accepted_record.clone();
    invalid_utf8_record[0] = 0xff;
    assert!(matches!(
        recover_review_only_root_policy_resolution(&conflicts, &invalid_utf8_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::InvalidUtf8)
    ));
    let crlf_record = accepted_text.replacen('\n', "\r\n", 1);
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
            crlf_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidFraming)
    ));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
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
            &conflicts,
            &uppercase_fingerprint_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidFingerprint)
    ));

    let mut invalid_candidate_record = accepted_record.clone();
    invalid_candidate_record[candidate_offset] = b'g';
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
            &invalid_candidate_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidCandidateClosure)
    ));

    let mut invalid_commitment_record = accepted_record.clone();
    invalid_commitment_record[commitment_offset] = b'g';
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
            &invalid_commitment_record,
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidCommitment)
    ));

    let noncanonical_count_record =
        accepted_text.replacen("decision_count 2", "decision_count 02", 1);
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
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
            &conflicts,
            overflowing_count_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::InvalidDecisionCount)
    ));

    let invalid_disposition_record =
        accepted_text.replacen("accept_candidate_change", "allow_candidate_change", 1);
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
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
            &conflicts,
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
            &conflicts,
            duplicate_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::Resolution(
            ReviewOnlyRootPolicyResolutionError::DuplicateDecision { .. }
        ))
    ));

    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
            &accepted_record,
            ReviewOnlyRootPolicyRecordLimits::new(accepted_record.len() - 1, 2, 2)
        ),
        Err(ReviewOnlyRootPolicyRecordError::ByteLimitExceeded { .. })
    ));
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &conflicts,
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
            &conflicts,
            &accepted_record,
            ReviewOnlyRootPolicyRecordLimits::new(accepted_record.len(), 2, 1)
        ),
        Err(ReviewOnlyRootPolicyRecordError::ConflictLimitExceeded { .. })
    ));

    let stale_conflicts = compare_review_only_capabilities(
        &stale_baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
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
        recover_review_only_root_policy_resolution(&conflicts, &stale_record, record_limits),
        Err(ReviewOnlyRootPolicyRecordError::UnknownConflictFingerprint { .. })
    ));
    assert!(matches!(
        policy_project.recover_resolution(&accepted_policy_path, &stale_conflicts, record_limits),
        Err(ReviewOnlyRootPolicyFileError::Record(
            ReviewOnlyRootPolicyRecordError::InvalidDecisionCount
        ))
    ));
    assert_eq!(
        stale_decision.candidate_closure(),
        package.candidate_closure()
    );
    assert!(matches!(
        resolve_review_only_root_policy_decisions(&conflicts, &[stale_decision, first_accept]),
        Err(ReviewOnlyRootPolicyResolutionError::StaleOrForeignConflict { .. })
    ));
    assert_eq!(
        resolve_review_only_root_policy_decisions(&conflicts, &[first_accept, second_accept])
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
        resolve_review_only_root_policy_decisions(&conflicts, &[first_accept, second_reject])
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
        recover_review_only_root_policy_resolution(&conflicts, &rejected_record, record_limits)
            .expect("recover rejected root policy"),
        rejected_resolution
    );
    let rejected_policy_path =
        ReviewOnlyRootPolicyName::parse("rejected.policy").expect("canonical rejected policy path");
    policy_project
        .persist_new_resolution(&rejected_policy_path, &rejected_resolution, record_limits)
        .expect("persist rejected root policy");
    assert_eq!(
        policy_project
            .recover_resolution(&rejected_policy_path, &conflicts, record_limits)
            .expect("recover rejected root policy from project custody"),
        rejected_resolution
    );
    assert!(matches!(
        resolve_review_only_root_policy_decisions(&conflicts, &[]),
        Err(ReviewOnlyRootPolicyResolutionError::EmptyDecisionSet)
    ));
    assert!(matches!(
        resolve_review_only_root_policy_decisions(&conflicts, &[first_accept]),
        Err(ReviewOnlyRootPolicyResolutionError::MissingDecision { .. })
    ));
    assert!(matches!(
        resolve_review_only_root_policy_decisions(&conflicts, &[first_accept, first_accept]),
        Err(ReviewOnlyRootPolicyResolutionError::DuplicateDecision { .. })
    ));
    assert!(matches!(
        resolve_review_only_root_policy_decisions(
            &conflicts,
            &[first_accept, first_accept, second_accept]
        ),
        Err(ReviewOnlyRootPolicyResolutionError::TooManyDecisions { maximum: 2 })
    ));
    assert_eq!(
        triage_review_update(&baseline_reviews, &candidate_reviews, &BTreeSet::new()).disposition(),
        PackageTriageDisposition::BlockedCapabilityChange
    );

    let repeated = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("repeat deterministic comparison");
    assert_eq!(repeated, conflicts);

    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render bounded conflict evidence");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V15\n"));
    assert!(rendered.contains("change added\nkind public_proposition\nrisk blocking\n"));
    assert!(rendered.contains("candidate_location declaration package "));
    assert!(rendered.contains(" \"main.omg\"\n"));
    assert!(!rendered.contains(&live.display().to_string()));
    assert!(!rendered.contains(&candidate_cache.display().to_string()));
    assert_eq!(
        conflicts
            .render_bounded(rendered.len())
            .expect("exact output ceiling"),
        rendered
    );
    let too_small = conflicts
        .render_bounded(rendered.len() - 1)
        .expect_err("renderer rejects rather than truncating");
    assert_eq!(too_small.required_bytes(), Some(rendered.len()));

    let zero_conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::new(
            4_096,
            131_072,
            16 * 1024 * 1024,
            32 * 1024 * 1024,
            262_144,
            16 * 1024 * 1024,
            0,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            1_024,
        ),
    )
    .expect_err("row-count ceiling rejects without a partial result");
    assert!(matches!(
        zero_conflicts,
        ReviewOnlyCapabilityConflictError::TooManyConflicts { maximum: 0 }
    ));
    let zero_bytes = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::new(
            4_096,
            131_072,
            16 * 1024 * 1024,
            32 * 1024 * 1024,
            262_144,
            16 * 1024 * 1024,
            65_536,
            0,
            8 * 1024 * 1024,
            1_024,
        ),
    )
    .expect_err("changed-row byte ceiling rejects without a partial result");
    assert!(matches!(
        zero_bytes,
        ReviewOnlyCapabilityConflictError::ChangedRowBytesExceeded { maximum_bytes: 0 }
    ));

    let mismatched_custody = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &baseline_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect_err("candidate evidence cannot detach from candidate custody");
    assert!(matches!(
        mismatched_custody,
        ReviewOnlyCapabilityConflictError::CandidateResolutionMismatch { .. }
    ));

    let unchanged = compare_review_only_capabilities(
        &baseline_reviews,
        &baseline_reviews,
        &baseline_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("unchanged rows compare cleanly");
    assert!(unchanged.is_empty());
    assert!(matches!(
        resolve_review_only_root_policy_decisions(&unchanged, &[]),
        Err(ReviewOnlyRootPolicyResolutionError::NoBlockingConflicts)
    ));

    let removal_conflicts = compare_review_only_capabilities(
        &candidate_reviews,
        &baseline_reviews,
        &baseline_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare blocking candidate removals");
    let [removal_package] = removal_conflicts.packages() else {
        panic!("one package removes public propositions")
    };
    assert_eq!(removal_package.conflicts().len(), 2);
    for removal in removal_package.conflicts() {
        assert_eq!(
            removal.kind(),
            PackageReviewCanonicalRowKind::PublicProposition
        );
        assert_eq!(
            removal.change(),
            ReviewOnlyCapabilityConflictChange::Removed
        );
        assert!(removal.baseline_row().is_some());
        assert!(removal.candidate_row().is_none());
        assert!(removal.is_blocking());
        removal_package
            .root_policy_decision(
                removal,
                ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
            )
            .expect("candidate-change decision also covers row removal");
    }

    write_package(
        &live,
        r#"boundary data PlatformToken;

pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}
"#,
    );
    let representation_sources = resolve_external_local_package_closure(
        &live,
        ExternalSourceContext::derive(b"capability-conflict-test-lock"),
        &representation_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve representation-TCB candidate");
    let representation_reviews =
        compile_resolved_package_reviews(&representation_sources, "windows_x64", &build_root)
            .expect("compile representation-TCB review");
    let representation_conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &representation_reviews,
        &representation_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare representation-TCB row");
    let [representation_package] = representation_conflicts.packages() else {
        panic!("one package has representation-TCB change")
    };
    let [representation_conflict] = representation_package.conflicts() else {
        panic!("one representation-TCB row is introduced")
    };
    assert_eq!(
        representation_conflict.kind(),
        PackageReviewCanonicalRowKind::RepresentationTcb
    );
    assert_eq!(
        representation_conflict.risk(),
        PackageReviewCanonicalRowRisk::AuditRecommended
    );
    assert!(!representation_conflict.is_blocking());
    assert!(matches!(
        representation_package.root_policy_decision(
            representation_conflict,
            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        ),
        Err(ReviewOnlyRootPolicyResolutionError::NonBlockingConflict { .. })
    ));
    let nonblocking_record = format!(
        "OMEGA_PACKAGE_ROOT_POLICY_RESOLUTION_V1\n\
candidate_closure {}\n\
decision_count 1\n\
decision {} accept_candidate_change\n\
resolution_commitment {}\n\
end_root_policy_resolution\n",
        hex_digest(representation_package.candidate_closure().digest()),
        hex_digest(representation_conflict.fingerprint().digest()),
        "0".repeat(64),
    );
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &representation_conflicts,
            nonblocking_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::Resolution(
            ReviewOnlyRootPolicyResolutionError::NonBlockingConflict { .. }
        ))
    ));
    assert_eq!(
        triage_review_update(&baseline_reviews, &representation_reviews, &BTreeSet::new())
            .disposition(),
        PackageTriageDisposition::AdmittedWithAuditRecommended
    );

    write_package(
        &live,
        r#"use omega::language::std::filesystem_host;

pub machine reserved_filesystem_authority()
reaches FilesystemHost
{
}
"#,
    );
    let dangerous_slack_sources = resolve_external_local_package_closure(
        &live,
        ExternalSourceContext::derive(b"capability-conflict-test-lock"),
        &dangerous_slack_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve dangerous-slack candidate");
    let dangerous_slack_reviews =
        compile_resolved_package_reviews(&dangerous_slack_sources, "windows_x64", &build_root)
            .expect("compile dangerous-slack review");
    let dangerous_slack_conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &dangerous_slack_reviews,
        &dangerous_slack_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare dangerous-slack row");
    let dangerous_slack_conflict = dangerous_slack_conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::DangerousAuthoritySlack)
        .unwrap_or_else(|| {
            panic!(
                "declared-but-unused dangerous authority produces an exact slack conflict; got {:?}",
                dangerous_slack_conflicts
                    .packages()
                    .iter()
                    .flat_map(|package| package.conflicts())
                    .map(|conflict| conflict.kind())
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        dangerous_slack_conflict.change(),
        ReviewOnlyCapabilityConflictChange::Added
    );
    assert_eq!(
        dangerous_slack_conflict.risk(),
        PackageReviewCanonicalRowRisk::AuditRecommended
    );
    assert!(!dangerous_slack_conflict.is_blocking());
    let slack_locations = dangerous_slack_conflict
        .candidate_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("dangerous slack has authority and callable provenance");
    assert!(slack_locations.iter().any(|location| {
        location.role()
            == omega_package_review::PackageReviewSourceLocationRole::AuthorityDeclaration
    }));
    assert!(slack_locations.iter().any(|location| {
        location.role() == omega_package_review::PackageReviewSourceLocationRole::AuthorityExposure
            && location.relative_path() == "main.omg"
    }));
    let slack_triage = triage_review_update(
        &baseline_reviews,
        &dangerous_slack_reviews,
        &BTreeSet::new(),
    );
    assert!(slack_triage.decisions().iter().any(|decision| {
        decision
            .reasons()
            .contains(&PackageTriageReason::DangerousAuthoritySlack(
                omega_package_review::PackageReviewDangerousAuthorityClass::Filesystem,
            ))
    }));

    write_package(
        &live,
        r#"boundary machine trusted_zero() -> u64
ensures result == 0;
"#,
    );
    let accepted_claim_baseline_sources = resolve_external_local_package_closure(
        &live,
        ExternalSourceContext::derive(b"accepted-claim-conflict-test-lock"),
        &accepted_claim_baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve accepted-claim baseline");
    let accepted_claim_baseline_reviews = compile_resolved_package_reviews(
        &accepted_claim_baseline_sources,
        "windows_x64",
        &build_root,
    )
    .expect("compile accepted-claim baseline");

    write_package(
        &live,
        r#"boundary machine trusted_zero() -> u64
ensures result == 1;
"#,
    );
    let accepted_claim_candidate_sources = resolve_external_local_package_closure(
        &live,
        ExternalSourceContext::derive(b"accepted-claim-conflict-test-lock"),
        &accepted_claim_candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve changed accepted claim");
    let accepted_claim_candidate_reviews = compile_resolved_package_reviews(
        &accepted_claim_candidate_sources,
        "windows_x64",
        &build_root,
    )
    .expect("compile changed accepted claim");
    let accepted_claim_conflicts = compare_review_only_capabilities(
        &accepted_claim_baseline_reviews,
        &accepted_claim_candidate_reviews,
        &accepted_claim_candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare changed accepted claim");
    let accepted_claim_conflict = accepted_claim_conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .expect("changed accepted guarantee produces an exact trust conflict");
    assert_eq!(
        accepted_claim_conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert_eq!(
        accepted_claim_conflict.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    assert!(accepted_claim_conflict.is_blocking());
    let accepted_claim_render = accepted_claim_conflicts
        .render_bounded(1024 * 1024)
        .expect("render changed accepted claim");
    assert!(accepted_claim_render.contains("baseline_location contract_clause package "));
    assert!(accepted_claim_render.contains("candidate_location contract_clause package "));
    let accepted_claim_package = accepted_claim_conflicts
        .packages()
        .iter()
        .find(|package| {
            package
                .conflicts()
                .iter()
                .any(|conflict| conflict.fingerprint() == accepted_claim_conflict.fingerprint())
        })
        .expect("accepted-claim package");
    let wrong_candidate_decision = accepted_claim_package
        .root_policy_decision(
            accepted_claim_conflict,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        )
        .expect("bind other candidate decision");
    assert!(matches!(
        resolve_review_only_root_policy_decisions(
            &conflicts,
            &[wrong_candidate_decision, first_accept]
        ),
        Err(ReviewOnlyRootPolicyResolutionError::WrongCandidateClosure { .. })
    ));
    assert!(matches!(
        package.root_policy_decision(
            accepted_claim_conflict,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        ),
        Err(ReviewOnlyRootPolicyResolutionError::ConflictDoesNotBelongToPackage { .. })
    ));
    assert!(
        accepted_claim_conflict
            .candidate_source()
            .and_then(PackageReviewCanonicalRowSource::authored_locations)
            .expect("accepted-claim conflict has declaration provenance")
            .iter()
            .any(|location| location.relative_path() == "main.omg")
    );
    assert_eq!(
        triage_review_update(
            &accepted_claim_baseline_reviews,
            &accepted_claim_baseline_reviews,
            &BTreeSet::new(),
        )
        .disposition(),
        PackageTriageDisposition::Admitted,
        "an unchanged accepted baseline remains visible without blanket reapproval"
    );

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(stale_baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(representation_cache);
    let _ = std::fs::remove_dir_all(dangerous_slack_cache);
    let _ = std::fs::remove_dir_all(accepted_claim_baseline_cache);
    let _ = std::fs::remove_dir_all(accepted_claim_candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
    let _ = std::fs::remove_dir_all(policy_root);
    let _ = std::fs::remove_dir_all(policy_outside);
}

#[test]
fn public_const_changes_render_as_blocking_review_conflicts() {
    let live = temp_root("public-const-live");
    let baseline_cache = temp_root("public-const-baseline");
    let candidate_cache = temp_root("public-const-candidate");
    let build_root = temp_root("public-const-build");
    let context = ExternalSourceContext::derive(b"public-const-conflict-test");

    write_package(&live, "pub const LIMIT: u64 = 4;\n");
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public const baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public const baseline");

    write_package(&live, "pub const LIMIT: u64 = 5;\n");
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public const candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public const candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public const compatibility");
    assert_eq!(conflicts.conflict_count(), 1);
    let [package] = conflicts.packages() else {
        panic!("one changed package")
    };
    let [conflict] = package.conflicts() else {
        panic!("one changed public const row")
    };
    assert_eq!(conflict.kind(), PackageReviewCanonicalRowKind::PublicConst);
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    let baseline_locations = conflict
        .baseline_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("changed public const retains baseline source custody");
    let candidate_locations = conflict
        .candidate_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("changed public const retains candidate source custody");
    for locations in [baseline_locations, candidate_locations] {
        assert!(locations.iter().any(|location| {
            location.role() == omega_compiler::PackageReviewSourceLocationRole::ConstInitializer
                && location.relative_path() == "main.omg"
        }));
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public const conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V15\n"));
    assert!(rendered.contains("change changed\nkind public_const\nrisk blocking\n"));
    assert!(rendered.contains("baseline_location const_initializer package "));
    assert!(rendered.contains("candidate_location const_initializer package "));
    assert_ne!(conflict.fingerprint().digest(), [0; 32]);

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_operator_changes_render_as_blocking_review_conflicts() {
    let live = temp_root("public-operator-live");
    let baseline_cache = temp_root("public-operator-baseline");
    let candidate_cache = temp_root("public-operator-candidate");
    let build_root = temp_root("public-operator-build");
    let context = ExternalSourceContext::derive(b"public-operator-conflict-test");

    write_package(
        &live,
        "pub data Token [copy] { value: u64; }\npub operator < Token::less(left: Token, right: Token) -> bool;\n",
    );
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public operator baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public operator baseline");

    write_package(
        &live,
        "pub data Token [copy] { value: u64; }\npub operator < Token::less(left: Token, right: Token) -> bool\nrequires true;\n",
    );
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public operator candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public operator candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public operator compatibility");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicOperator)
        .expect("changed public operator row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public operator conflict");
    assert!(rendered.contains("change changed\nkind public_operator\nrisk blocking\n"));
    assert!(rendered.contains("candidate_location contract_clause package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_callable_parameter_changes_render_exact_parameter_locations() {
    let live = temp_root("public-callable-parameter-live");
    let baseline_cache = temp_root("public-callable-parameter-baseline");
    let candidate_cache = temp_root("public-callable-parameter-candidate");
    let build_root = temp_root("public-callable-parameter-build");
    let context = ExternalSourceContext::derive(b"public-callable-parameter-conflict-test");
    let baseline_source = "pub machine inspect(baseline_value: u32) -> u32 { baseline_value }\n";
    let candidate_source = "pub machine inspect(candidate_value: u32) -> u32 { candidate_value }\n";

    write_package(&live, baseline_source);
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public callable parameter baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public callable parameter baseline");

    write_package(&live, candidate_source);
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public callable parameter candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public callable parameter candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public callable parameter rows");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed public callable row");
    let expected_locations = [
        (
            conflict.baseline_source(),
            baseline_source,
            "baseline_value",
        ),
        (
            conflict.candidate_source(),
            candidate_source,
            "candidate_value",
        ),
    ];
    for (source, package_source, parameter) in expected_locations {
        let start = u64::try_from(
            package_source
                .find(parameter)
                .expect("parameter identifier in package source"),
        )
        .expect("parameter start fits review coordinate");
        let end = start
            + u64::try_from(parameter.len()).expect("parameter length fits review coordinate");
        let location = source
            .and_then(PackageReviewCanonicalRowSource::authored_locations)
            .expect("public callable source locations")
            .iter()
            .find(|location| {
                location.role()
                    == omega_compiler::PackageReviewSourceLocationRole::CallableParameter
            })
            .expect("exact callable parameter source location");
        assert_eq!(location.relative_path(), "main.omg");
        assert_eq!(location.start_byte(), start);
        assert_eq!(location.end_byte(), end);
    }

    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public callable parameter conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V15\n"));
    for (label, package_source, parameter) in [
        ("baseline", baseline_source, "baseline_value"),
        ("candidate", candidate_source, "candidate_value"),
    ] {
        let start = u64::try_from(
            package_source
                .find(parameter)
                .expect("rendered parameter identifier in package source"),
        )
        .expect("rendered parameter start fits review coordinate");
        let end = start
            + u64::try_from(parameter.len()).expect("parameter length fits review coordinate");
        let line = rendered
            .lines()
            .find(|line| line.starts_with(&format!("{label}_location callable_parameter package ")))
            .expect("rendered callable parameter location");
        assert!(line.ends_with(&format!(" {start} {end} \"main.omg\"")));
    }

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn callable_changes_render_exact_checked_body_call_locations() {
    let live = temp_root("body-call-live");
    let baseline_cache = temp_root("body-call-baseline");
    let candidate_cache = temp_root("body-call-candidate");
    let build_root = temp_root("body-call-build");
    let context = ExternalSourceContext::derive(b"body-call-conflict-test");
    let source = |target: &str| {
        format!(
            "machine first() {{ }}\nmachine second() {{ }}\npub machine run() {{ {target}(); }}\n"
        )
    };

    write_package(&live, &source("first"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve body-call baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile body-call baseline");

    write_package(&live, &source("second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve body-call candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile body-call candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare changed checked body call");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::Callable
                && conflict
                    .row_key()
                    .windows("run".len())
                    .any(|window| window == b"run")
        })
        .expect("changed run callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render checked body-call conflict");
    assert!(rendered.contains("baseline_location body_call package "));
    assert!(rendered.contains("candidate_location body_call package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_conformance_changes_render_as_blocking_review_conflicts() {
    let live = temp_root("public-conformance-live");
    let baseline_cache = temp_root("public-conformance-baseline");
    let candidate_cache = temp_root("public-conformance-candidate");
    let build_root = temp_root("public-conformance-build");
    let context = ExternalSourceContext::derive(b"public-conformance-conflict-test");

    let source = |argument: &str| {
        format!(
            r#"pub data First {{ }}
pub data Second {{ }}
pub trait Marker<Tag> {{ }}
pub Choice: First satisfies Marker<{argument}> {{ }}
"#
        )
    };
    write_package(&live, &source("First"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public conformance baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public conformance baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public conformance candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public conformance candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public conformance compatibility");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("changed public conformance row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    assert!(
        conflicts
            .render_bounded(1024 * 1024)
            .expect("render public conformance conflict")
            .contains("change changed\nkind public_conformance\nrisk blocking\n")
    );

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_trait_requirement_changes_render_exact_requirement_locations() {
    let live = temp_root("public-trait-requirement-live");
    let baseline_cache = temp_root("public-trait-requirement-baseline");
    let candidate_cache = temp_root("public-trait-requirement-candidate");
    let build_root = temp_root("public-trait-requirement-build");
    let context = ExternalSourceContext::derive(b"public-trait-requirement-conflict-test");

    let source = |parameter: &str| {
        format!(
            r#"pub trait Handler {{
    machine handle(value: {parameter}) -> u64;
}}
"#
        )
    };
    write_package(&live, &source("u32"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public trait requirement baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public trait requirement baseline");

    write_package(&live, &source("u64"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public trait requirement candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public trait requirement candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public trait requirement rows");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicTrait)
        .expect("changed public trait row");
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .expect("public trait source locations")
                .iter()
                .any(|location| {
                    location.role()
                        == omega_compiler::PackageReviewSourceLocationRole::TraitRequirement
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public trait requirement conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V15\n"));
    assert!(rendered.contains("baseline_location trait_requirement package "));
    assert!(rendered.contains("candidate_location trait_requirement package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_trait_parent_changes_render_exact_nested_review_locations() {
    let live = temp_root("public-trait-parent-live");
    let baseline_cache = temp_root("public-trait-parent-baseline");
    let candidate_cache = temp_root("public-trait-parent-candidate");
    let build_root = temp_root("public-trait-parent-build");
    let context = ExternalSourceContext::derive(b"public-trait-parent-conflict-test");

    let source = |parent: &str| {
        format!(
            r#"pub trait First {{ }}
pub trait Second {{ }}
pub trait Child: {parent} {{ }}
"#
        )
    };
    write_package(&live, &source("First"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public-trait baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public-trait baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public-trait candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public-trait candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public-trait compatibility");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicTrait)
        .expect("changed public-trait row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public-trait conflict");
    assert!(rendered.contains("change changed\nkind public_trait\nrisk blocking\n"));
    assert!(rendered.contains("baseline_location trait_parent package "));
    assert!(rendered.contains("candidate_location trait_parent package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_data_shape_changes_render_exact_member_locations() {
    let live = temp_root("public-data-member-live");
    let baseline_cache = temp_root("public-data-member-baseline");
    let candidate_cache = temp_root("public-data-member-candidate");
    let build_root = temp_root("public-data-member-build");
    let context = ExternalSourceContext::derive(b"public-data-member-conflict-test");

    write_package(&live, "pub data Packet { value: u32; }\n");
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public data baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public data baseline");

    write_package(&live, "pub data Packet { value: u64; }\n");
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public data candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public data candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public data rows");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicData)
        .expect("changed public data row");
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .expect("public data source locations")
                .iter()
                .any(|location| {
                    location.role() == omega_compiler::PackageReviewSourceLocationRole::DataMember
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public data member conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V15\n"));
    assert!(rendered.contains("baseline_location data_member package "));
    assert!(rendered.contains("candidate_location data_member package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn invocation_changes_render_exact_authored_target_locations() {
    let live = temp_root("invocation-location-live");
    let baseline_cache = temp_root("invocation-location-baseline");
    let candidate_cache = temp_root("invocation-location-candidate");
    let build_root = temp_root("invocation-location-build");
    let context = ExternalSourceContext::derive(b"invocation-location-conflict-test");

    let source = |service: &str| {
        format!(
            r#"pub boundary trait First {{ machine ping() reaches First; }}
pub boundary trait Second {{ machine ping() reaches Second; }}
pub machine dispatch()
reaches First + Second
invokes {service};
{{
    {service}::ping();
}}
"#
        )
    };
    write_package(&live, &source("First"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve invocation baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile invocation baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve invocation candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile invocation candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare invocation change");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render invocation conflict");
    assert!(rendered.contains("baseline_location synchronous_invocation package "));
    assert!(rendered.contains("candidate_location synchronous_invocation package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn service_reach_changes_render_exact_authored_target_locations() {
    let live = temp_root("service-reach-location-live");
    let baseline_cache = temp_root("service-reach-location-baseline");
    let candidate_cache = temp_root("service-reach-location-candidate");
    let build_root = temp_root("service-reach-location-build");
    let context = ExternalSourceContext::derive(b"service-reach-location-conflict-test");

    let source = |service: &str| {
        format!(
            r#"pub boundary trait First {{ machine ping() reaches First; }}
pub boundary trait Second {{ machine ping() reaches Second; }}
pub machine dispatch()
reaches {service}
{{ }}
"#
        )
    };
    write_package(&live, &source("First"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve service-reach baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile service-reach baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve service-reach candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile service-reach candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare service-reach change");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render service-reach conflict");
    assert!(rendered.contains("baseline_location service_reach package "));
    assert!(rendered.contains("candidate_location service_reach package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn operational_changes_render_exact_authored_clause_locations() {
    let live = temp_root("operational-location-live");
    let baseline_cache = temp_root("operational-location-baseline");
    let candidate_cache = temp_root("operational-location-candidate");
    let build_root = temp_root("operational-location-build");
    let context = ExternalSourceContext::derive(b"operational-location-conflict-test");

    let source = |clause: &str| format!("pub machine operate()\n{clause};\n{{ }}\n");
    write_package(&live, &source("suspends"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve operational baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile operational baseline");

    write_package(&live, &source("blocks"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve operational candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile operational candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare operational change");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render operational conflict");
    assert!(rendered.contains("baseline_location suspension package "));
    assert!(rendered.contains("candidate_location blocking package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn external_executable_supply_changes_render_as_opaque_blocking_conflicts() {
    let live = temp_root("external-supply-live");
    let baseline_cache = temp_root("external-supply-baseline");
    let candidate_cache = temp_root("external-supply-candidate");
    let build_root = temp_root("external-supply-build");
    let context = ExternalSourceContext::derive(b"external-supply-conflict-test");

    let source = |symbol: &str| {
        format!(
            r#"pub boundary trait ForeignSurface {{
    machine invoke() reaches ForeignSurface;
}}
pub machine invoke_leaf()
    satisfies ForeignSurface::invoke
    via Binding::DllImport("omega-host", "{symbol}");
"#,
        )
    };
    write_package(&live, &source("invoke_v1"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve external-supply baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile external-supply baseline");

    write_package(&live, &source("invoke_v2"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve external-supply candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile external-supply candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare external executable supply");
    let external_conflicts = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .filter(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply
        })
        .collect::<Vec<_>>();
    let [conflict] = external_conflicts.as_slice() else {
        panic!("expected exactly one external executable-supply conflict")
    };
    assert_eq!(
        conflict.risk(),
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    );
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    assert!(!conflicts.packages().iter().any(|package| {
        package
            .conflicts()
            .iter()
            .any(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
    }));
    assert!(
        conflicts
            .render_bounded(1024 * 1024)
            .expect("render external executable-supply conflict")
            .contains("change changed\nkind external_executable_supply\nrisk opaque_blocking\n")
    );

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn transparent_proposition_changes_render_exact_formula_custody() {
    let live = temp_root("transparent-proposition-live");
    let baseline_cache = temp_root("transparent-proposition-baseline");
    let candidate_cache = temp_root("transparent-proposition-candidate");
    let build_root = temp_root("transparent-proposition-build");
    let context = ExternalSourceContext::derive(b"transparent-proposition-conflict-test");

    write_package(&live, "pub proposition ready() = true;\n");
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve transparent proposition baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile transparent proposition baseline");

    write_package(&live, "pub proposition ready() = false;\n");
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve transparent proposition candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile transparent proposition candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare transparent proposition compatibility");
    let [package] = conflicts.packages() else {
        panic!("one changed package")
    };
    let [conflict] = package.conflicts() else {
        panic!("one changed transparent proposition row")
    };
    assert_eq!(
        conflict.kind(),
        PackageReviewCanonicalRowKind::PublicProposition
    );
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .unwrap()
                .iter()
                .any(|location| {
                    location.role()
                        == omega_compiler::PackageReviewSourceLocationRole::PropositionFormula
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render transparent proposition conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V15\n"));
    assert!(rendered.contains("baseline_location proposition_formula package "));
    assert!(rendered.contains("candidate_location proposition_formula package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_domain_changes_render_exact_proof_fact_custody() {
    let live = temp_root("public-domain-proof-fact-live");
    let baseline_cache = temp_root("public-domain-proof-fact-baseline");
    let candidate_cache = temp_root("public-domain-proof-fact-candidate");
    let build_root = temp_root("public-domain-proof-fact-build");
    let context = ExternalSourceContext::derive(b"public-domain-proof-fact-conflict-test");

    write_package(
        &live,
        "pub data Packet { value: u32; }\npub domain Packet::Ready\nrequires self.value == 0;\n",
    );
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public domain baseline");
    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
            .expect("compile public domain baseline");

    write_package(
        &live,
        "pub data Packet { value: u32; }\npub domain Packet::Ready\nrequires self.value == 1;\n",
    );
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public domain candidate");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
            .expect("compile public domain candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public domain proof facts");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicDomain)
        .expect("changed public domain row");
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .expect("public domain source locations")
                .iter()
                .any(|location| {
                    location.role() == omega_compiler::PackageReviewSourceLocationRole::ProofFact
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public domain proof-fact conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V15\n"));
    assert!(rendered.contains("baseline_location proof_fact package "));
    assert!(rendered.contains("candidate_location proof_fact package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}
