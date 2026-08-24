use omega_compiler::{
    CheckedPackageReviewProjection, PackageReviewCallableRole, PackageReviewContractExpression,
    PackageReviewContractFact, PackageReviewContractKind, compile_to_checked_with_packages,
    project_checked_package_review,
};
use omega_packages::{
    LocalSourceLimits, PackageSourceClosureLimits, SourceLineage, WorkspaceMemberPath,
    package_compilation_inputs, resolve_workspace_package_closure,
};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PACKAGES: &[&str] = &[
    "arithmetic-kernels",
    "generated-table",
    "file-journal",
    "network-overreach",
    "axiom-ledger",
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

fn root_snapshot<'closure>(
    closure: &'closure omega_packages::ResolvedPackageSourceClosure,
) -> &'closure Path {
    let root = closure.graph().root();
    closure
        .custodies()
        .iter()
        .find(|custody| custody.key() == root)
        .expect("resolved closure must retain root custody")
        .snapshot_root()
}

fn assert_fixture_evidence(package: &str, review: &CheckedPackageReviewProjection) {
    let (trait_count, data_count, role) = match package {
        "file-journal" | "provider-switchboard" => (1, 1, PackageReviewCallableRole::Public),
        "capability-vault" => (2, 1, PackageReviewCallableRole::Public),
        "network-overreach" => (1, 0, PackageReviewCallableRole::Public),
        "axiom-ledger" => (0, 0, PackageReviewCallableRole::Boundary),
        _ => (0, 0, PackageReviewCallableRole::Public),
    };
    assert_eq!(
        review.public_traits().len(),
        trait_count,
        "{package} traits"
    );
    assert_eq!(review.public_data().len(), data_count, "{package} data");
    assert_eq!(review.callables().len(), 2, "{package} callable plus build");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.role() == role)
        .unwrap_or_else(|| panic!("{package} intended review callable"));

    match package {
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
            assert!(matches!(
                contract.fact(),
                PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary { left, right, .. })
                    if matches!(left.as_ref(), PackageReviewContractExpression::Result)
                        && matches!(right.as_ref(), PackageReviewContractExpression::Integer(value) if value == "0")
            ));
        }
        _ => {}
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
        let inputs = package_compilation_inputs(&closure)
            .unwrap_or_else(|error| panic!("{package} compiler handoff should close: {error:?}"));
        let checked = compile_to_checked_with_packages(
            &root_snapshot(&closure).join("main.omg"),
            Some("windows_x64"),
            inputs,
        )
        .unwrap_or_else(|diagnostics| {
            panic!("{package} package-aware compilation should check: {diagnostics:#?}")
        });
        let review = project_checked_package_review(&checked).unwrap_or_else(|diagnostics| {
            panic!("{package} review should close: {diagnostics:#?}")
        });

        assert_eq!(review.package(), closure.graph().root().identity());
        assert_fixture_evidence(package, &review);
        assert!(
            !review
                .canonical_review_bytes()
                .expect("fixture review must encode")
                .is_empty(),
            "{package} review encoding must be nonempty"
        );
        let _ = std::fs::remove_dir_all(cache);
    }
}
