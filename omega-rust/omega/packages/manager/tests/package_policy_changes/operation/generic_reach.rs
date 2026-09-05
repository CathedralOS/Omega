use super::*;
use package_evidence::record::{
    PackagePolicyBaseline, PackagePolicyCallable, PackagePolicyCallableRole,
    PackagePolicyTypeParameterKind, PackageReviewNominalOwner,
};

// The public ceiling stays fixed. Work has no selected implementation: its
// invocation must retain the authored contract's conservative service reach.
const GENERIC: &str = r#"pub boundary trait First {}
pub boundary trait Second {}
pub machine invoke<machine Work>()
where machine Work() reaches First;
reaches First + Second
{
    Work();
}
"#;

fn public_callable(policy: &PackagePolicyBaseline) -> &PackagePolicyCallable {
    let public = policy
        .callables()
        .callables()
        .iter()
        .filter(|callable| callable.role() == PackagePolicyCallableRole::Public)
        .collect::<Vec<_>>();
    let [invoke] = public.as_slice() else {
        panic!("one public generic callable: {public:?}");
    };
    assert!(invoke.identity().path().contains("invoke"));
    invoke
}

fn assert_contract_reach(policy: &PackagePolicyBaseline, expected: &[&str]) {
    let invoke = public_callable(policy);
    assert_eq!(
        invoke.identity().owner(),
        PackageReviewNominalOwner::Package(policy.package())
    );
    let [parameter] = invoke.type_parameters() else {
        panic!("one retained static machine parameter");
    };
    let PackagePolicyTypeParameterKind::Machine(contract) = parameter.kind() else {
        panic!("Work retains its machine contract");
    };
    let signature = contract.structural().expect("structural Work contract");
    assert!(!signature.service_reach_is_installation_bound());
    assert_eq!(
        signature
            .service_reach()
            .iter()
            .map(|service| service.path())
            .collect::<Vec<_>>(),
        expected
    );
    assert!(signature.service_reach().iter().all(|service| {
        service.owner() == PackageReviewNominalOwner::Package(policy.package())
    }));
    assert_eq!(
        invoke.checked_service_reach().realized().unwrap(),
        signature.service_reach(),
        "the actual Work() invocation must contribute its nonempty contract reach"
    );
    assert_eq!(
        invoke.checked_service_reach().concrete().unwrap(),
        signature.service_reach()
    );
    assert!(invoke.unresolved_installation_reaches().is_empty());
    assert!(policy.boundary_applications().realizations().is_empty());
}

#[test]
fn invoked_generic_contract_reach_survives_policy_and_changed_review() {
    let tree = Tree::new();
    source(&tree, GENERIC, "");
    let initial = review(&tree, "generic-initial", None);
    let root = initial.source_closure().graph().root();
    let original = initial.reviews().review(root).unwrap().policy();
    assert_contract_reach(original, &["First"]);
    let accepted = propose(&initial);
    assert_round_trip(&initial, accepted.clone());

    let expanded = GENERIC.replace(
        "where machine Work() reaches First;",
        "where machine Work() reaches First + Second;",
    );
    fs::write(tree.path("sources/root/main.omg"), expanded).unwrap();
    let updated = review(&tree, "generic-expanded", Some(&accepted));
    let candidate = updated.reviews().review(root).unwrap().policy();
    assert_contract_reach(candidate, &["First", "Second"]);
    assert_eq!(original.public_api(), candidate.public_api());
    assert_eq!(
        public_callable(original).declared_service_reach(),
        public_callable(candidate).declared_service_reach()
    );
    assert_ne!(original, candidate);
    assert_ne!(
        original.canonical_bytes().unwrap(),
        candidate.canonical_bytes().unwrap()
    );
    let changes = updated
        .changes()
        .packages()
        .iter()
        .find(|package| package.key() == root)
        .unwrap();
    let [row] = changes.rows() else {
        panic!(
            "only the generic callable contract changes: {:?}",
            changes.rows()
        );
    };
    assert_eq!(row.kind(), PackagePolicyRowKind::Callable);
    assert_eq!(row.change(), PackagePolicyChangeKind::Changed);
    assert!(row.requires_decision());
    let baseline = row.baseline().unwrap();
    let proposed = row.candidate().unwrap();
    assert_ne!(baseline.canonical_bytes(), proposed.canonical_bytes());
    let report = render_package_policy_review(updated.changes(), MAXIMUM_DOCUMENT_BYTES).unwrap();
    assert!(report.contains("change callable changed\n"));
    for (prefix, retained) in [("-", baseline), ("+", proposed)] {
        assert!(retained.canonical_text().contains("invoke"));
        assert!(retained.canonical_text().contains("First"));
        for line in retained.canonical_text().lines() {
            assert!(report.contains(&format!("{prefix} {line}\n")));
        }
    }
    assert!(matches!(
        recover_package_policy_review(updated.changes(), &report, MAXIMUM_DOCUMENT_BYTES),
        Err(PackagePolicyReviewError::UnresolvedDecision(_))
    ));
    assert_round_trip(&updated, propose(&updated));
    assert!(!tree.path("sources/root/omega.lock").exists());
}

// This is the checked-source fixture from package-evidence's unresolved
// installation-reach regression, exercised through the package operation.
const UNRESOLVED: &str = r#"pub boundary trait MachineControl {}
pub boundary trait PortIo {}
pub boundary trait InterruptCompletion {
    machine complete() -> u64
    reaches <= MachineControl + PortIo;
}
pub machine invoke<machine Completion>() -> u64
where machine Completion satisfies InterruptCompletion::complete;
reaches MachineControl + PortIo
{
    Completion()
}
"#;

#[test]
fn unresolved_installation_generic_rejects_with_exact_package_at_projection() {
    for transitive in [false, true] {
        let tree = Tree::new();
        if transitive {
            super::transitive::source_chain(&tree, "generic-leaf", UNRESOLVED);
        } else {
            source(&tree, UNRESOLVED, "");
        }
        let build = fs::read(tree.path("sources/root/build.omg")).unwrap();
        let closure = resolve(&tree, "generic-unresolved");
        let owner = if transitive {
            assert_eq!(closure.graph().packages().len(), 3);
            closure
                .custodies()
                .iter()
                .find(|custody| custody.key().name().as_str() == "generic-leaf")
                .unwrap()
                .key()
                .clone()
        } else {
            closure.graph().root().clone()
        };
        let error = review_package_change(closure, TARGET, None, &tree.path("build")).unwrap_err();
        let PackageChangeError::Compilation(CompileResolvedPackageReviewsError::Projection {
            package,
            diagnostics,
        }) = error
        else {
            panic!("expected checked projection failure, transitive={transitive}: {error:?}");
        };
        assert_eq!(package, owner);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(
                    "ordinary public callable `invoke` cannot export unresolved installation-reach",
                ) && diagnostic.message.contains("InterruptCompletion::complete")
            }),
            "unexpected diagnostics, transitive={transitive}: {diagnostics:#?}"
        );
        assert_eq!(
            fs::read(tree.path("sources/root/build.omg")).unwrap(),
            build
        );
        assert!(!tree.path("sources/root/omega.lock").exists());
    }
}
