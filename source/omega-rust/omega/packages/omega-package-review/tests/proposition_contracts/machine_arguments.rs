use crate::support::*;

#[test]
fn review_projects_exact_concrete_machine_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected: &str| {
        format!(
            r#"pub machine chosen(value: u64) -> u64 {{ value }}
pub machine alternate(value: u64) -> u64 {{ value }}
pub machine apply<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64
{{
    Selected(value)
}}
boundary machine trusted_zero() -> u64
ensures result == apply<{selected}>(0);
"#,
        )
    };
    package.write("main.omg", &source("chosen"));
    changed.write("main.omg", &source("alternate"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free static contract call should check");
        project_checked_package_review(&checked)
            .expect("an exact concrete machine argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_zero = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_zero")
        .expect("trusted boundary callable");
    let [contract] = trusted_zero.contracts() else {
        panic!("one trusted-zero contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-zero equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("static apply call")
    };
    let [PackageReviewContractStaticArgument::ConcreteMachine(selected)] =
        static_arguments.as_slice()
    else {
        panic!("one exact concrete machine argument")
    };
    assert_eq!(selected.path(), "chosen::entry");
    assert_eq!(
        selected.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("chosen-machine contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("alternate-machine contract encoding"),
        "changing an exact concrete static-machine selection must change package-review identity",
    );
}

#[test]
fn review_projects_contract_machine_binders_by_canonical_static_ordinal() {
    let Some(target) = host_target_name() else {
        return;
    };
    let compile = |binder: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub machine apply<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64
{{
    Selected(value)
}}
pub machine trusted_apply<machine {binder}>(value: u64) -> u64
where machine {binder}(value: u64) -> u64;
requires apply<{binder}>(value) == apply<{binder}>(value)
{{
    0
}}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic public contract fixture should check");
        project_checked_package_review(&checked)
            .expect("a forwarded machine binder has a canonical contract row")
    };
    let original = compile("Operation");
    let renamed = compile("RenamedOperation");
    let trusted_apply = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_apply")
        .expect("trusted generic public callable");
    let [contract] = trusted_apply.contracts() else {
        panic!("one trusted-apply contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-apply equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic apply call")
    };
    assert_eq!(
        static_arguments,
        &[PackageReviewContractStaticArgument::GenericMachineBinder(0)]
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original generic contract encoding"),
        renamed
            .canonical_review_bytes()
            .expect("renamed generic contract encoding"),
        "renaming a local machine binder must not alter package-review identity",
    );
}

#[test]
fn compiler_rejects_nested_machine_arguments_before_package_review() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary machine sample(value: u64) -> u64;
machine inspect<machine Operation>() -> u64
where machine Operation<machine Inner>(value: u64) -> u64
where machine Inner(value: u64) -> u64;
{
    0
}
machine identity<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64;
{
    value
}
boundary machine trusted_identity() -> u64
ensures result == inspect<identity<sample>>();
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("nested machine applications must fail before checked lowering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nested machine application; recursive specialization identity")
    }));
}
