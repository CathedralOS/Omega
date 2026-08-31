use crate::support::*;

const BUILD: &str = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;

const BINARY_SUBTRACT: &str = r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32;

pub data CheckedMathProvider {}
pub machine CheckedMathProvider::subtract_impl(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract
{
    left
}
"#;

fn compile_fixture(source: &str) -> omega_compiler::CheckedCompilation {
    let target = host_target_name().expect("host target fixture");
    let package = TempPackage::new();
    package.write("main.omg", source);
    package.write("build.omg", BUILD);
    compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .unwrap_or_else(|diagnostics| {
        panic!("fixed-token adapter fixture should check: {diagnostics:?}")
    })
}

#[test]
fn review_admits_binary_fixed_token_boundary_checked_adapter() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(BINARY_SUBTRACT);
    let review = project_checked_package_review(&checked)
        .expect("binary fixed-token checked adapter should enter package review");

    let declaration = review
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "CheckedMath::subtract")
        .expect("public fixed-token boundary declaration");
    assert!(declaration.is_boundary());
    assert_eq!(
        declaration.spelling(),
        Some(psi_language_core::OperatorSpelling::Subtract)
    );

    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "CheckedMathProvider::subtract_impl")
        .expect("public fixed-token checked adapter callable");
    let [realization] = callable.operator_realizations() else {
        panic!("one exact fixed-token boundary realization")
    };
    assert_eq!(realization.coordinate(), declaration.coordinate());
    assert_eq!(realization.alias(), None);

    let provider = review
        .selected_providers()
        .iter()
        .find(|provider| provider.schema_declaration() == declaration.coordinate().identity())
        .expect("selected fixed-token boundary provider");
    let [row] = provider.rows() else {
        panic!("one exact fixed-token provider row")
    };
    assert!(matches!(
        row.binding,
        omega_effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
    ));
}

#[test]
fn fixed_token_checked_adapter_neighbors_remain_fail_closed() {
    let Some(target) = host_target_name() else {
        return;
    };
    let cases = [
        (
            "unsupported unary subtraction",
            r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::negate(value: i32) -> i32;
pub data CheckedMathProvider {}
pub machine CheckedMathProvider::negate_impl(input: i32) -> i32
satisfies CheckedMath::negate
{ input }
"#,
            "unsupported dispatch shape `-` and 1 normalized operands",
        ),
        (
            "unsupported range",
            r#"pub data CheckedMath {}
pub boundary operator [..] CheckedMath::range(items: i32, start: i32, end: i32) -> i32;
pub data CheckedMathProvider {}
pub machine CheckedMathProvider::range_impl(items: i32, start: i32, end: i32) -> i32
satisfies CheckedMath::range
{ items }
"#,
            "unsupported dispatch shape `[..]` and 3 normalized operands",
        ),
        (
            "aliased binary subtraction",
            r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32;
pub data CheckedMathProvider {}
pub machine CheckedMathProvider::subtract_impl(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract as Selected
{ left }
"#,
            "through an alias not represented by checked-adapter token dispatch",
        ),
        (
            "bodyless binary subtraction",
            r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32;
pub data CheckedMathProvider {}
pub boundary machine CheckedMathProvider::subtract_impl(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract;
"#,
            "without one checked implementation body",
        ),
    ];

    for (label, source, expected) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", BUILD);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .unwrap_or_else(|diagnostics| panic!("{label} fixture should check: {diagnostics:?}"));
        let diagnostics = project_checked_package_review(&checked)
            .expect_err("unsupported fixed-token checked-adapter neighbor must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{label}: {diagnostics:?}"
        );
    }
}

#[test]
fn fixed_token_checked_adapter_rejects_post_check_overload_drift() {
    if host_target_name().is_none() {
        return;
    }
    let mut checked = compile_fixture(
        r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32;
pub boundary operator + CheckedMath::add(left: i32, right: i32) -> i32;

pub data CheckedMathProvider {}
pub machine CheckedMathProvider::subtract_impl(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract
{ left }
"#,
    );
    let add_symbol = checked
        .typed
        .operators()
        .iter()
        .find(|operator| checked.typed.operator_path_members(operator.name)[1].as_str() == "add")
        .expect("decoy add declaration")
        .symbol;
    let provider_index = checked
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str().ends_with("subtract_impl"))
        .expect("subtract provider machine");
    let satisfies = checked.typed.machines()[provider_index].satisfies;
    checked
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .requirement_symbol = add_symbol;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check fixed-token overload drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
            || diagnostic
                .message
                .contains("drifted from its retained exact overload")
    }));
}

#[test]
fn fixed_token_checked_adapter_rejects_post_check_external_and_binder_drift() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(
        r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32;
machine generic<Element>() { }

pub data CheckedMathProvider {}
pub machine CheckedMathProvider::subtract_impl(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract
{ left }
"#,
    );

    let provider_index = checked
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str().ends_with("subtract_impl"))
        .expect("subtract provider machine");

    let mut external = checked.clone();
    let binding = external
        .typed
        .external_bindings
        .intern(psi_language_semantics::ExternalBindingIdentity::Syscall { number: 60 });
    let satisfies = external.typed.machines()[provider_index].satisfies;
    external.typed.machines_mut()[provider_index].supply_mode =
        psi_language_semantics::MachineSupplyMode::ExternalRealization {
            binding: Some(binding),
            mechanism: Some(psi_language_semantics::ExternalBindingMechanism::Syscall),
        };
    external.typed.machines_mut()[provider_index].body_is_present = false;
    external
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(binding);
    external.facts.operators.operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &external.typed,
        );
    let diagnostics = project_checked_package_review(&external)
        .expect_err("post-check externalization of a checked adapter must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("has no exact authored `via` custody")
        }),
        "{diagnostics:?}"
    );

    let generic_parameters = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generic")
        .expect("generic helper machine")
        .type_parameters;
    let mut generic = checked.clone();
    generic.typed.machines_mut()[provider_index].type_parameters = generic_parameters;
    generic.facts.operators.operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &generic.typed,
        );
    let diagnostics = project_checked_package_review(&generic)
        .expect_err("post-check generic fixed-token adapter must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "resolves to neither one exact trait requirement nor one exact checked operator"
            )),
        "{diagnostics:?}"
    );

    let mut lifetime = checked;
    lifetime.typed.machines_mut()[provider_index]
        .lifetime_parameters
        .push(psi_typed_trees::name::Identifier::generated_static(
            "forged_lifetime",
        ));
    lifetime.facts.operators.operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &lifetime.typed,
        );
    let diagnostics = project_checked_package_review(&lifetime)
        .expect_err("post-check lifetime-bearing fixed-token adapter must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("generic or lifetime-parameterized fixed-token boundary operator")
    }));
}
