use super::*;

#[test]
fn local_fixtures_issue_compiler_review_evidence_from_resolver_custody() {
    let fixtures = workspace_root().join("tests/fixtures/packages");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();

    for package in PACKAGES {
        let cache = temp_root(package);
        let closure = resolve_workspace_package_closure(
            &workspace_lineage,
            SourceRelativePath::parse(package).expect("fixture member path"),
            &fixtures,
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{package} source closure should resolve: {error}"));
        let reviews = compile_resolved_package_reviews(
            &closure,
            "windows_x86_64",
            &cache.join("compiler-build"),
        )
        .unwrap_or_else(|error| panic!("{package} package reviews should close: {error:#?}"));
        assert!(
            std::fs::read_dir(cache.join("compiler-build"))
                .expect("review build workspace remains readable")
                .next()
                .is_none(),
            "successful review must dispose its private build session"
        );

        assert_eq!(reviews.reviews().len(), closure.graph().packages().len());
        let mut closure_build_fuel = 0_u64;
        let mut closure_filesystem_attempts = 0_u64;
        for (package_index, node) in closure.graph().packages().iter().enumerate() {
            let custody = closure
                .custody(node.source().key())
                .expect("resolved graph package retains source custody");
            let issued = reviews
                .review(node.source().key())
                .expect("every resolved graph package receives compiler review material");
            let executes_filesystem_build =
                node.source().key().name().as_str() == "generated-table";
            assert_eq!(issued.resolution(), custody.resolution());
            let usage = issued
                .build_evaluation_usage()
                .expect("package review retains sponsored build-evaluation usage");
            assert_eq!(usage.usage_schema_version, 4);
            assert_eq!(usage.step_schedule_marker, 1);
            assert_eq!(
                usage.invocation_fuel_ceiling,
                if executes_filesystem_build {
                    10_000_000
                } else {
                    100_000
                }
            );
            assert_eq!(usage.sponsor_schema_version, Some(4));
            assert_eq!(usage.session_fuel_ceiling, Some(100_000_000));
            assert_eq!(usage.session_build_log_byte_ceiling, Some(16 * 1024 * 1024));
            assert_eq!(usage.session_filesystem_attempt_ceiling, Some(65_536));
            assert_eq!(usage.session_live_filesystem_handle_ceiling, Some(4_096));
            assert!(usage.session_peak_live_filesystem_handles <= 4_096);
            assert!(usage.fuel_units > 0);
            assert!(usage.fuel_units <= usage.invocation_fuel_ceiling);
            assert!(usage.replay_fuel_units <= usage.invocation_fuel_ceiling);
            assert_eq!(usage.build_log_bytes, 0);
            assert_eq!(usage.replay_build_log_bytes, 0);
            closure_build_fuel = closure_build_fuel
                .checked_add(usage.fuel_units)
                .and_then(|total| total.checked_add(usage.replay_fuel_units))
                .expect("fixture closure build fuel fits u64");
            assert_ne!(
                issued.source_consumption_commitment().digest(),
                [0; 32],
                "{} compiler-consumption commitment must be nonzero",
                node.source().key().name().as_str()
            );
            assert_eq!(
                issued.projection().package(),
                node.source().key().identity()
            );
            let observations = issued
                .build_observation_summary()
                .expect("fixture package build machine publishes observation evidence");
            assert_eq!(
                usage.filesystem_operation_attempts,
                u64::try_from(observations.filesystem_operation_attempts().len())
                    .expect("small fixture attempt count")
            );
            closure_filesystem_attempts = closure_filesystem_attempts
                .checked_add(usage.filesystem_operation_attempts)
                .and_then(|total| total.checked_add(usage.replay_filesystem_operation_attempts))
                .expect("fixture closure filesystem attempts fit u64");
            assert_eq!(
                observations.ceiling(),
                if executes_filesystem_build {
                    BuildObservationClass::Volatile
                } else {
                    BuildObservationClass::Hermetic
                }
            );
            assert_eq!(
                observations.realized(),
                if executes_filesystem_build {
                    BuildObservationClass::Receipted
                } else {
                    BuildObservationClass::Hermetic
                }
            );
            assert_eq!(
                observations.filesystem_operation_attempts().len(),
                if executes_filesystem_build { 6 } else { 0 },
                "only generated-table executes its declared filesystem build"
            );
            if executes_filesystem_build {
                assert!(
                    observations
                        .filesystem_replay_verdict()
                        .replays_source_inputs()
                );
                assert!(observations.filesystem_replay_verdict().is_complete());
                let [_, read, _, _, _, _] = observations.filesystem_operation_attempts() else {
                    panic!("generated-table retains its six filesystem attempts")
                };
                let [region] = read.observed_byte_regions() else {
                    panic!("generated-table retains one observed source-content region")
                };
                assert_eq!(
                    region.kind(),
                    BuildFilesystemObservedByteRegionKind::SequentialFileRead
                );
                assert_eq!(region.output_operand_ordinal(), 1);
                assert_eq!(region.offset(), 0);
                assert_eq!(region.length(), 24);
                assert_eq!(
                    read.observed_bytes(region),
                    Some(b"alpha=1\nbeta=2\ngamma=3\n\n".as_slice())
                );
            }
            let staged_output = observations
                .staged_output_tree()
                .expect("sponsored package review commits even an empty staged-output tree");
            assert_eq!(
                staged_output.entry_count(),
                usize::from(executes_filesystem_build) as u64
            );
            assert_eq!(
                staged_output.file_bytes(),
                if executes_filesystem_build { 42 } else { 0 }
            );
            let replay_root = cache.join(format!("review-output-replay-{package_index}"));
            std::fs::create_dir(&replay_root).expect("create fresh retained-output replay root");
            assert_eq!(
                staged_output
                    .materialize_into(&replay_root)
                    .expect("retained output must replay after review-session disposal"),
                staged_output.commitment()
            );
            if executes_filesystem_build {
                assert_eq!(
                    std::fs::read_to_string(replay_root.join("table.generated.omg")).unwrap(),
                    "pub machine table_size() -> u64 {\n    3\n}\n"
                );
            }
            std::fs::remove_dir_all(&replay_root)
                .expect("retained-output replay root remains removable");
            assert!(
                !issued.canonical_review_bytes().is_empty(),
                "{} review encoding must be nonempty",
                node.source().key().name().as_str()
            );
            let ledger_bytes = encode_ordinary_package_obligation_ledger(issued.obligations())
                .expect("compiler-issued review retains a canonical obligation ledger");
            let recovered = decode_ordinary_package_obligation_ledger(&ledger_bytes)
                .expect("retained obligation ledger should recover canonically");
            assert_eq!(&recovered, issued.obligations());
        }
        assert!(closure_build_fuel <= 100_000_000);
        assert!(closure_filesystem_attempts <= 65_536);

        let root_review = reviews
            .review(closure.graph().root())
            .expect("root package receives compiler review material");

        assert_fixture_evidence(package, root_review.projection());
        let initial_triage = triage_initial_install(&reviews);
        let initial_root = initial_triage
            .decisions()
            .iter()
            .find(|decision| decision.package_name() == *package)
            .expect("initial triage retains root package");
        let initial_disposition = match *package {
            "axiom-ledger" => PackageTriageDisposition::BlockedCapabilityChange,
            "generated-table" | "file-journal" | "process-exit" | "remote-journal"
            | "opaque-carrier" => PackageTriageDisposition::AdmittedWithAuditRecommended,
            _ => PackageTriageDisposition::Admitted,
        };
        assert_eq!(
            initial_root.disposition(),
            initial_disposition,
            "{package} initial source triage"
        );
        assert!(
            initial_triage.render_bounded(64 * 1024).is_ok(),
            "{package} compiler evidence should fit the bounded triage projection"
        );
        let missing_baseline = triage_update_without_admission_baseline(&reviews);
        assert_eq!(
            missing_baseline.disposition(),
            PackageTriageDisposition::BlockedMissingAdmissionBaseline,
            "{package} update without accepted admission evidence"
        );
        assert!(missing_baseline.decisions().iter().all(|decision| {
            decision
                .reasons()
                .contains(&PackageTriageReason::MissingAdmissionBaseline)
        }));
        let initial_review = assemble_initial_source_review(
            &reviews,
            &closure,
            omega_package_manager::review::PackageSourceReviewLimits::default(),
        )
        .expect("initial review input joins compiler rows to exact source custody");
        assert_eq!(
            initial_review
                .source_patches()
                .iter()
                .any(|patch| patch.candidate_key() == closure.graph().root()),
            initial_disposition != PackageTriageDisposition::Admitted,
            "{package} initial source packet follows compiler-derived audit policy"
        );
        let rendered_initial = initial_review
            .render_bounded(8 * 1024 * 1024)
            .expect("fixture initial review input stays bounded");
        assert!(!rendered_initial.contains(&cache.display().to_string()));
        let unchanged_triage = triage_review_update(&reviews, &reviews, &BTreeSet::new());
        let unchanged_root = unchanged_triage
            .decisions()
            .iter()
            .find(|decision| decision.package_name() == *package)
            .expect("unchanged triage retains root package");
        let retained_dangerous_authority = matches!(
            *package,
            "generated-table" | "file-journal" | "process-exit" | "remote-journal"
        );
        assert_eq!(
            unchanged_root.disposition(),
            if retained_dangerous_authority {
                PackageTriageDisposition::AdmittedWithAuditRecommended
            } else {
                PackageTriageDisposition::Admitted
            },
            "{package} unchanged source triage"
        );
        let unchanged_review = assemble_update_source_review(
            &reviews,
            &reviews,
            closure.custodies(),
            &closure,
            omega_package_manager::review::PackageSourceReviewLimits::default(),
        )
        .expect("unchanged review input joins exact baseline and candidate custody");
        assert!(
            unchanged_review.source_patches().is_empty(),
            "{package} unchanged custody needs no redundant source packet"
        );
        let unavailable = BTreeSet::from([closure.graph().root().clone()]);
        let unavailable_triage = triage_review_update(&reviews, &reviews, &unavailable);
        let unavailable_root = unavailable_triage
            .decisions()
            .iter()
            .find(|decision| decision.candidate_key() == Some(closure.graph().root()))
            .expect("unavailable-source triage retains exact root package");
        assert_eq!(
            unavailable_root.disposition(),
            PackageTriageDisposition::AdmittedWithAuditRecommended,
            "{package} missing old source must recommend standalone candidate audit"
        );
        let unavailable_review = assemble_update_source_review(
            &reviews,
            &reviews,
            &[],
            &closure,
            omega_package_manager::review::PackageSourceReviewLimits::default(),
        )
        .expect("missing old source retains compiler baseline and renders candidate custody");
        let unavailable_patch = unavailable_review
            .source_patches()
            .iter()
            .find(|patch| patch.candidate_key() == closure.graph().root())
            .expect("missing old root source receives a standalone source packet");
        assert!(unavailable_patch.baseline_key().is_none());
        if matches!(
            *package,
            "arithmetic-kernels" | "generated-table" | "graph-workbench"
        ) {
            let baseline = ReviewOnlyBaselineCapsule::capture(
                &closure,
                &reviews,
                ReviewOnlyBaselineLimits::default(),
            )
            .expect("capture fixture review baseline");
            let baseline = ReviewOnlyBaselineCapsule::decode(
                &baseline
                    .encode(ReviewOnlyBaselineLimits::default())
                    .expect("encode fixture review baseline"),
                ReviewOnlyBaselineLimits::default(),
            )
            .expect("recover fixture review baseline");
            if *package == "generated-table" {
                let generated = baseline
                    .packages()
                    .iter()
                    .find(|recovered| recovered.key() == closure.graph().root())
                    .expect("recovered generated-table baseline package");
                let replay = generated
                    .filesystem_replay_record()
                    .expect("generated-table baseline retains its verified replay receipt");
                assert!(!replay.canonical_bytes().is_empty());
                assert_ne!(replay.commitment(), [0; 32]);
            }
            let recovered_unchanged = assemble_update_source_review_from_baseline(
                &baseline,
                &reviews,
                closure.custodies(),
                &closure,
                omega_package_manager::review::PackageSourceReviewLimits::default(),
            )
            .expect("recovered baseline joins available old custody");
            assert_eq!(recovered_unchanged, unchanged_review);
            let recovered_unavailable = assemble_update_source_review_from_baseline(
                &baseline,
                &reviews,
                &[],
                &closure,
                omega_package_manager::review::PackageSourceReviewLimits::default(),
            )
            .expect("recovered baseline survives unavailable old source");
            assert_eq!(recovered_unavailable, unavailable_review);

            if *package == "arithmetic-kernels" {
                let baseline_directory_path = cache.join("review-baselines");
                let outside_directory = cache.join("outside-review-baselines");
                std::fs::create_dir(&baseline_directory_path)
                    .expect("create explicit review-baseline directory");
                std::fs::create_dir(&outside_directory)
                    .expect("create outside review-baseline directory");
                let baseline_directory_capability = cap_std::fs::Dir::open_ambient_dir(
                    &baseline_directory_path,
                    cap_std::ambient_authority(),
                )
                .expect("open explicit review-baseline directory capability");
                let baseline_directory = ReviewOnlyBaselineDirectory::from_capability(
                    baseline_directory_capability,
                    &baseline_directory_path,
                )
                .expect("bind explicit review-baseline directory capability");
                let baseline_name = ReviewOnlyBaselineName::parse("candidate.baseline")
                    .expect("canonical review-baseline filename");
                let baseline_limits = ReviewOnlyBaselineLimits::default();
                let encoded = baseline
                    .encode(baseline_limits)
                    .expect("encode persisted fixture baseline");
                baseline_directory
                    .persist_new_capsule(&baseline_name, &baseline, baseline_limits)
                    .expect("persist review-only baseline capsule");
                assert_eq!(
                    std::fs::read_dir(&baseline_directory_path)
                        .expect("review-baseline directory")
                        .count(),
                    1,
                    "successful baseline publication removes its private stage"
                );
                let reopened = baseline_directory
                    .recover_capsule(&baseline_name, baseline_limits)
                    .expect("recover review-only baseline capsule");
                assert_eq!(
                    reopened
                        .encode(baseline_limits)
                        .expect("reencode reopened baseline"),
                    encoded
                );
                assert_eq!(
                    compare_review_only_capabilities_from_baseline(
                        &reopened,
                        &reviews,
                        &closure,
                        ReviewOnlyCapabilityConflictLimits::default(),
                    )
                    .expect("reopened baseline comparison"),
                    compare_review_only_capabilities(
                        &reviews,
                        &reviews,
                        &closure,
                        ReviewOnlyCapabilityConflictLimits::default(),
                    )
                    .expect("live baseline comparison")
                );
                assert_eq!(
                    triage_review_update_from_baseline(&reopened, &reviews, &closure, &unavailable,),
                    unavailable_triage,
                    "reopened baseline preserves unavailable-source triage"
                );
                assert_eq!(
                    assemble_update_source_review_from_baseline(
                        &reopened,
                        &reviews,
                        &[],
                        &closure,
                        omega_package_manager::review::PackageSourceReviewLimits::default(),
                    )
                    .expect("reopened baseline preserves standalone source review"),
                    unavailable_review
                );
                assert!(matches!(
                    baseline_directory.persist_new_capsule(
                        &baseline_name,
                        &baseline,
                        baseline_limits
                    ),
                    Err(ReviewOnlyBaselineFileError::DestinationExists { .. })
                ));
                assert_eq!(
                    std::fs::read(baseline_directory_path.join(baseline_name.as_str()))
                        .expect("existing baseline remains readable"),
                    encoded
                );

                let too_small = ReviewOnlyBaselineLimits::new(
                    encoded.len() - 1,
                    1_024,
                    16_384,
                    128,
                    4 * 1024,
                    256,
                    65_536,
                    32 * 1024 * 1024,
                );
                assert!(matches!(
                    baseline_directory.recover_capsule(&baseline_name, too_small),
                    Err(ReviewOnlyBaselineFileError::ByteLimitExceeded { .. })
                ));

                for invalid_name in [
                    "",
                    "/candidate.baseline",
                    "nested/candidate.baseline",
                    "../candidate.baseline",
                    "candidate\\baseline",
                    "Candidate.baseline",
                    "candidate.",
                    "NUL.txt",
                    "COM1",
                ] {
                    assert_eq!(
                        ReviewOnlyBaselineName::parse(invalid_name),
                        Err(ReviewOnlyBaselineNameError::InvalidName),
                        "accepted noncanonical review-baseline name {invalid_name:?}"
                    );
                }
                assert_eq!(
                    ReviewOnlyBaselineName::parse(&"a".repeat(256)),
                    Err(ReviewOnlyBaselineNameError::InvalidName)
                );

                let corrupt_name =
                    ReviewOnlyBaselineName::parse("corrupt.baseline").expect("corrupt name");
                let mut corrupt = encoded.clone();
                corrupt[0] ^= 1;
                std::fs::write(baseline_directory_path.join(corrupt_name.as_str()), corrupt)
                    .expect("write corrupt review baseline");
                assert!(matches!(
                    baseline_directory.recover_capsule(&corrupt_name, baseline_limits),
                    Err(ReviewOnlyBaselineFileError::Capsule(_))
                ));

                let directory_name = ReviewOnlyBaselineName::parse("directory.baseline")
                    .expect("directory leaf name");
                std::fs::create_dir(baseline_directory_path.join(directory_name.as_str()))
                    .expect("create non-file baseline leaf");
                assert!(matches!(
                    baseline_directory.recover_capsule(&directory_name, baseline_limits),
                    Err(ReviewOnlyBaselineFileError::NotRegularFile { .. })
                ));

                #[cfg(unix)]
                {
                    use std::os::unix::fs::{PermissionsExt, symlink};

                    assert_eq!(
                        std::fs::metadata(baseline_directory_path.join(baseline_name.as_str()))
                            .expect("persisted baseline metadata")
                            .permissions()
                            .mode()
                            & 0o777,
                        0o600
                    );
                    let outside_record = outside_directory.join("outside.baseline");
                    std::fs::write(&outside_record, &encoded)
                        .expect("write outside review baseline");
                    let link_name =
                        ReviewOnlyBaselineName::parse("link.baseline").expect("baseline link name");
                    symlink(
                        &outside_record,
                        baseline_directory_path.join(link_name.as_str()),
                    )
                    .expect("create baseline leaf symlink");
                    assert!(matches!(
                        baseline_directory.recover_capsule(&link_name, baseline_limits),
                        Err(ReviewOnlyBaselineFileError::NotRegularFile { .. })
                    ));
                    assert!(matches!(
                        baseline_directory.persist_new_capsule(
                            &link_name,
                            &baseline,
                            baseline_limits
                        ),
                        Err(ReviewOnlyBaselineFileError::DestinationExists { .. })
                    ));
                    assert_eq!(
                        std::fs::read(outside_record).expect("outside baseline remains unchanged"),
                        encoded
                    );
                }
            }
        }
        let _ = std::fs::remove_dir_all(cache);
    }
}
