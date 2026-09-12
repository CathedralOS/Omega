use super::*;

#[test]
fn selected_operator_crash_evidence_projects_through_review_and_callable_policy() {
    for (operator_contract, caller_contract, cause) in [
        (
            "crashes Trap",
            "crashes Trap",
            PackageReviewCrashCause::Trap,
        ),
        (
            "crashes Abort",
            "crashes Abort",
            PackageReviewCrashCause::Abort,
        ),
    ] {
        let fixture = Fixture::local(&format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool {operator_contract};
             pub data ComparisonProvider {{}}
             machine ComparisonProvider::equal_impl(left: i32, right: i32) -> bool
             satisfies Comparison::equal
             {{ transition {{ _ -> true }} }}
             pub machine compare(left: i32, right: i32) -> bool {caller_contract} {{ left == right }}"
        ));
        assert!(
            fixture
                .checked
                .facts
                .operators
                .has_crash_qualified_uses(&fixture.checked.typed)
        );
        let review = package_evidence::project_checked_package_review(&fixture.checked)
            .expect("operator crash review projection");
        let compare = review
            .callables()
            .iter()
            .find(|callable| callable.identity().path().contains("compare"))
            .expect("compare callable review row");
        let [site] = compare.checked_crash().checked_operators() else {
            panic!("one selected operator crash site: {compare:#?}")
        };
        assert!(site.selected_operator().path().contains("equal"));
        let [published] = site.published() else {
            panic!("one published operator route: {site:#?}")
        };
        assert_eq!(published.cause(), cause);
        assert_eq!(
            published.alternative_guards(),
            &[PackageReviewCrashRouteGuard::Truth]
        );
        assert_eq!(site.surviving(), site.published());
        let policy =
            project_checked_callable_policy(&fixture.checked, fixture.target, package_identity())
                .expect("operator crash callable policy");
        let crash = callable(&policy, "compare").checked_crash();
        assert_eq!(
            crash.interface(),
            PackageReviewCrashInterface::PublishedCeiling
        );
        let [route] = crash.published() else {
            panic!("one published caller route: {crash:#?}")
        };
        assert_eq!(route.cause(), cause);
        assert_eq!(
            route.alternative_guards(),
            &[PackagePolicyCrashGuard::Truth]
        );
        assert_eq!(crash.inferred(), &PackagePolicyInferredCrash::Unknown);
    }
}

#[test]
fn discharged_selected_operator_crash_retains_its_site_without_an_inferred_cause() {
    let fixture = Fixture::local(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool crashes Trap false;
         pub data ComparisonProvider {}
         machine ComparisonProvider::equal_impl(left: i32, right: i32) -> bool
         satisfies Comparison::equal
         { transition { _ -> true } }
         pub machine compare(left: i32, right: i32) -> bool { left == right }",
    );
    assert!(
        fixture
            .checked
            .facts
            .operators
            .has_crash_qualified_uses(&fixture.checked.typed)
    );
    let review = package_evidence::project_checked_package_review(&fixture.checked)
        .expect("discharged operator crash review projection");
    let compare = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("compare"))
        .expect("compare callable review row");
    let [site] = compare.checked_crash().checked_operators() else {
        panic!("a proved discharge still retains the site row: {compare:#?}")
    };
    assert_eq!(site.published().len(), 1);
    assert!(site.surviving().is_empty());
    let policy =
        project_checked_callable_policy(&fixture.checked, fixture.target, package_identity())
            .expect("discharged operator crash callable policy");
    let crash = callable(&policy, "compare").checked_crash();
    assert_eq!(
        crash.interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    assert!(crash.published().is_empty());
    assert_eq!(
        crash.inferred(),
        &PackagePolicyInferredCrash::Complete { causes: Vec::new() }
    );
}

#[test]
fn selected_build_inferred_crash_causes_distinguish_quiet_trap_and_abort() {
    let quiet = project(&Fixture::local(""));
    let quiet_build = callable(&quiet, "build");
    assert_eq!(quiet_build.role(), PackagePolicyCallableRole::Build);
    assert_eq!(
        quiet_build.checked_crash().inferred(),
        &PackagePolicyInferredCrash::Complete { causes: Vec::new() }
    );
    for (name, cause) in [
        ("Trap", PackageReviewCrashCause::Trap),
        ("Abort", PackageReviewCrashCause::Abort),
    ] {
        let build = format!(
            "machine build(builder: &mut Build) {{ builder.package(\"review-fixture\"); crash {name}; }}\n"
        );
        let policy = project(&Fixture::with_build("", &build));
        let row = callable(&policy, "build");
        assert_eq!(row.role(), PackagePolicyCallableRole::Build);
        assert!(row.checked_crash().published().is_empty());
        assert_eq!(
            row.checked_crash().inferred(),
            &PackagePolicyInferredCrash::Complete {
                causes: vec![cause]
            }
        );
        assert_ne!(
            quiet.canonical_bytes().unwrap(),
            policy.canonical_bytes().unwrap()
        );
    }
}

#[test]
fn selected_build_inferred_crash_follows_private_helpers_without_their_names() {
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); forward(); }
machine forward() { leaf(); }
machine leaf() { crash Trap; }
"#;
    let original = project(&Fixture::with_build("", build));
    let renamed = project(&Fixture::with_build(
        "",
        &build.replace("forward", "relay").replace("leaf", "finish"),
    ));
    assert_eq!(
        callable(&original, "build").checked_crash().inferred(),
        &PackagePolicyInferredCrash::Complete {
            causes: vec![PackageReviewCrashCause::Trap]
        }
    );
    assert_eq!(original, renamed);
    assert_eq!(
        original.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );
}

#[test]
fn published_crash_guards_remain_structural_and_distinguish_absent_from_false() {
    let source = "pub machine run(flag: bool) crashes Trap flag; {}\n";
    let original = project(&Fixture::local(source));
    let changed = project(&Fixture::local(&source.replace("Trap flag", "Trap !flag")));
    let explicit_false = project(&Fixture::local(&source.replace("Trap flag", "Trap false")));
    let absent = project(&Fixture::local("pub machine run(flag: bool) {}\n"));
    let crash = callable(&original, "run").checked_crash();
    assert_eq!(
        crash.interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    assert_eq!(crash.published().len(), 1);
    assert_eq!(crash.published()[0].cause(), PackageReviewCrashCause::Trap);
    assert_ne!(crash, callable(&changed, "run").checked_crash());
    assert!(
        !callable(&explicit_false, "run")
            .checked_crash()
            .published()
            .is_empty()
    );
    assert!(
        callable(&absent, "run")
            .checked_crash()
            .published()
            .is_empty()
    );
    assert_ne!(
        explicit_false.canonical_bytes().unwrap(),
        absent.canonical_bytes().unwrap()
    );
}

#[test]
fn same_spelled_foreign_crash_predicate_retains_exact_package_owner() {
    let root =
        "use dependency::helpers;\npub machine run(flag: bool) crashes Trap permitted(flag); {}\n";
    let dependency = "pub machine permitted(flag: bool) -> bool terminates; { flag }\n";
    let first = Fixture::foreign(
        root,
        dependency,
        PackageKeyIdentity::from_digest([42; 32]).unwrap(),
    );
    let second = Fixture::foreign(
        root,
        dependency,
        PackageKeyIdentity::from_digest([43; 32]).unwrap(),
    );
    let first = project(&first);
    let second = project(&second);
    assert_eq!(
        callable(&first, "run").identity(),
        callable(&second, "run").identity()
    );
    assert_eq!(
        callable(&first, "run").parameters(),
        callable(&second, "run").parameters()
    );
    assert_ne!(
        callable(&first, "run").checked_crash(),
        callable(&second, "run").checked_crash()
    );
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}

#[test]
fn nested_static_crash_guard_retains_foreign_callable_owner() {
    let root = r#"use dependency::helpers;
pub machine accepts<machine Work>()
where machine Work(flag: bool) crashes Trap permitted(flag);
{}
"#;
    let helper = "pub machine permitted(flag: bool) -> bool terminates; { flag }\n";
    let first_owner = PackageKeyIdentity::from_digest([42; 32]).unwrap();
    let second_owner = PackageKeyIdentity::from_digest([43; 32]).unwrap();
    let first = project(&Fixture::foreign(root, helper, first_owner));
    let second = project(&Fixture::foreign(root, helper, second_owner));
    let signature = |policy: &PackagePolicyCallables| {
        let PackagePolicyTypeParameterKind::Machine(contract) =
            callable(policy, "accepts").type_parameters()[0].kind()
        else {
            panic!("one static machine parameter")
        };
        contract
            .structural()
            .expect("structural static contract")
            .clone()
    };
    let first_signature = signature(&first);
    let second_signature = signature(&second);
    assert_eq!(first_signature.parameters(), second_signature.parameters());
    for (signature, owner) in [
        (&first_signature, first_owner),
        (&second_signature, second_owner),
    ] {
        let [route] = signature.published_crash() else {
            panic!("one nested static crash route")
        };
        let [
            PackagePolicyCrashGuard::Expression(PackageReviewContractExpression::Call {
                target,
                ..
            }),
        ] = route.alternative_guards()
        else {
            panic!("typed guard call retains its exact target, not opaque predicate bytes")
        };
        let target = target.nominal().expect("exact helper declaration identity");
        assert_eq!(target.owner(), PackageReviewNominalOwner::Package(owner));
    }
    assert_ne!(
        first_signature.published_crash(),
        second_signature.published_crash()
    );
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}
