use super::*;

pub(super) const PACKAGES: &[&str] = &[
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

pub(super) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join("tests/fixtures/packages").is_dir())
        .expect("omega-package-manager should live beneath the Omega workspace")
        .to_path_buf()
}

pub(super) fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-evidence-{name}-{}-{stamp}",
        std::process::id()
    ))
}

pub(super) fn resolve_workspace_package_closure(
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

pub(super) fn assert_fixture_evidence(package: &str, review: &CheckedPackageReviewProjection) {
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
