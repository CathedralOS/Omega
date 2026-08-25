use omega_compiler::{
    BuildObservationClass, CheckedPackageReviewProjection, PackageReviewCallableRole,
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCheckedServiceReach,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewPropositionEvidence, PackageReviewRepresentationAbiCommitment,
    PackageReviewRepresentationMechanism, PackageReviewSourceLocationRole,
};
use omega_packages::{
    CompileResolvedPackageReviewsError, LocalSourceLimits, PackageSourceClosureLimits,
    PackageSourceVerificationPhase, PackageTriageDisposition, PackageTriageReason,
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineLimits, SourceLineage, SourceResolveError,
    WorkspaceMemberPath, assemble_initial_source_review, assemble_update_source_review,
    assemble_update_source_review_from_baseline, compile_resolved_package_reviews,
    resolve_workspace_package_closure, triage_initial_install, triage_review_update,
    triage_update_without_admission_baseline,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const PACKAGES: &[&str] = &[
    "arithmetic-kernels",
    "generated-table",
    "file-journal",
    "process-exit",
    "network-overreach",
    "remote-journal",
    "axiom-ledger",
    "opaque-carrier",
    "provider-switchboard",
    "capability-vault",
    "graph-workbench",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect("omega-packages should live under the Omega workspace")
        .to_path_buf()
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-evidence-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn assert_fixture_evidence(package: &str, review: &CheckedPackageReviewProjection) {
    let (trait_count, data_count, callable_count, role) = match package {
        "file-journal" => (0, 1, 2, PackageReviewCallableRole::Public),
        "provider-switchboard" => (1, 1, 2, PackageReviewCallableRole::Public),
        "remote-journal" => (1, 1, 2, PackageReviewCallableRole::Public),
        "capability-vault" => (2, 1, 2, PackageReviewCallableRole::Public),
        "network-overreach" => (1, 0, 2, PackageReviewCallableRole::Public),
        "axiom-ledger" => (0, 0, 2, PackageReviewCallableRole::Boundary),
        "opaque-carrier" => (0, 1, 2, PackageReviewCallableRole::Boundary),
        "generated-table" => (0, 0, 2, PackageReviewCallableRole::Public),
        _ => (0, 0, 2, PackageReviewCallableRole::Public),
    };
    assert_eq!(
        review.public_traits().len(),
        trait_count,
        "{package} traits"
    );
    assert_eq!(review.public_data().len(), data_count, "{package} data");
    assert_eq!(
        review.callables().len(),
        callable_count,
        "{package} callables"
    );
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.role() == role)
        .unwrap_or_else(|| panic!("{package} intended review callable"));

    let expected_dangerous_authority = match package {
        "generated-table" | "file-journal" | "remote-journal" => Some((
            PackageReviewDangerousAuthorityClass::Filesystem,
            "FilesystemHost",
        )),
        "process-exit" => Some((PackageReviewDangerousAuthorityClass::Process, "Console")),
        _ => None,
    };
    assert_eq!(
        review.dangerous_authorities().len(),
        usize::from(expected_dangerous_authority.is_some()),
        "{package} dangerous authorities"
    );
    if let Some((expected_class, expected_service)) = expected_dangerous_authority {
        let [authority] = review.dangerous_authorities() else {
            panic!("{package} exact dangerous authority")
        };
        assert_eq!(authority.class(), expected_class);
        let PackageReviewNominalOwner::ToolchainSource(source) = authority.service().owner() else {
            panic!("{package} dangerous authority must retain exact toolchain source")
        };
        assert_ne!(source.digest(), [0; 32]);
        assert_eq!(authority.service().path(), expected_service);
    }
    assert_eq!(
        review.dangerous_authority_slack().len(),
        0,
        "{package} dangerous authority slack"
    );

    match package {
        "generated-table" => {
            let build = review
                .callables()
                .iter()
                .find(|callable| callable.role() == PackageReviewCallableRole::Build)
                .expect("generated-table canonical build row");
            let [service] = build.declared_service_reach().expect("build reach ceiling") else {
                panic!("generated-table exact build service reach")
            };
            assert_eq!(service.path(), "FilesystemHost");
            assert!(matches!(
                build.checked_service_reach(),
                PackageReviewCheckedServiceReach::CheckedBody {
                    realized,
                    concrete,
                } if realized.as_slice() == [service.clone()]
                    && concrete.as_slice() == [service.clone()]
            ));
            let invocations = build
                .declared_synchronous_invocations()
                .expect("build invocation ceiling");
            assert_eq!(invocations.len(), 1);
            assert_eq!(
                invocations[0]
                    .service()
                    .expect("build service invocation ceiling")
                    .path(),
                "FilesystemHost"
            );
            assert_eq!(build.realized_synchronous_invocations().len(), 1);
            assert!(
                build
                    .realized_synchronous_invocations()
                    .iter()
                    .all(|invocation| {
                        invocation
                            .service()
                            .is_some_and(|realized| realized == service)
                    })
            );
        }
        "file-journal" => {
            let [service] = callable.declared_service_reach().expect("published reach") else {
                panic!("file-journal exact filesystem reach")
            };
            assert_eq!(service.path(), "FilesystemHost");
            let [invocation] = callable
                .declared_synchronous_invocations()
                .expect("published invocation")
            else {
                panic!("file-journal exact filesystem invocation")
            };
            assert_eq!(
                invocation.service().expect("service invocation").path(),
                "FilesystemHost"
            );
        }
        "process-exit" => {
            let [service] = callable.declared_service_reach().expect("published reach") else {
                panic!("process-exit exact Console reach")
            };
            assert_eq!(service.path(), "Console");
            let [invocation] = callable
                .declared_synchronous_invocations()
                .expect("published invocation")
            else {
                panic!("process-exit exact Console invocation")
            };
            assert_eq!(invocation.parameter(), Some(0));
        }
        "network-overreach" => {
            let [service] = callable.declared_service_reach().expect("published reach") else {
                panic!("network-overreach exact network reach")
            };
            assert_eq!(service.path(), "NetworkHost");
            assert!(
                callable
                    .declared_synchronous_invocations()
                    .expect("published invocation ceiling")
                    .is_empty()
                    && callable.realized_synchronous_invocations().is_empty(),
                "network-overreach must retain reach without a hidden invocation"
            );
        }
        "remote-journal" => {
            let reach = callable.declared_service_reach().expect("published reach");
            assert_eq!(reach.len(), 2, "remote-journal exact dangerous reach");
            assert!(
                reach
                    .iter()
                    .any(|service| service.path() == "FilesystemHost")
            );
            assert!(reach.iter().any(|service| service.path() == "NetworkHost"));
            let invocations = callable
                .declared_synchronous_invocations()
                .expect("published invocation");
            assert_eq!(
                invocations.len(),
                2,
                "remote-journal exact dangerous invocations"
            );
            assert!(invocations.iter().any(|invocation| {
                invocation
                    .service()
                    .is_some_and(|service| service.path() == "FilesystemHost")
            }));
            assert!(invocations.iter().any(|invocation| {
                invocation
                    .service()
                    .is_some_and(|service| service.path() == "NetworkHost")
            }));
        }
        "provider-switchboard" => {
            let [service] = callable.declared_service_reach().expect("published reach") else {
                panic!("provider-switchboard exact clock reach")
            };
            assert_eq!(service.path(), "ClockHost");
            let [invocation] = callable
                .declared_synchronous_invocations()
                .expect("published invocation")
            else {
                panic!("provider-switchboard exact clock invocation")
            };
            assert_eq!(
                invocation.service().expect("service invocation").path(),
                "ClockHost"
            );
            let [provider] = review.selected_providers() else {
                panic!("provider-switchboard exact selected provider")
            };
            assert_eq!(provider.provider_type(), "MonotonicClock");
            assert_eq!(provider.service_schema(), "ClockHost");
            assert_eq!(provider.rows().len(), 1);
            let rows = review
                .canonical_rows()
                .expect("provider-switchboard canonical rows");
            let selected = rows
                .iter()
                .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
                .expect("selected-provider canonical row");
            let locations = selected
                .source()
                .authored_locations()
                .expect("selected-provider row must retain authored provenance");
            assert!(locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ProviderSelection
                    && location.relative_path() == "build.omg"
            }));
            assert!(locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ProviderSchemaDeclaration
                    && location.relative_path() == "main.omg"
            }));
            assert!(locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ProviderTypeDeclaration
                    && location.relative_path() == "main.omg"
            }));
            assert!(locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ProviderRequirementDeclaration
                    && location.relative_path() == "main.omg"
            }));
            assert!(locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ProviderRealization
                    && location.relative_path() == "main.omg"
            }));
            assert!(selected.source().compiler_derivations().is_empty());
        }
        "capability-vault" => {
            let flows = callable.capability_flows();
            let acquisitions = flows
                .iter()
                .filter(|flow| flow.kind().as_str() == "acquires")
                .collect::<Vec<_>>();
            let [acquisition] = acquisitions.as_slice() else {
                panic!("capability-vault must retain one exact capability acquisition")
            };
            assert_eq!(acquisition.capability().path(), "SecretHost");
            let returns = flows
                .iter()
                .filter(|flow| flow.kind().as_str() == "returns")
                .collect::<Vec<_>>();
            let [returned] = returns.as_slice() else {
                panic!("capability-vault must retain one exact capability return")
            };
            assert_eq!(returned.capability().path(), "SecretHost");
            for flow in flows {
                assert_eq!(flow.capability().path(), "SecretHost");
                assert_eq!(flow.state().path(), "Vault::open_secret::open_secret");
                assert_eq!(
                    flow.capability().owner(),
                    PackageReviewNominalOwner::Package(review.package()),
                    "capability identity must retain exact package provenance"
                );
                assert_eq!(
                    flow.state().owner(),
                    PackageReviewNominalOwner::Package(review.package()),
                    "state identity must retain exact package provenance"
                );
                if let Some(via) = flow.via_state() {
                    assert_eq!(
                        via.owner(),
                        PackageReviewNominalOwner::Package(review.package()),
                        "capability-flow intermediary must retain exact package provenance"
                    );
                }
            }
        }
        "axiom-ledger" => {
            let [contract] = callable.contracts() else {
                panic!("axiom-ledger exact accepted claim")
            };
            assert_eq!(contract.kind(), PackageReviewContractKind::Ensures);
            let PackageReviewContractFact::Proposition(application) = contract.fact() else {
                panic!("axiom-ledger exact accepted proposition")
            };
            assert_eq!(application.declaration().path(), "is_zero");
            assert_eq!(
                application.arguments(),
                [PackageReviewContractExpression::Result]
            );
            assert_eq!(
                application.evidence(),
                &PackageReviewPropositionEvidence::FactOnly
            );
            let canonical_rows = review
                .canonical_rows()
                .expect("axiom-ledger canonical rows");
            let accepted_claims = canonical_rows
                .iter()
                .filter(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
                .collect::<Vec<_>>();
            let [accepted_claim] = accepted_claims.as_slice() else {
                panic!("axiom-ledger exact accepted-claim row")
            };
            assert_eq!(
                accepted_claim.risk(),
                PackageReviewCanonicalRowRisk::Blocking
            );
            assert!(
                accepted_claim
                    .source()
                    .authored_locations()
                    .expect("accepted claim source")
                    .iter()
                    .any(|location| {
                        location.role() == PackageReviewSourceLocationRole::Declaration
                            && location.relative_path() == "main.omg"
                    })
            );
        }
        "opaque-carrier" => {
            let [opaque] = review.public_data() else {
                panic!("opaque-carrier exact public data row")
            };
            assert_eq!(opaque.identity().path(), "PlatformToken");
            assert_ne!(
                opaque.supply(),
                Default::default(),
                "opaque-carrier supply must not collapse to an ordinary checked shape"
            );
            assert!(opaque.members().is_empty());
            let [representation] = review.representation_tcb() else {
                panic!("opaque-carrier exact representation-TCB row")
            };
            assert_eq!(representation.declaration(), opaque.identity());
            assert_eq!(
                representation.abi(),
                PackageReviewRepresentationAbiCommitment::Unbound
            );
            assert_eq!(
                representation.mechanism(),
                PackageReviewRepresentationMechanism::Unbound
            );
            assert!(callable.contracts().is_empty());
            assert!(
                callable
                    .declared_service_reach()
                    .expect("claim-free boundary publishes an empty reach ceiling")
                    .is_empty()
            );
        }
        _ => {}
    }
    if package != "opaque-carrier" {
        assert!(
            review.representation_tcb().is_empty(),
            "{package} must not fabricate opaque representation evidence"
        );
    }
}

#[test]
fn local_fixtures_issue_compiler_review_evidence_from_resolver_custody() {
    let fixtures = workspace_root().join("fixtures/packages");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();

    for package in PACKAGES {
        let cache = temp_root(package);
        let closure = resolve_workspace_package_closure(
            &workspace_lineage,
            WorkspaceMemberPath::parse(package).expect("fixture member path"),
            &fixtures,
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{package} source closure should resolve: {error}"));
        let reviews = compile_resolved_package_reviews(
            &closure,
            "windows_x64",
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
        let compiler_executable_commitment = reviews
            .reviews()
            .first()
            .expect("nonempty package closure receives review material")
            .compiler_executable_commitment();
        assert_ne!(
            compiler_executable_commitment.digest(),
            [0; 32],
            "review set must identify its observed producer executable"
        );
        for (package_index, node) in closure.graph().packages().iter().enumerate() {
            let custody = closure
                .custody(node.source().key())
                .expect("resolved graph package retains source custody");
            let issued = reviews
                .review(node.source().key())
                .expect("every resolved graph package receives compiler review material");
            assert_eq!(issued.resolution(), custody.resolution());
            assert_eq!(
                issued.compiler_executable_commitment(),
                compiler_executable_commitment,
                "every row in one review operation must retain the same producer executable"
            );
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
            let executes_filesystem_build =
                node.source().key().name().as_str() == "generated-table";
            let expected_class = if executes_filesystem_build {
                BuildObservationClass::Volatile
            } else {
                BuildObservationClass::Hermetic
            };
            assert_eq!(observations.ceiling(), expected_class);
            assert_eq!(observations.realized(), expected_class);
            assert_eq!(
                observations.filesystem_operation_attempts().len(),
                if executes_filesystem_build { 6 } else { 0 },
                "only generated-table executes its declared filesystem build"
            );
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
        }

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
            omega_packages::PackageSourceReviewLimits::default(),
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
            omega_packages::PackageSourceReviewLimits::default(),
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
            omega_packages::PackageSourceReviewLimits::default(),
        )
        .expect("missing old source retains compiler baseline and renders candidate custody");
        let unavailable_patch = unavailable_review
            .source_patches()
            .iter()
            .find(|patch| patch.candidate_key() == closure.graph().root())
            .expect("missing old root source receives a standalone source packet");
        assert!(unavailable_patch.baseline_key().is_none());
        if matches!(*package, "arithmetic-kernels" | "graph-workbench") {
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
            let recovered_unchanged = assemble_update_source_review_from_baseline(
                &baseline,
                &reviews,
                closure.custodies(),
                &closure,
                omega_packages::PackageSourceReviewLimits::default(),
            )
            .expect("recovered baseline joins available old custody");
            assert_eq!(recovered_unchanged, unchanged_review);
            let recovered_unavailable = assemble_update_source_review_from_baseline(
                &baseline,
                &reviews,
                &[],
                &closure,
                omega_packages::PackageSourceReviewLimits::default(),
            )
            .expect("recovered baseline survives unavailable old source");
            assert_eq!(recovered_unavailable, unavailable_review);
        }
        let _ = std::fs::remove_dir_all(cache);
    }
}

#[test]
fn review_compilation_rejects_snapshot_tampering_before_compiler_consumption() {
    let fixtures = workspace_root().join("fixtures/packages");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let cache = temp_root("tampered-custody");
    let closure = resolve_workspace_package_closure(
        &workspace_lineage,
        WorkspaceMemberPath::parse("arithmetic-kernels").unwrap(),
        &fixtures,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("fixture source closure should resolve");
    let root = closure.graph().root().clone();
    let main = closure
        .source_root(&root)
        .expect("root custody")
        .join("main.omg");
    let mut permissions = std::fs::metadata(&main).unwrap().permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(&main, permissions).unwrap();
    std::fs::write(&main, b"pub machine altered() -> u32 { 0 }\n").unwrap();
    let mut permissions = std::fs::metadata(&main).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&main, permissions).unwrap();

    let error =
        compile_resolved_package_reviews(&closure, "windows_x64", &cache.join("compiler-build"))
            .expect_err("tampered resolver custody must not reach compilation");

    assert!(matches!(
        error,
        CompileResolvedPackageReviewsError::SourceCustody {
            source_package,
            phase: PackageSourceVerificationPhase::BeforeCompilation,
            error: SourceResolveError::SourceSnapshotContentMismatch { .. },
            ..
        } if source_package == root
    ));
    assert!(
        std::fs::read_dir(cache.join("compiler-build"))
            .expect("review build workspace remains readable")
            .next()
            .is_none(),
        "failed review must dispose its private build session"
    );
    let _ = std::fs::remove_dir_all(cache);
}
