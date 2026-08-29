use super::*;

#[test]
fn git_update_escalating_to_process_authority_blocks_and_requests_source_audit() {
    let repository = temp_root("git-process-authority-repository");
    let baseline_cache = temp_root("git-process-authority-baseline-cache");
    let candidate_cache = temp_root("git-process-authority-candidate-cache");
    let compiler_workspace = temp_root("git-process-authority-compiler-workspace");
    let process_fixture = fixture_root().join("process-exit");
    std::fs::create_dir_all(&repository).expect("create process-authority repository");
    std::fs::copy(
        process_fixture.join("build.omg"),
        repository.join("build.omg"),
    )
    .expect("copy stable package declaration");
    std::fs::write(
        repository.join("main.omg"),
        r#"use omega::language::std::console;

pub machine terminate(console: Console, return_code: i32)
{
}
"#,
    )
    .expect("write inert baseline package");
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(
        &repository,
        ["commit", "--quiet", "-m", "inert process boundary"],
    );
    let baseline_revision = test_git_head(&repository);

    std::fs::copy(
        process_fixture.join("main.omg"),
        repository.join("main.omg"),
    )
    .expect("copy canonical process-authority candidate");
    run_test_git(&repository, ["add", "main.omg"]);
    run_test_git(
        &repository,
        ["commit", "--quiet", "-m", "exercise process authority"],
    );
    let candidate_revision = test_git_head(&repository);
    assert_ne!(baseline_revision, candidate_revision);

    let canonical_lineage = "https://github.com/CathedralOS/process-exit.git";
    let baseline_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(baseline_revision.clone()),
        canonical_lineage,
    )
    .expect("construct exact baseline Git request");
    let candidate_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(candidate_revision.clone()),
        canonical_lineage,
    )
    .expect("construct exact candidate Git request");
    let baseline_sources = resolve_git_package_closure(
        &baseline_request,
        omega_target::TargetProfile::CrossPlatformCli,
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve baseline Git custody");
    let candidate_sources = resolve_git_package_closure(
        &candidate_request,
        omega_target::TargetProfile::CrossPlatformCli,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve candidate Git custody");

    assert_eq!(
        baseline_sources.graph().root(),
        candidate_sources.graph().root(),
        "declared package identity and canonical Git lineage stay stable"
    );
    let baseline_custody = baseline_sources
        .custody(baseline_sources.graph().root())
        .expect("baseline root custody");
    let candidate_custody = candidate_sources
        .custody(candidate_sources.graph().root())
        .expect("candidate root custody");
    assert_ne!(
        baseline_custody.resolution(),
        candidate_custody.resolution()
    );
    assert_ne!(
        baseline_custody.snapshot_root(),
        candidate_custody.snapshot_root()
    );
    for (closure, expected_revision) in [
        (&baseline_sources, baseline_revision.as_str()),
        (&candidate_sources, candidate_revision.as_str()),
    ] {
        let PackageRootSourceRequest::Git(request) = closure.source_requests().root().request()
        else {
            panic!("authority update root must retain its exact Git request")
        };
        assert_eq!(request.requested_locator(), canonical_lineage);
        assert_eq!(request.requested_revision(), expected_revision);
        assert_eq!(request.transport_profile(), GitTransportProfile::TestFile);
        assert!(closure.source_requests().dependencies().next().is_none());
    }

    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &compiler_workspace)
            .expect("compile baseline package evidence");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &compiler_workspace)
            .expect("compile candidate package evidence");
    let baseline = baseline_reviews
        .review(baseline_sources.graph().root())
        .expect("baseline root review");
    let candidate = candidate_reviews
        .review(candidate_sources.graph().root())
        .expect("candidate root review");
    assert!(baseline.projection().dangerous_authorities().is_empty());
    let [authority] = candidate.projection().dangerous_authorities() else {
        panic!("candidate must derive one effective dangerous authority")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Process
    );
    assert_eq!(authority.service().path(), "Console");
    assert!(matches!(
        authority.service().owner(),
        PackageReviewNominalOwner::ToolchainSource(_)
    ));

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare compiler-derived authority escalation");
    let [package] = conflicts.packages() else {
        panic!("authority escalation must affect exactly one package")
    };
    assert_eq!(package.conflicts().len(), 2);
    let dangerous = package
        .conflicts()
        .iter()
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .expect("effective authority must produce its own canonical conflict");
    assert_eq!(
        dangerous.change(),
        ReviewOnlyCapabilityConflictChange::Added
    );
    assert_eq!(dangerous.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert!(dangerous.is_blocking());
    assert!(dangerous.baseline_row().is_none());
    assert!(dangerous.baseline_source().is_none());
    assert!(dangerous.candidate_row().is_some());
    let dangerous_locations = dangerous
        .candidate_source()
        .and_then(|source| source.authored_locations())
        .expect("dangerous authority retains compiler-owned source coordinates");
    assert!(dangerous_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
    }));
    assert!(dangerous_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityExposure
            && location.relative_path() == "main.omg"
    }));

    let callable = package
        .conflicts()
        .iter()
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed declared and realized reach must change the callable row");
    assert_eq!(
        callable.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert_eq!(callable.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert!(callable.is_blocking());
    for source in [
        callable
            .baseline_source()
            .expect("baseline callable source"),
        callable
            .candidate_source()
            .expect("candidate callable source"),
    ] {
        assert!(
            source
                .authored_locations()
                .expect("callable source locations")
                .iter()
                .any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                })
        );
    }

    let triage = triage_review_update(&baseline_reviews, &candidate_reviews, &BTreeSet::new());
    let [decision] = triage.decisions() else {
        panic!("authority escalation must produce one package decision")
    };
    assert_eq!(
        decision.disposition(),
        PackageTriageDisposition::BlockedCapabilityChange
    );
    assert_eq!(
        decision.reasons(),
        [
            PackageTriageReason::CapabilityOrApiChanged,
            PackageTriageReason::SourceChanged,
            PackageTriageReason::BuildObservationChanged,
            PackageTriageReason::RetainedDangerousAuthority(
                PackageReviewDangerousAuthorityClass::Process,
            ),
        ]
    );

    let review = assemble_update_source_review(
        &baseline_reviews,
        &candidate_reviews,
        baseline_sources.custodies(),
        &candidate_sources,
        PackageSourceReviewLimits::default(),
    )
    .expect("assemble exact source review for blocked escalation");
    let [patch] = review.source_patches() else {
        panic!("blocked authority escalation must carry one source patch")
    };
    assert_eq!(patch.baseline_key(), Some(baseline.key()));
    assert_eq!(patch.candidate_key(), candidate.key());
    assert_eq!(patch.changed_entries(), 1);
    assert!(!patch.requires_standalone_audit());
    assert!(patch.as_str().contains("mode update\n"));
    assert!(patch.as_str().contains("entry main.omg\n"));
    assert!(patch.as_str().contains("added lf reaches Console"));
    assert!(patch.as_str().contains("added lf invokes console;"));
    assert!(
        patch
            .as_str()
            .contains("added lf     console.exit_process(return_code);")
    );

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(compiler_workspace);
}
