use crate::support::*;

#[test]
fn unsupported_external_boundary_operator_neighbors_remain_fail_closed() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let cases = [
        (
            "private-operator",
            r#"data F32 {}
boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
"#,
            "realizes non-public operator",
        ),
        (
            "aliased",
            r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum as Selected
    via Binding::CompilerIntrinsic;
"#,
            "through an alias not yet represented",
        ),
        (
            "generic-machine",
            r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum<T>(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
"#,
            "generic or lifetime-parameterized boundary operator",
        ),
    ];

    for (label, source, expected) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .unwrap_or_else(|diagnostics| panic!("{label} fixture should check: {diagnostics:?}"));
        let diagnostics = project_checked_package_review(&checked)
            .expect_err("unsupported external operator realization must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{label}: {diagnostics:?}"
        );
    }
}

#[test]
fn external_boundary_operator_supply_rejects_post_check_requirement_drift() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
pub boundary operator F32::maximum(left: f32, right: f32) -> f32;

pub data FloatProvider {}
pub machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
pub machine FloatProvider::maximum(left: f32, right: f32) -> f32
    satisfies F32::maximum
    via Binding::CompilerIntrinsic;
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
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("external boundary-operator drift fixture should check");
    let wrong_requirement = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked.typed.operator_path_members(operator.name)[1].as_str() == "maximum"
        })
        .expect("maximum boundary operator")
        .symbol;
    let satisfies = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "FloatProvider::minimum")
        .expect("minimum external realization")
        .satisfies;
    checked
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .requirement_symbol = wrong_requirement;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check external requirement drift must fail closed");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "checked operator-realization contracts do not equal compiler rederivation",
            )
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn external_binding_changes_only_the_supply_row_for_a_stable_callable() {
    let Some(target) = host_target_name() else {
        return;
    };
    let project = |number: i64| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub boundary trait ExternalSurface {{
    machine invoke() reaches ExternalSurface;
}}
pub machine invoke_leaf()
    satisfies ExternalSurface::invoke
    via Binding::Syscall({number});
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
        .expect("external syscall fixture should check");
        project_checked_package_review(&checked)
            .expect("external syscall package review should close")
            .canonical_rows()
            .expect("external syscall canonical rows")
    };

    let old = project(60);
    let new = project(61);
    let old_callable = old
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("old callable row");
    let new_callable = new
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("new callable row");
    assert_eq!(old_callable.key_bytes(), new_callable.key_bytes());
    assert_eq!(
        old_callable.canonical_bytes(),
        new_callable.canonical_bytes()
    );

    let old_supply = old
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("old external-supply row");
    let new_supply = new
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("new external-supply row");
    assert_eq!(old_supply.key_bytes(), new_supply.key_bytes());
    assert_ne!(old_supply.canonical_bytes(), new_supply.canonical_bytes());
}

#[test]
fn external_executable_supply_projection_rejects_inconsistent_checked_state() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait ExternalSurface {
    machine invoke() reaches ExternalSurface;
}
pub machine invoke_leaf()
    satisfies ExternalSurface::invoke
    via Binding::Syscall(60);
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
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("external tamper fixture should check");

    fn replace_external_binding(
        checked: &mut CheckedCompilation,
        identity: psi_language_semantics::ExternalBindingIdentity,
    ) {
        let mechanism = identity.mechanism();
        let binding = checked.typed.external_bindings.intern(identity);
        let leaf = checked
            .typed
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.name.as_str() == "invoke_leaf")
            .expect("external leaf");
        let satisfies = leaf.satisfies;
        leaf.supply_mode =
            psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism };
        checked
            .typed
            .machine_trait_conformances
            .span_mut_or_empty(satisfies)[0]
            .external_binding = Some(binding);
    }

    let mut mechanism_mismatch = checked.clone();
    let leaf = mechanism_mismatch
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf");
    let psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, .. } =
        leaf.supply_mode
    else {
        panic!("external leaf supply")
    };
    leaf.supply_mode = psi_language_semantics::MachineSupplyMode::ExternalRealization {
        binding,
        mechanism: psi_language_semantics::ExternalBindingMechanism::Import,
    };
    let diagnostics = project_checked_package_review(&mechanism_mismatch)
        .expect_err("mechanism mismatch must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("supply mechanism inconsistent with its exact binding identity")
    }));

    let mut span_without_conformance_binding = checked.clone();
    let satisfies = span_without_conformance_binding
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    span_without_conformance_binding
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = None;
    let diagnostics = project_checked_package_review(&span_without_conformance_binding)
        .expect_err("authored custody without a binding must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retains authored `via` custody without an external binding")
    }));

    let mut binding_without_source_span = checked.clone();
    let satisfies = binding_without_source_span
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    binding_without_source_span
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding_source_span = None;
    let diagnostics = project_checked_package_review(&binding_without_source_span)
        .expect_err("external binding without authored custody must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact authored `via` custody")
    }));

    let mut invalid_source_span = checked.clone();
    let satisfies = invalid_source_span
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    invalid_source_span
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding_source_span = Some(Default::default());
    let diagnostics = project_checked_package_review(&invalid_source_span)
        .expect_err("source-free external binding custody must fail closed");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source span"))
    );

    let mut missing_binding_identity = checked.clone();
    let invalid_binding = psi_language_semantics::ExternalBindingId(u32::MAX);
    let leaf = missing_binding_identity
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf");
    let satisfies = leaf.satisfies;
    leaf.supply_mode = psi_language_semantics::MachineSupplyMode::ExternalRealization {
        binding: invalid_binding,
        mechanism: psi_language_semantics::ExternalBindingMechanism::Syscall,
    };
    missing_binding_identity
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(invalid_binding);
    let diagnostics = project_checked_package_review(&missing_binding_identity)
        .expect_err("missing binding-table identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact binding-table identity")
    }));

    let mut bodyful_external = checked.clone();
    bodyful_external
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .body_is_present = true;
    let diagnostics = project_checked_package_review(&bodyful_external)
        .expect_err("bodyful external supply must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retains an implementation body")
    }));

    let mut missing_conformance = checked.clone();
    missing_conformance
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies = Default::default();
    let diagnostics = project_checked_package_review(&missing_conformance)
        .expect_err("external supply without a conformance must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has 0 conformance applications; expected exactly one")
    }));

    let mut duplicate_conformance = checked.clone();
    let leaf_index = duplicate_conformance
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf index");
    let duplicate = duplicate_conformance
        .typed
        .machine_trait_conformances(&duplicate_conformance.typed.machines()[leaf_index])[0]
        .clone();
    let machine_roots = duplicate_conformance.typed.roots.machines;
    let tables = &mut duplicate_conformance.typed.tables;
    let leaf = &mut tables.machines.span_mut_or_empty(machine_roots)[leaf_index];
    tables
        .machine_trait_conformances
        .append_to_span(&mut leaf.satisfies, duplicate);
    let diagnostics = project_checked_package_review(&duplicate_conformance)
        .expect_err("multiple external conformances must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has 2 conformance applications; expected exactly one")
    }));

    let mut mismatched_conformance_binding = checked.clone();
    let different_binding = mismatched_conformance_binding
        .typed
        .external_bindings
        .intern(psi_language_semantics::ExternalBindingIdentity::Syscall { number: 61 });
    let satisfies = mismatched_conformance_binding
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    mismatched_conformance_binding
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(different_binding);
    let diagnostics = project_checked_package_review(&mismatched_conformance_binding)
        .expect_err("different valid conformance binding must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance binding inconsistent with its supply mode")
    }));

    let mut nonexternal_supply = checked.clone();
    nonexternal_supply
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .supply_mode = psi_language_semantics::MachineSupplyMode::Boundary;
    let diagnostics = project_checked_package_review(&nonexternal_supply)
        .expect_err("external conformance binding on ordinary supply must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("external conformance binding without external supply")
    }));

    let malformed = [
        (
            psi_language_semantics::ExternalBindingIdentity::Import {
                library: String::new(),
                symbol: "entry".to_owned(),
            },
            "has no exact import-library identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::Import {
                library: "omega".to_owned(),
                symbol: String::new(),
            },
            "has no exact import-symbol identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::Syscall { number: -1 },
            "has a syscall number outside 0..=u32::MAX",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::VtableSlot { index: -1 },
            "has a negative vtable-slot index",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::VtableField {
                field: String::new(),
            },
            "has no exact table-field identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::TableFunction {
                field: "invoke".to_owned(),
            },
            "has table-field supply without one exact attached provider data declaration",
        ),
    ];
    for (identity, expected) in malformed {
        let mut tampered = checked.clone();
        replace_external_binding(&mut tampered, identity);
        let diagnostics = project_checked_package_review(&tampered)
            .expect_err("malformed external binding payload must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing diagnostic containing {expected:?}: {diagnostics:?}"
        );
    }
}
