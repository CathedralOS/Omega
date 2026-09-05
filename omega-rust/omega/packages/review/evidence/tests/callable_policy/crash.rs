use super::*;

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
