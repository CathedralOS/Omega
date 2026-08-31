use omega_package_evidence::ledger::OrdinaryPackageObligationStatus;
use omega_package_evidence::record::PackageReviewCallableSupply;
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolveWorkspacePackageClosureError, ResolvedPackageSourceClosure,
    resolve_external_local_package_closure_with_storage,
    resolve_workspace_package_closure_with_storage,
};
use omega_package_manager::resolution::source::ResolvePackageSourceError;
use omega_package_manager::review::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionLimits,
    FreshPackageRootPolicyError, LocallyComposedPackageObligationResults,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDisposition,
    bind_fresh_package_root_policy, compare_review_only_initial_capabilities,
    compile_resolved_package_reviews, resolve_review_only_root_policy_decisions,
};
use omega_package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, SourceRelativePath,
    SourceResolverStorage,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const QUESTION_MAGIC: &[u8] = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join("tests/fixtures/packages").is_dir())
        .expect("omega-package-manager should live beneath the Omega workspace")
        .to_path_buf()
}

fn temporary_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-package-reconstruction-question-{label}-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ))
}

fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_workspace_package_closure_with_storage(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        source_limits,
        closure_limits,
    )
}

fn resolve_external_closure(
    live_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
) -> ResolvedPackageSourceClosure {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).expect("source storage");
    resolve_external_local_package_closure_with_storage(
        live_root,
        ExternalSourceContext::derive(b"open-claim-composition"),
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve external package closure")
}

fn graph_workbench_question() -> (
    PathBuf,
    ResolvedPackageSourceClosure,
    omega_package_manager::review::CompilerIssuedPackageReviewSet,
    CanonicalPackageReconstructionQuestion,
) {
    let temporary = temporary_root("graph-workbench");
    std::fs::create_dir_all(&temporary).expect("create temporary root");
    let fixture_root = workspace_root().join("tests/fixtures/packages");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let closure = resolve_workspace_package_closure(
        &workspace_lineage,
        SourceRelativePath::parse("graph-workbench").unwrap(),
        &fixture_root,
        temporary.join("cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve graph-workbench source closure");
    let reviews =
        compile_resolved_package_reviews(&closure, "windows_x86_64", &temporary.join("build"))
            .expect("compile graph-workbench package reviews");
    let question = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &closure,
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
    )
    .expect("associate source and obligation closure");
    (temporary, closure, reviews, question)
}

#[test]
fn canonical_question_round_trips_and_freshly_reconstructs_complete_closure() {
    let (temporary, closure, reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();

    assert_eq!(question.entries().len(), closure.graph().packages().len());
    assert_eq!(question.target_name(), "windows_x86_64");
    assert!(
        question
            .entries()
            .iter()
            .map(|entry| entry.package())
            .eq(question
                .source_closure()
                .packages()
                .iter()
                .map(|source| source.key()))
    );
    for entry in question.entries() {
        assert_eq!(entry.obligations().package(), entry.package().identity());
        let expected_transitive_packages = match entry.package().name().as_str() {
            "graph-workbench" => 3,
            "arithmetic-kernels" | "file-journal" => 1,
            package => panic!("unexpected graph-workbench package `{package}`"),
        };
        assert_eq!(
            entry.obligations().dependency_closure().packages().len(),
            expected_transitive_packages,
            "each ledger must retain its own exact transitive closure"
        );
    }

    let recovered =
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), limits)
            .expect("recover canonical reconstruction question");
    assert_eq!(recovered, question);
    assert_eq!(recovered.fingerprint(), question.fingerprint());
    assert!(
        recovered
            .matches_resolved_and_reviews(&closure, &reviews, limits)
            .expect("fresh source and review reconstruction should succeed")
    );
    let conflicts = compare_review_only_initial_capabilities(
        &reviews,
        &closure,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("derive graph-workbench fresh conflicts");
    let accepted_decisions = conflicts
        .packages()
        .iter()
        .flat_map(|package| {
            package
                .conflicts()
                .iter()
                .filter(|conflict| conflict.is_blocking())
                .map(|conflict| {
                    package
                        .root_policy_decision(
                            conflict,
                            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
                        )
                        .expect("bind graph-workbench blocking row")
                })
        })
        .collect::<Vec<_>>();
    assert!(
        !accepted_decisions.is_empty(),
        "fixture exercises additional non-claim blockers"
    );
    let accepted_policy =
        resolve_review_only_root_policy_decisions(&conflicts, &accepted_decisions)
            .expect("resolve every graph-workbench blocker");
    let accepted = bind_fresh_package_root_policy(
        &closure,
        &reviews,
        limits,
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect("every exact graph-workbench blocker is accepted");
    assert_eq!(accepted.root_policy(), Some(&accepted_policy));
    remove_temporary_tree(&temporary);
}

#[test]
fn fresh_closure_without_blockers_needs_no_synthetic_root_policy() {
    let temporary = temporary_root("no-root-policy");
    let root = temporary.join("root");
    std::fs::create_dir_all(&root).expect("create claim-free root");
    std::fs::write(
        root.join("build.omg"),
        r#"target windows_x86_64 { }

machine build(builder: &mut Build) {
    builder.package("claim-free");
}
"#,
    )
    .expect("write claim-free build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write claim-free source");

    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews =
        compile_resolved_package_reviews(&closure, "windows_x86_64", &temporary.join("build"))
            .expect("compile claim-free package");
    let accepted = bind_fresh_package_root_policy(
        &closure,
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        None,
    )
    .expect("claim-free closure needs no synthetic root policy");
    assert!(accepted.root_policy().is_none());
    assert!(accepted.conflicts().is_empty());
    assert!(
        accepted
            .obligations()
            .root_open_accepted_claims()
            .next()
            .is_none()
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn dependency_open_claims_require_exact_fresh_root_policy() {
    let temporary = temporary_root("open-claim-composition");
    let dependency = temporary.join("dependency");
    let root = temporary.join("root");
    std::fs::create_dir_all(&dependency).expect("create claim dependency");
    std::fs::create_dir_all(&root).expect("create claim consumer");
    std::fs::write(
        dependency.join("build.omg"),
        r#"target windows_x86_64 { }

machine build(builder: &mut Build) {
    builder.package("claim-dependency");
}
"#,
    )
    .expect("write claim dependency build");
    std::fs::write(
        dependency.join("main.omg"),
        r#"boundary machine trusted_zero() -> u64
ensures result == 0;
"#,
    )
    .expect("write open accepted claim");
    std::fs::write(
        root.join("build.omg"),
        r#"target windows_x86_64 { }

machine build(builder: &mut Build) {
    builder.package("claim-consumer");
    builder.depend(Source::Path {
        location: "../dependency"
    });
}
"#,
    )
    .expect("write claim consumer build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write claim consumer source");

    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews =
        compile_resolved_package_reviews(&closure, "windows_x86_64", &temporary.join("build"))
            .expect("compile two-package claim closure");
    let composed = LocallyComposedPackageObligationResults::from_resolved_and_reviews(
        &closure,
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
    )
    .expect("compose locally reconstructed open claims");

    assert_eq!(composed.entries().len(), 2);
    let root_entry = composed
        .entries()
        .iter()
        .find(|entry| entry.package() == closure.graph().root())
        .expect("selected root result");
    assert!(root_entry.results().open_accepted_claims().is_empty());
    let propagated = composed.root_open_accepted_claims().collect::<Vec<_>>();
    let [(owner, claim)] = propagated.as_slice() else {
        panic!("one dependency claim must propagate to the root")
    };
    assert_eq!(owner.name().as_str(), "claim-dependency");
    assert_eq!(
        claim.status(),
        OrdinaryPackageObligationStatus::OpenRootAdmission
    );
    assert_eq!(
        claim.callable().supply(),
        PackageReviewCallableSupply::AdmissionClaim
    );
    assert_eq!(
        claim.row().kind(),
        omega_package_evidence::record::PackageReviewCanonicalRowKind::AcceptedClaim
    );
    assert_eq!(
        composed.question().source_closure().packages().len(),
        closure.graph().packages().len()
    );

    let conflicts = compare_review_only_initial_capabilities(
        &reviews,
        &closure,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("derive exact fresh-admission conflicts");
    let claim_package = conflicts
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "claim-dependency")
        .expect("dependency owns its accepted-claim conflict");
    let claim_conflict = claim_package
        .conflicts()
        .iter()
        .find(|conflict| {
            conflict.kind()
                == omega_package_evidence::record::PackageReviewCanonicalRowKind::AcceptedClaim
        })
        .expect("fresh accepted claim is blocking");

    assert!(matches!(
        bind_fresh_package_root_policy(
            &closure,
            &reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            ReviewOnlyCapabilityConflictLimits::default(),
            None,
        ),
        Err(FreshPackageRootPolicyError::MissingRootPolicy)
    ));

    let rejected_decision = claim_package
        .root_policy_decision(
            claim_conflict,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        )
        .expect("bind rejection to exact accepted claim");
    let rejected_policy =
        resolve_review_only_root_policy_decisions(&conflicts, &[rejected_decision])
            .expect("complete rejecting policy");
    assert!(matches!(
        bind_fresh_package_root_policy(
            &closure,
            &reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            ReviewOnlyCapabilityConflictLimits::default(),
            Some(&rejected_policy),
        ),
        Err(FreshPackageRootPolicyError::RejectedBlockingConflict)
    ));

    let accepted_decision = claim_package
        .root_policy_decision(
            claim_conflict,
            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        )
        .expect("bind acceptance to exact accepted claim");
    let accepted_policy =
        resolve_review_only_root_policy_decisions(&conflicts, &[accepted_decision])
            .expect("complete accepting policy");
    let accepted = bind_fresh_package_root_policy(
        &closure,
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect("exact fresh policy admits the open claim in memory");
    assert_eq!(accepted.root_policy(), Some(&accepted_policy));
    let accepted_claims = accepted
        .obligations()
        .root_open_accepted_claims()
        .collect::<Vec<_>>();
    let [(owner, _)] = accepted_claims.as_slice() else {
        panic!("one accepted dependency claim")
    };
    assert_eq!(owner.name().as_str(), "claim-dependency");

    std::fs::write(
        dependency.join("main.omg"),
        r#"boundary machine trusted_zero() -> u64
ensures result == 1;
"#,
    )
    .expect("change accepted claim");
    let changed_closure = resolve_external_closure(&root, temporary.join("changed-cache"));
    let changed_reviews = compile_resolved_package_reviews(
        &changed_closure,
        "windows_x86_64",
        &temporary.join("changed-build"),
    )
    .expect("compile changed claim closure");
    assert!(matches!(
        bind_fresh_package_root_policy(
            &changed_closure,
            &changed_reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            ReviewOnlyCapabilityConflictLimits::default(),
            Some(&accepted_policy),
        ),
        Err(FreshPackageRootPolicyError::InvalidRootPolicy(_))
    ));

    remove_temporary_tree(&temporary);
}

#[test]
fn exact_nested_source_request_changes_question_with_identical_ledgers_and_fresh_match_rejects() {
    let (temporary, _closure, reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let fixture_root = workspace_root().join("tests/fixtures/packages");
    let alternate_request_spelling = fixture_root.join(".");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let alternate_closure = resolve_workspace_package_closure(
        &workspace_lineage,
        SourceRelativePath::parse("graph-workbench").unwrap(),
        &alternate_request_spelling,
        temporary.join("alternate-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve the same source through alternate exact request spelling");
    let alternate = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &alternate_closure,
        &reviews,
        limits,
    )
    .expect("reuse identical locally reconstructed ledgers with alternate source request");

    assert_ne!(
        question.source_closure().canonical_bytes(),
        alternate.source_closure().canonical_bytes(),
        "exact caller request spelling belongs to the nested source subject"
    );
    assert!(
        question
            .entries()
            .iter()
            .map(|entry| entry.obligations())
            .eq(alternate.entries().iter().map(|entry| entry.obligations())),
        "the alternate question reuses byte-identical obligation ledgers"
    );
    assert_ne!(question.canonical_bytes(), alternate.canonical_bytes());
    assert_ne!(question.fingerprint(), alternate.fingerprint());
    assert!(
        !question
            .matches_resolved_and_reviews(&alternate_closure, &reviews, limits)
            .expect("alternate fresh reconstruction remains structurally valid"),
        "fresh match must reject a different exact source question"
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn recovery_rejects_missing_duplicate_reordered_and_source_inconsistent_ledgers() {
    let (temporary, _closure, _reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let (version, source, ledgers) = split_question(question.canonical_bytes());
    assert_eq!(ledgers.len(), 3);

    let mut missing = ledgers.clone();
    missing.pop();
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &missing),
            limits,
        )
        .is_err()
    );

    let mut duplicate = ledgers.clone();
    duplicate[2] = duplicate[0].clone();
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &duplicate),
            limits,
        )
        .is_err()
    );

    let mut reordered = ledgers.clone();
    reordered.swap(0, 1);
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &reordered),
            limits,
        )
        .is_err()
    );

    let mut changed_alias = ledgers.clone();
    let graph_ledger = changed_alias
        .iter_mut()
        .find(|ledger| find_subslice(ledger, b"arithmetic_kernels").is_some())
        .expect("root ledger retains dependency alias");
    let alias_offset =
        find_subslice(graph_ledger, b"arithmetic_kernels").expect("root ledger alias offset");
    graph_ledger[alias_offset] = b'b';
    let error = CanonicalPackageReconstructionQuestion::recover(
        &join_question(version, &source, &changed_alias),
        limits,
    )
    .expect_err("source-inconsistent dependency alias must reject");
    assert_eq!(
        error.message(),
        "obligation ledger dependency edges do not match the source subject"
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn recovery_rejects_unknown_version_trailing_bytes_and_resource_violations() {
    let (temporary, _closure, _reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();

    let mut unknown_version = question.canonical_bytes().to_vec();
    unknown_version[QUESTION_MAGIC.len()..QUESTION_MAGIC.len() + 2]
        .copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(CanonicalPackageReconstructionQuestion::recover(&unknown_version, limits).is_err());

    let mut trailing = question.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(CanonicalPackageReconstructionQuestion::recover(&trailing, limits).is_err());

    let record_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_record_bytes: question.canonical_bytes().len() - 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), record_bound,)
            .is_err()
    );

    let package_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_packages: question.entries().len() - 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), package_bound,)
            .is_err()
    );

    let ledger_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_ledger_bytes: 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), ledger_bound,)
            .is_err()
    );

    let aggregate_ledger_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_total_ledger_bytes: 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            question.canonical_bytes(),
            aggregate_ledger_bound,
        )
        .is_err()
    );

    remove_temporary_tree(&temporary);
}

fn split_question(bytes: &[u8]) -> (u16, Vec<u8>, Vec<Vec<u8>>) {
    let mut offset = 0usize;
    assert_eq!(&bytes[..QUESTION_MAGIC.len()], QUESTION_MAGIC);
    offset += QUESTION_MAGIC.len();
    let version = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    offset += 2;
    let source = take_frame(bytes, &mut offset).to_vec();
    let ledger_count = take_u32(bytes, &mut offset) as usize;
    let ledgers = (0..ledger_count)
        .map(|_| take_frame(bytes, &mut offset).to_vec())
        .collect::<Vec<_>>();
    assert_eq!(offset, bytes.len());
    (version, source, ledgers)
}

fn join_question(version: u16, source: &[u8], ledgers: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(QUESTION_MAGIC);
    bytes.extend_from_slice(&version.to_le_bytes());
    push_frame(&mut bytes, source);
    bytes.extend_from_slice(&(ledgers.len() as u32).to_le_bytes());
    for ledger in ledgers {
        push_frame(&mut bytes, ledger);
    }
    bytes
}

fn take_u32(bytes: &[u8], offset: &mut usize) -> u32 {
    let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    value
}

fn take_frame<'bytes>(bytes: &'bytes [u8], offset: &mut usize) -> &'bytes [u8] {
    let length = take_u32(bytes, offset) as usize;
    let framed = &bytes[*offset..*offset + length];
    *offset += length;
    framed
}

fn push_frame(bytes: &mut Vec<u8>, framed: &[u8]) {
    bytes.extend_from_slice(&(framed.len() as u32).to_le_bytes());
    bytes.extend_from_slice(framed);
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|candidate| candidate == needle)
}

fn remove_temporary_tree(root: &std::path::Path) {
    make_tree_owner_writable(root);
    std::fs::remove_dir_all(root).expect("remove temporary root");
}

#[cfg(unix)]
fn make_tree_owner_writable(root: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    if !metadata.is_dir() {
        return;
    }
    let mode = metadata.permissions().mode() | 0o700;
    let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode));
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            make_tree_owner_writable(&entry.path());
        }
    }
}

#[cfg(windows)]
fn make_tree_owner_writable(root: &std::path::Path) {
    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    let _ = std::fs::set_permissions(root, permissions);
    if metadata.is_dir() {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                make_tree_owner_writable(&entry.path());
            }
        }
    }
}
