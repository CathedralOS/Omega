use crate::support::*;

#[test]
fn empty_boundary_body_is_checked_callable_and_remains_directly_invocable() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait FilesystemHost { }

boundary machine adapter() reaches FilesystemHost { }

pub machine caller() reaches FilesystemHost {
    adapter();
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let candidate = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("an explicit empty boundary body remains executable");
    let accepted = candidate
        .candidate_service_binding(
            AcceptedSemanticBindingRole::FilesystemHostService,
            package_identity(),
            "FilesystemHost",
        )
        .expect("derive exact filesystem authority candidate");
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0)
            .with_accepted_semantic_bindings(vec![accepted])
            .expect("accepted binding names the exact fixture package"),
    )
    .expect("exact filesystem authority should settle");
    let review =
        project_checked_package_review(&checked).expect("empty boundary body review should close");
    let adapter = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "adapter")
        .expect("adapter review row");
    assert!(matches!(
        adapter.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    assert!(review.dangerous_authority_slack().iter().any(|slack| {
        slack.class() == PackageReviewDangerousAuthorityClass::Filesystem
            && slack.callable().path() == "adapter"
    }));
}

#[test]
fn package_review_rejects_impossible_supply_body_combinations() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write("main.omg", "pub machine api() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
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
    .expect("ordinary package should check");

    let mut missing_body = checked.clone();
    missing_body
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine")
        .body_is_present = false;
    let diagnostics = project_checked_package_review(&missing_body)
        .expect_err("checked supply without a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("classified as checked supply but has no retained body")
    }));

    let mut bodyful_accepted = checked;
    let api = bodyful_accepted
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine");
    api.supply_mode = psi_language_semantics::MachineSupplyMode::AdmissionClaim;
    let diagnostics = project_checked_package_review(&bodyful_accepted)
        .expect_err("bodyless supply with a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has bodyless supply but retains a body")
    }));
}
