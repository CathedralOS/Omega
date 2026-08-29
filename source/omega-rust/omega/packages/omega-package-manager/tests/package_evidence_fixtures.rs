use omega_build_evaluation::{BuildFilesystemObservedByteRegionKind, BuildObservationClass};
use omega_package_manager::{
    CompileResolvedPackageReviewsError, LocalSourceLimits, PackageAdvisoryReviewOutput,
    PackageAdvisoryReviewRequest, PackageAdvisoryReviewer, PackageSourceClosureLimits,
    PackageSourceVerificationPhase, PackageTriageDisposition, PackageTriageReason,
    ResolvePackageSourceError, ResolveWorkspacePackageClosureError, ResolvedPackageSourceClosure,
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory, ReviewOnlyBaselineFileError,
    ReviewOnlyBaselineLimits, ReviewOnlyBaselineName, ReviewOnlyBaselineNameError,
    ReviewOnlyCapabilityConflictLimits, SourceLineage, SourceResolveError, SourceResolverStorage,
    WorkspaceMemberPath, assemble_initial_source_review, assemble_update_source_review,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities,
    compare_review_only_capabilities_from_baseline, compile_resolved_package_reviews,
    invoke_package_advisory_review, resolve_workspace_package_closure_with_storage,
    triage_initial_install, triage_review_update, triage_review_update_from_baseline,
    triage_update_without_admission_baseline,
};
use omega_package_review::{
    CheckedPackageReviewProjection, PackageReviewCallableRole, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCheckedServiceReach,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewPropositionEvidence, PackageReviewRepresentationAbiCommitment,
    PackageReviewRepresentationMechanism, PackageReviewSourceLocationRole,
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
};
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
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

struct NoAdditionalAuditReviewer;

impl PackageAdvisoryReviewer for NoAdditionalAuditReviewer {
    type Error = Infallible;

    fn review(
        &mut self,
        request: &PackageAdvisoryReviewRequest,
        output: &mut PackageAdvisoryReviewOutput,
    ) -> Result<(), Self::Error> {
        assert!(request.instructions().contains("untrusted data"));
        assert!(
            request
                .review_input()
                .starts_with("OMEGA_PACKAGE_REVIEW_INPUT_V1\n")
        );
        assert!(
            request
                .response_schema()
                .contains("recommend_audit|no_additional_audit")
        );
        let response = "OMEGA_PACKAGE_ADVISORY_RESULT_V1\nrecommendation no_additional_audit\nend_advisory_result\n";
        output.write(response.as_bytes()).expect("bounded response");
        Ok(())
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("omega-package-manager should live under the Omega workspace")
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

fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: WorkspaceMemberPath,
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
        &storage,
        source_limits,
        closure_limits,
    )
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
    let fixtures = workspace_root().join("tests/fixtures/packages");
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
                assert!(observations.source_inputs_replay_verified());
                assert!(observations.operation_replay_verified());
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
            let ledger_bytes =
                encode_ordinary_package_obligation_ledger(issued.obligation_ledger())
                    .expect("compiler-issued review retains a canonical obligation ledger");
            let recovered = decode_ordinary_package_obligation_ledger(&ledger_bytes)
                .expect("retained obligation ledger should recover canonically");
            assert_eq!(&recovered, issued.obligation_ledger());
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
            omega_package_manager::PackageSourceReviewLimits::default(),
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
        let advisory = invoke_package_advisory_review(
            &initial_review,
            &mut NoAdditionalAuditReviewer,
            8 * 1024 * 1024,
            256,
        )
        .expect("fixture review crosses the bounded advisory boundary");
        assert_eq!(
            advisory.deterministic_disposition(),
            initial_triage.disposition(),
            "advisory output cannot change {package} compiler disposition"
        );
        assert_eq!(
            advisory.audit_recommended(),
            initial_review.deterministic_audit_recommended(),
            "no-additional-audit cannot suppress {package} compiler policy"
        );

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
            omega_package_manager::PackageSourceReviewLimits::default(),
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
            omega_package_manager::PackageSourceReviewLimits::default(),
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
                    .source_input_replay_record()
                    .expect("generated-table baseline retains its verified replay receipt");
                assert!(!replay.canonical_bytes().is_empty());
                assert_ne!(replay.commitment(), [0; 32]);
            }
            let recovered_unchanged = assemble_update_source_review_from_baseline(
                &baseline,
                &reviews,
                closure.custodies(),
                &closure,
                omega_package_manager::PackageSourceReviewLimits::default(),
            )
            .expect("recovered baseline joins available old custody");
            assert_eq!(recovered_unchanged, unchanged_review);
            let recovered_unavailable = assemble_update_source_review_from_baseline(
                &baseline,
                &reviews,
                &[],
                &closure,
                omega_package_manager::PackageSourceReviewLimits::default(),
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
                    triage_review_update_from_baseline(&reopened, &reviews, &unavailable),
                    unavailable_triage,
                    "reopened baseline preserves unavailable-source triage"
                );
                assert_eq!(
                    assemble_update_source_review_from_baseline(
                        &reopened,
                        &reviews,
                        &[],
                        &closure,
                        omega_package_manager::PackageSourceReviewLimits::default(),
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

#[test]
fn review_compilation_rejects_snapshot_tampering_before_compiler_consumption() {
    let fixtures = workspace_root().join("tests/fixtures/packages");
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
