use crate::support::*;

#[test]
fn review_projects_external_top_level_requirement_supply_with_exact_overload_identity() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data InterruptAcknowledgement [copy] { token: u64; }
pub data LinuxCompletion {}

pub boundary requirement InterruptAcknowledgement::complete(self);

pub machine LinuxCompletion::complete(acknowledgement: InterruptAcknowledgement)
    satisfies InterruptAcknowledgement::complete
    via Binding::Syscall(60);
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
    .expect("external top-level requirement fixture should check and select");
    let review = project_checked_package_review(&checked)
        .expect("external top-level requirement supply should project exactly");

    let [supply] = review.external_executable_supply() else {
        panic!("one exact external executable-supply row")
    };
    assert_eq!(supply.callable().path(), "LinuxCompletion::complete");
    assert_eq!(supply.signature().lifetime_parameter_count(), 0);
    assert!(supply.signature().static_parameters().is_empty());
    assert_eq!(supply.signature().parameters().len(), 1);
    assert_eq!(
        supply.binding(),
        &PackageReviewExternalBinding::Syscall { number: 60 }
    );
    assert_eq!(supply.conformance(), None);
    assert_eq!(supply.operator(), None);
    let requirement = supply
        .top_level_requirement()
        .expect("top-level requirement classification");
    let selected = review
        .selected_providers()
        .iter()
        .find(|provider| {
            provider
                .row_declarations()
                .iter()
                .any(|row| row.realization() == supply.callable())
        })
        .expect("selected external top-level provider");
    let [selected_row] = selected.row_declarations() else {
        panic!("one selected top-level requirement row")
    };
    assert_eq!(selected_row.requirement(), requirement);
    assert!(matches!(
        supply.requirement(),
        PackageReviewExternalRequirement::TopLevelRequirement { identity, .. }
            if identity == requirement
    ));

    let rows = review
        .canonical_rows()
        .expect("canonical external top-level supply rows");
    let supply_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("canonical external executable-supply row");
    assert_eq!(
        supply_row.risk(),
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    );
    let encoded = encode_package_review_canonical_row(supply_row)
        .expect("external top-level supply recovery envelope should encode");
    let decoded = decode_package_review_canonical_row(&encoded)
        .expect("external top-level supply recovery envelope should decode");
    assert_eq!(decoded.key_bytes(), supply_row.key_bytes());
    assert_eq!(decoded.canonical_bytes(), supply_row.canonical_bytes());
}

#[test]
fn review_projects_every_external_executable_supply_mechanism_as_opaque_blocking() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait ExternalSurface {
    machine imported() reaches ExternalSurface;
    machine syscalled() reaches ExternalSurface;
    machine intrinsic() reaches ExternalSurface;
    machine field() reaches ExternalSurface;
    machine table() reaches ExternalSurface;
}

pub data DispatchTable {
    dispatch: addr;
    invoke: addr;
}

pub machine import_leaf()
    satisfies ExternalSurface::imported
    via Binding::DllImport("libomega", "omega_entry");
pub machine syscall_leaf()
    satisfies ExternalSurface::syscalled
    via Binding::Syscall(61);
machine intrinsic_leaf()
    satisfies ExternalSurface::intrinsic
    via Binding::CompilerIntrinsic;
pub machine DispatchTable::field_leaf()
    satisfies ExternalSurface::field
    via Binding::VtableField(dispatch);
pub machine DispatchTable::table_leaf()
    satisfies ExternalSurface::table
    via Binding::TableFunction(invoke);
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
    .expect("external executable-supply fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("external executable-supply review should close");

    let expected = [
        (
            "import_leaf",
            PackageReviewExternalBinding::Import {
                library: "libomega".to_owned(),
                symbol: "omega_entry".to_owned(),
            },
        ),
        (
            "syscall_leaf",
            PackageReviewExternalBinding::Syscall { number: 61 },
        ),
        (
            "intrinsic_leaf",
            PackageReviewExternalBinding::CompilerIntrinsic,
        ),
        (
            "DispatchTable::field_leaf",
            PackageReviewExternalBinding::VtableField {
                field: "dispatch".to_owned(),
            },
        ),
        (
            "DispatchTable::table_leaf",
            PackageReviewExternalBinding::TableFunction {
                field: "invoke".to_owned(),
            },
        ),
    ];
    let expected_count = expected.len();
    assert_eq!(review.external_executable_supply().len(), expected_count);
    for (callable, binding) in expected {
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path() == callable)
            .unwrap_or_else(|| panic!("missing external supply for {callable}"));
        assert_eq!(supply.binding(), &binding);
        assert_eq!(
            supply
                .conformance()
                .expect("trait-bound external supply")
                .trait_identity()
                .path(),
            "ExternalSurface"
        );
        let callable_row = review
            .callables()
            .iter()
            .find(|candidate| candidate.identity() == supply.callable());
        if callable == "intrinsic_leaf" {
            assert!(
                callable_row.is_none(),
                "a private external leaf must not become public callable API"
            );
        } else {
            assert!(callable_row.is_some_and(|candidate| {
                candidate.supply() == PackageReviewCallableSupply::ExternalRealization
            }));
        }
    }

    let rows = review
        .canonical_rows()
        .expect("canonical external-supply rows");
    let supply_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    assert_eq!(supply_rows.len(), expected_count);
    assert!(supply_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::OpaqueBlocking
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                }) && locations
                    .iter()
                    .filter(|location| {
                        location.role() == PackageReviewSourceLocationRole::ExternalBinding
                            && location.relative_path() == "main.omg"
                            && location.end_byte() - location.start_byte() == 3
                    })
                    .count()
                    == 1
            })
    }));
    for row in supply_rows {
        let encoded = encode_package_review_canonical_row(row)
            .expect("external-supply recovery envelope should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("external-supply recovery envelope should decode");
        assert_eq!(
            decoded.kind(),
            PackageReviewCanonicalRowKind::ExternalExecutableSupply
        );
        assert_eq!(
            decoded.risk(),
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        );
        assert_eq!(decoded.key_bytes(), row.key_bytes());
    }
}

#[test]
fn review_joins_external_boundary_operator_supply_without_implying_visibility() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
pub boundary operator F32::maximum(left: f32, right: f32) -> f32;
pub boundary operator F32::square_root(value: f32) -> f32;

pub data FloatProvider {}
pub machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
machine FloatProvider::maximum(left: f32, right: f32) -> f32
    satisfies F32::maximum
    via Binding::CompilerIntrinsic;
machine FloatProvider::square_root(value: f32) -> f32
    satisfies F32::square_root
    via Binding::CompilerIntrinsic;
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
    .expect("external boundary-operator fixture should check and select exact intrinsics");
    let review = project_checked_package_review(&checked)
        .expect("external boundary-operator supply should project exactly");

    assert_eq!(review.external_executable_supply().len(), 3);
    for (requirement, expected_builtin) in [
        ("minimum", psi_symbols::BuiltinFunction::Min),
        ("maximum", psi_symbols::BuiltinFunction::Max),
        ("square_root", psi_symbols::BuiltinFunction::Sqrt),
    ] {
        let callable_path = format!("FloatProvider::{requirement}");
        let declaration = review
            .public_operators()
            .iter()
            .find(|operator| {
                operator.coordinate().identity().path() == format!("F32::{requirement}")
            })
            .unwrap_or_else(|| panic!("missing public operator {requirement}"));
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path() == callable_path)
            .unwrap_or_else(|| panic!("missing external supply for {callable_path}"));
        assert!(matches!(
            supply.requirement(),
            PackageReviewExternalRequirement::Operator(operator)
                if operator == declaration.coordinate()
        ));
        assert_eq!(supply.operator(), Some(declaration.coordinate()));
        assert_eq!(supply.conformance(), None);
        assert_eq!(
            supply.binding(),
            &PackageReviewExternalBinding::CompilerIntrinsic
        );

        let selected = review
            .selected_providers()
            .iter()
            .find(|provider| provider.schema_declaration() == declaration.coordinate().identity())
            .unwrap_or_else(|| panic!("missing selected provider for {requirement}"));
        let [selected_row] = selected.row_declarations() else {
            panic!("one selected realization for {requirement}")
        };
        assert_eq!(selected_row.realization(), supply.callable());
        assert_eq!(
            selected_row.requirement().owner(),
            declaration.coordinate().identity().owner()
        );
        assert_eq!(
            selected_row.compiler_intrinsic_builtin(),
            Some(expected_builtin),
            "selected provider review must retain the closed builtin child separately from the authored realization",
        );
        assert!(matches!(
            selected.rows()[0].binding,
            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
        ));
    }

    let public_callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "FloatProvider::minimum")
        .expect("public external operator leaf should remain public callable API");
    assert_eq!(
        public_callable.supply(),
        PackageReviewCallableSupply::ExternalRealization
    );
    assert_eq!(public_callable.operator_realizations().len(), 1);
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "FloatProvider::maximum"),
        "private external operator leaf must not become public callable API"
    );

    let rows = review
        .canonical_rows()
        .expect("canonical external operator-supply rows");
    let supply_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    assert_eq!(supply_rows.len(), 3);
    assert!(supply_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::OpaqueBlocking
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                }) && locations
                    .iter()
                    .filter(|location| {
                        location.role() == PackageReviewSourceLocationRole::ExternalBinding
                            && location.relative_path() == "main.omg"
                            && location.end_byte() - location.start_byte() == 3
                    })
                    .count()
                    == 1
            })
    }));
    for row in supply_rows {
        let encoded = encode_package_review_canonical_row(row)
            .expect("external operator-supply recovery envelope should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("external operator-supply recovery envelope should decode");
        assert_eq!(
            decoded.kind(),
            PackageReviewCanonicalRowKind::ExternalExecutableSupply
        );
        assert_eq!(
            decoded.risk(),
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        );
        assert_eq!(decoded.key_bytes(), row.key_bytes());
    }
    let selected_provider_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .expect("canonical selected-provider identity");
    let encoded = encode_package_review_canonical_row(selected_provider_row)
        .expect("selected-provider recovery envelope should encode");
    let decoded = decode_package_review_canonical_row(&encoded)
        .expect("selected-provider recovery envelope should decode");
    assert_eq!(
        decoded.canonical_bytes(),
        selected_provider_row.canonical_bytes(),
        "canonical recovery must preserve the builtin ordinals",
    );
}
