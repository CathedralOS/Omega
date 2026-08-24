use omega_compiler::{
    CheckedPackageReviewProjection, PackageReviewCallableRole, PackageReviewContractExpression,
    PackageReviewContractFact, PackageReviewContractKind, PackageReviewDangerousAuthorityClass,
    PackageReviewNominalOwner, PackageReviewPropositionEvidence,
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
};
use omega_packages::{
    CompileResolvedPackageReviewsError, LocalSourceLimits, PackageSourceClosureLimits,
    PackageSourceVerificationPhase, SourceLineage, SourceResolveError, WorkspaceMemberPath,
    compile_resolved_package_reviews, resolve_workspace_package_closure,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const PACKAGES: &[&str] = &[
    "arithmetic-kernels",
    "generated-table",
    "file-journal",
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

    let expected_dangerous_authority_count = match package {
        "generated-table" | "file-journal" | "remote-journal" => 1,
        _ => 0,
    };
    assert_eq!(
        review.dangerous_authorities().len(),
        expected_dangerous_authority_count,
        "{package} dangerous authorities"
    );
    for authority in review.dangerous_authorities() {
        assert_eq!(
            authority.class(),
            PackageReviewDangerousAuthorityClass::Filesystem
        );
        let PackageReviewNominalOwner::ToolchainSource(source) = authority.service().owner() else {
            panic!("{package} filesystem authority must retain exact toolchain source")
        };
        assert_ne!(source.digest(), [0; 32]);
        assert_eq!(authority.service().path(), "FilesystemHost");
    }

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
            assert_eq!(build.realized_service_reach(), [service.clone()]);
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
            assert!(build.realized_synchronous_invocations().is_empty());
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
        }
        "capability-vault" => assert!(
            !callable.capability_flows().is_empty(),
            "capability-vault must issue exact capability-flow evidence"
        ),
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

        assert_eq!(reviews.reviews().len(), closure.graph().packages().len());
        for node in closure.graph().packages() {
            let custody = closure
                .custody(node.source().key())
                .expect("resolved graph package retains source custody");
            let issued = reviews
                .review(node.source().key())
                .expect("every resolved graph package receives compiler review material");
            assert_eq!(issued.resolution(), custody.resolution());
            assert_eq!(
                issued.projection().package(),
                node.source().key().identity()
            );
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
    let _ = std::fs::remove_dir_all(cache);
}
