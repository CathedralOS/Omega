use omega_compiler::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};
use omega_packages::{
    ExternalSourceContext, LocalSourceLimits, PackageSourceClosureLimits, PackageTriageDisposition,
    PackageTriageReason, ReviewOnlyBaselineCapsule, ReviewOnlyBaselineLimits,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictLimits, compare_review_only_capabilities,
    compare_review_only_capabilities_from_baseline, compile_resolved_package_reviews,
    resolve_external_local_package_closure, triage_review_update,
    triage_review_update_from_baseline,
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

fn write_package(root: &Path, main: &str) {
    std::fs::create_dir_all(root).expect("create test package");
    std::fs::write(
        root.join("build.omg"),
        r#"const PACKAGE: Package = Package {
    name: "conflict-probe"
};

target windows_x64 { }

machine build(builder: &mut Build) {
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
    assert_eq!(conflicts.conflict_count(), 1);
    let package = &conflicts.packages()[0];
    assert_eq!(package.key(), candidate_sources.graph().root());
    assert!(package.dependency_path().steps().is_empty());
    assert_ne!(package.candidate_closure().digest(), [0; 32]);
    let [conflict] = package.conflicts() else {
        panic!("one added public proposition row")
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
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V3\n"));
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
        location.role() == omega_compiler::PackageReviewSourceLocationRole::AuthorityDeclaration
    }));
    assert!(slack_locations.iter().any(|location| {
        location.role() == omega_compiler::PackageReviewSourceLocationRole::AuthorityExposure
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
                omega_compiler::PackageReviewDangerousAuthorityClass::Filesystem,
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
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(representation_cache);
    let _ = std::fs::remove_dir_all(dangerous_slack_cache);
    let _ = std::fs::remove_dir_all(accepted_claim_baseline_cache);
    let _ = std::fs::remove_dir_all(accepted_claim_candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}
