use crate::support::*;

#[test]
fn review_projects_nominal_static_machine_external_telescope() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Callback {
    machine call(value: u64) -> u64;
}
pub data CallbackSurface {}

pub boundary requirement CallbackSurface::bind<machine Schema>()
where machine Schema satisfies Callback::call;

pub machine bind_provider<machine Selected>()
where machine Selected satisfies Callback::call;
satisfies CallbackSurface::bind
via Binding::DllImport("omega-callback", "bind");
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("nominal static-machine top-level external supply should check");
    let review = project_checked_package_review(&checked)
        .expect("nominal static-machine external supply should project exactly");

    let [supply] = review.external_executable_supply() else {
        panic!("one nominal static-machine external executable-supply row")
    };
    let [provider_static] = supply.signature().static_parameters() else {
        panic!("one provider nominal machine parameter")
    };
    let [requirement_static] = supply
        .top_level_requirement_signature()
        .expect("top-level requirement signature")
        .static_parameters()
    else {
        panic!("one requirement nominal machine parameter")
    };
    let provider_nominal = provider_static
        .machine_contract()
        .expect("provider machine contract")
        .nominal()
        .expect("provider nominal contract");
    let requirement_nominal = requirement_static
        .machine_contract()
        .expect("requirement machine contract")
        .nominal()
        .expect("requirement nominal contract");
    assert_eq!(provider_nominal, requirement_nominal);
    assert_eq!(provider_nominal.0.path(), "Callback");
    assert!(provider_nominal.1.path().contains("Callback::call"));
}

#[test]
fn review_projects_recursive_static_machine_external_telescope() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CallbackSurface {}

pub boundary requirement CallbackSurface::register<machine Schema>()
where machine Schema<machine Inner>(value: u64) -> u64
where machine Inner(value: u64) -> u64;
;
;

pub machine register_provider<machine Operation>()
where machine Operation<machine Callback>(value: u64) -> u64
where machine Callback(value: u64) -> u64;
;
    satisfies CallbackSurface::register
    via Binding::DllImport("omega-callback", "register");
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("recursive static-machine top-level external supply should check");
    let review = project_checked_package_review(&checked)
        .expect("recursive static-machine external supply should project exactly");

    let [supply] = review.external_executable_supply() else {
        panic!("one static-machine external executable-supply row")
    };
    let [provider_static] = supply.signature().static_parameters() else {
        panic!("one provider static-machine parameter")
    };
    let provider_contract = provider_static
        .machine_contract()
        .expect("provider static-machine contract");
    let [requirement_static] = supply
        .top_level_requirement_signature()
        .expect("top-level requirement signature")
        .static_parameters()
    else {
        panic!("one requirement static-machine parameter")
    };
    let requirement_contract = requirement_static
        .machine_contract()
        .expect("requirement static-machine contract");
    assert_eq!(provider_contract, requirement_contract);
    let provider_signature = provider_contract
        .structural()
        .expect("outer structural machine contract");
    let [inner] = provider_signature.type_parameters() else {
        panic!("one nested static-machine parameter")
    };
    assert!(matches!(
        inner.kind(),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
            _
        ))
    ));

    let provider_telescope = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "register_provider")
        .expect("external callback provider")
        .type_parameters;
    let provider_static = &mut checked
        .typed
        .data_type_parameters
        .span_mut(provider_telescope)
        .expect("provider static telescope")[0];
    let original_kind = provider_static.kind.clone();
    provider_static.kind = psi_typed_trees::data::TypeParameterKind::Type;
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check static-machine kind substitution must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("static parameter 0 has a different kind")),
        "unexpected diagnostics for machine-kind substitution: {diagnostics:?}",
    );

    let provider_static = &mut checked
        .typed
        .data_type_parameters
        .span_mut(provider_telescope)
        .expect("provider static telescope")[0];
    provider_static.kind = original_kind;
    provider_static.bounds.multiplicity = psi_language_semantics::Multiplicity::Unrestricted;
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check type-property bounds on a machine parameter must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("machine parameter inapplicable type-property bounds")),
        "unexpected diagnostics for machine property bounds: {diagnostics:?}",
    );
}
