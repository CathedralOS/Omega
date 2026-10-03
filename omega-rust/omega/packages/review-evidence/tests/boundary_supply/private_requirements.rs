use crate::support::*;
use compiler::CheckedCompileRequest;

fn checked_fixture() -> ReviewFixture {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::core::external_binding;
boundary trait ProcessControl {}
data NativeExit {}
boundary requirement NativeExit::finish(code: u32)
requires code == 42
reaches ProcessControl;
windows_x86_64 machine exit_binding() -> ForeignBinding<12, 11, 0> {
    ForeignBinding::DllImport {
        import: DllImport::PeByName { library: "kernel32.dll", export: "ExitProcess" },
    }
}
windows_x86_64 machine NativeExit::finish_leaf(code: u32)
satisfies NativeExit::finish via exit_binding();
"#,
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { Build::package(builder, \"review_fixture\"); }",
    );
    compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .unwrap_or_else(|diagnostics| panic!("private boundary fixture should check: {diagnostics:?}"))
}

#[test]
fn private_requirement_contract_and_external_supply_remain_reviewable() {
    let checked = checked_fixture();
    let review = project_checked_package_review(&checked)
        .expect("private requirements retain contracts without becoming public declarations");
    let [supply] = review.external_executable_supply() else {
        panic!("one external supply");
    };
    assert_eq!(supply.callable().path(), "NativeExit::finish_leaf");
    let requirement = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("NativeExit::finish"))
        .expect("the private boundary requirement is part of the trust review");
    assert_eq!(requirement.contracts().len(), 1);
    assert_eq!(supply.top_level_requirement(), Some(requirement.identity()));
    let policies = package_evidence::project_checked_callable_policy(
        &checked,
        checked.custody.execution().selected_target_profile.unwrap(),
        checked.custody.sources().package_identity().unwrap(),
    )
    .expect("private contract exposure must also survive policy capture");
    let policy_requirement = policies
        .callables()
        .iter()
        .find(|callable| callable.supply() == PackageReviewCallableSupply::TopLevelRequirement)
        .expect("private requirement policy");
    assert_eq!(policy_requirement.contracts(), requirement.contracts());
}

#[test]
fn changing_private_requirement_to_public_does_not_relabel_retained_contract_facts() {
    let mut checked = checked_fixture();
    let requirement = checked
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "NativeExit::finish")
        .expect("private requirement");
    requirement.is_public = true;
    let errors = project_checked_package_review(&checked)
        .expect_err("public review must reject the retained private selection exposure");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("wrong retained selection exposure")),
        "{errors:?}"
    );
}
