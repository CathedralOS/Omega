use crate::support::*;

#[test]
fn review_projects_alpha_renamed_const_telescope_in_top_level_external_supply() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data ConstSurface {}
pub data ConstProvider {}

pub boundary requirement ConstSurface::identity<const Count: u64>(
    value: [u8; Count]
) -> [u8; Count];
pub machine ConstProvider::identity<const Length: u64>(
    value: [u8; Length]
) -> [u8; Length]
    satisfies ConstSurface::identity
    via Binding::DllImport("omega-const", "identity");
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
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("alpha-renamed const top-level external supply should check");
    let review = project_checked_package_review(&checked)
        .expect("const top-level external supply should project exactly");

    let [supply] = review.external_executable_supply() else {
        panic!("one const external executable-supply row")
    };
    let [provider_static] = supply.signature().static_parameters() else {
        panic!("one exact provider const parameter")
    };
    let provider_const_type = provider_static
        .const_type_identity()
        .expect("provider const parameter carrier");
    let requirement_signature = supply
        .top_level_requirement_signature()
        .expect("exact top-level requirement signature");
    let [requirement_static] = requirement_signature.static_parameters() else {
        panic!("one exact requirement const parameter")
    };
    assert_eq!(
        provider_const_type,
        requirement_static
            .const_type_identity()
            .expect("requirement const parameter carrier")
    );
    assert_eq!(
        supply.signature().parameters()[0].type_identity(),
        requirement_signature.parameters()[0].type_identity(),
        "positional const binders alpha-normalize independently of authored names",
    );
    for authored_binder in ["Count", "Length"] {
        assert!(
            !supply.signature().parameters()[0]
                .type_identity()
                .canonical()
                .contains(authored_binder)
        );
    }

    let (provider_telescope, provider_value_type) = {
        let provider = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "ConstProvider::identity")
            .expect("const external realization");
        let entry = checked
            .typed
            .machine_states(provider)
            .first()
            .expect("const external entry");
        (
            provider.type_parameters,
            checked.typed.state_parameters(entry)[0].type_reference,
        )
    };
    let byte_type = match checked
        .typed
        .type_reference_table
        .type_reference(provider_value_type)
    {
        psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
        unexpected => panic!("expected const-sized array, got {unexpected:?}"),
    };
    let provider_static = &mut checked
        .typed
        .data_type_parameters
        .span_mut(provider_telescope)
        .expect("provider const telescope")[0];
    let psi_typed_trees::data::TypeParameterKind::Const { type_reference } =
        &mut provider_static.kind
    else {
        panic!("provider const parameter")
    };
    let original_const_type = *type_reference;
    *type_reference = byte_type;
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check const carrier substitution must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("const static parameter 0 has a different type")
        }),
        "unexpected diagnostics for const carrier substitution: {diagnostics:?}",
    );

    let provider_static = &mut checked
        .typed
        .data_type_parameters
        .span_mut(provider_telescope)
        .expect("provider const telescope")[0];
    let psi_typed_trees::data::TypeParameterKind::Const { type_reference } =
        &mut provider_static.kind
    else {
        panic!("provider const parameter")
    };
    *type_reference = original_const_type;
    provider_static.bounds.multiplicity = psi_language_semantics::Multiplicity::Unrestricted;
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check type-property bounds on a const parameter must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("const parameter inapplicable type-property bounds")),
        "unexpected diagnostics for const property bounds: {diagnostics:?}",
    );
}

#[test]
fn review_projects_bounded_type_telescope_in_top_level_external_supply() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data GenericSurface {}
pub data GenericProvider {}

pub boundary requirement GenericSurface::identity<Element [copy]>(value: Element) -> Element;
pub machine GenericProvider::identity<Value>(value: Value) -> Value
    satisfies GenericSurface::identity
    via Binding::DllImport("omega-generic", "identity");
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
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("bounded generic top-level external supply should check");
    let review = project_checked_package_review(&checked)
        .expect("bounded generic top-level external supply should project exactly");

    let [supply] = review.external_executable_supply() else {
        panic!("one bounded external executable-supply row")
    };
    let [static_parameter] = supply.signature().static_parameters() else {
        panic!("one exact external static parameter")
    };
    let properties = static_parameter
        .type_properties()
        .expect("ordinary type static parameter");
    assert_eq!(
        properties.multiplicity(),
        psi_language_semantics::Multiplicity::Affine
    );
    assert_eq!(properties.carry(), None);
    let [requirement_parameter] = supply
        .top_level_requirement_signature()
        .expect("exact top-level requirement signature")
        .static_parameters()
    else {
        panic!("one exact requirement static parameter")
    };
    assert_eq!(
        requirement_parameter
            .type_properties()
            .expect("ordinary requirement type parameter")
            .multiplicity(),
        psi_language_semantics::Multiplicity::Unrestricted
    );
}

#[test]
fn review_projects_unselected_type_and_lifetime_generic_top_level_external_supply() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data GenericSurface {}
pub data GenericProvider {}
pub data LifetimeSurface {}
pub data LifetimeProvider {}

pub boundary requirement GenericSurface::identity<Element>(value: Element) -> Element;
pub boundary requirement LifetimeSurface::observe<'input>(value: &'input u32);

pub machine GenericProvider::identity<Value>(value: Value) -> Value
    satisfies GenericSurface::identity
    via Binding::DllImport("omega-generic", "identity");
pub machine LifetimeProvider::observe<'borrow>(value: &'borrow u32)
    satisfies LifetimeSurface::observe
    via Binding::DllImport("omega-generic", "observe");
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
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("generic top-level external supply should check");
    let review = project_checked_package_review(&checked)
        .expect("generic top-level external supply should project without installation");

    assert_eq!(review.external_executable_supply().len(), 2);
    for (callable, symbol, lifetime_count, type_count) in [
        ("GenericProvider::identity", "identity", 0, 1),
        ("LifetimeProvider::observe", "observe", 1, 0),
    ] {
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path().starts_with(callable))
            .unwrap_or_else(|| panic!("missing external supply for {callable}"));
        let requirement = supply
            .top_level_requirement()
            .expect("top-level requirement classification");
        assert_eq!(
            supply.signature().lifetime_parameter_count(),
            lifetime_count
        );
        assert_eq!(supply.signature().static_parameters().len(), type_count);
        for authored_binder in ["Element", "Value", "input", "borrow"] {
            assert!(
                !supply.callable().path().contains(authored_binder)
                    && !requirement.path().contains(authored_binder)
                    && !supply
                        .signature()
                        .parameters()
                        .iter()
                        .any(|parameter| parameter
                            .type_identity()
                            .canonical()
                            .contains(authored_binder))
                    && !supply
                        .signature()
                        .return_type()
                        .canonical()
                        .contains(authored_binder),
                "external-supply identity must alpha-normalize `{authored_binder}`",
            );
        }
        assert_eq!(
            supply.binding(),
            &PackageReviewExternalBinding::Import {
                library: "omega-generic".to_owned(),
                symbol: symbol.to_owned(),
            }
        );
        assert!(
            review.selected_providers().iter().all(|provider| provider
                .row_declarations()
                .iter()
                .all(|row| row.realization() != supply.callable())),
            "generic disclosure must not imply provider selection or installation",
        );
    }
}
