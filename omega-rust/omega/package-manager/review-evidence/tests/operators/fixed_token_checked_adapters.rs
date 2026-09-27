use crate::support::*;
use compiler::CheckedCompileRequest;
use package_evidence::{project_checked_package_policy, project_checked_package_review};
use target::TargetProfile;

const BUILD: &str = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;

const BINARY_SUBTRACT: &str = r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32;

pub data CheckedMathProvider {}
pub machine CheckedMathProvider::subtract_impl(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract
{
    left
}

pub machine exercise(left: i32, right: i32) -> i32
requires 0i32 <= right, right <= left, left <= 100i32
{
    left - right
}
"#;

fn compile_fixture(source: &str) -> ReviewFixture {
    let target = host_target_name().expect("host target fixture");
    let package = TempPackage::new();
    package.write("main.omg", source);
    package.write("build.omg", BUILD);
    compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .unwrap_or_else(|diagnostics| {
        panic!("fixed-token adapter fixture should check: {diagnostics:?}")
    })
}

#[test]
fn review_projects_fixed_token_generic_checked_adapter_application() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(
        r#"pub data GenericMath {}
pub boundary operator - GenericMath::subtract<Element>(
    left: Element,
    right: Element
) -> Element;

pub data GenericMathProvider {}
pub machine GenericMathProvider::subtract<Value>(left: Value, right: Value) -> Value
satisfies GenericMath::subtract
{ left }

pub machine exercise(left: i32, right: i32) -> i32
requires 0i32 <= right, right <= left, left <= 100i32
{
    left - right
}
"#,
    );
    let review = project_checked_package_review(&checked)
        .expect("fixed-token generic checked adapter application should project");
    let [application] = review.boundary_application_realizations() else {
        panic!("one exact fixed-token generic application")
    };
    let PackageReviewBoundaryApplication::Exact(arguments) = application.application() else {
        panic!("fixed-token generic use has a nonempty application")
    };
    let [PackageReviewBoundaryApplicationArgument::Type { type_identity, .. }] =
        arguments.as_slice()
    else {
        panic!("fixed-token generic use retains one type argument")
    };
    assert!(type_identity.canonical().contains("i32"));
    assert_eq!(
        application.role(),
        PackageReviewBoundaryApplicationRealizationRole::SpecializedCheckedBody
    );
}

#[test]
fn review_projects_fixed_token_const_generic_checked_adapter_application() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(
        r#"pub data Vector<const Count: u64> {
    value: u8
}

pub data ConstMath {}
pub boundary operator + ConstMath::combine<const Count: u64>(
    left: Vector<Count>,
    right: Vector<Count>
) -> Vector<Count>;

pub data ConstMathProvider {}
pub machine ConstMathProvider::combine<const Length: u64>(
    left: Vector<Length>,
    right: Vector<Length>
) -> Vector<Length>
satisfies ConstMath::combine
{ left }

pub machine exercise(left: Vector<4>, right: Vector<4>) -> Vector<4> {
    left + right
}
"#,
    );
    let review = project_checked_package_review(&checked)
        .expect("fixed-token const-generic checked adapter application should project");
    let [application] = review.boundary_application_realizations() else {
        panic!("one exact fixed-token const-generic application")
    };
    let PackageReviewBoundaryApplication::Exact(arguments) = application.application() else {
        panic!("fixed-token const-generic use has a nonempty application")
    };
    let [
        PackageReviewBoundaryApplicationArgument::Const {
            binder_ordinal,
            declared_carrier,
            value_type,
            value_encoding,
        },
    ] = arguments.as_slice()
    else {
        panic!("fixed-token const-generic use retains one const argument")
    };
    assert_eq!(*binder_ordinal, 0);
    assert!(declared_carrier.canonical().contains("u64"));
    let expected = language_semantics::const_value::CanonicalConstIdentity::integer("u64", 4);
    assert_eq!(value_type, &expected.type_name);
    assert_eq!(value_encoding, &expected.encoding);
    assert_eq!(
        application.role(),
        PackageReviewBoundaryApplicationRealizationRole::SpecializedCheckedBody
    );
}

#[test]
fn review_projects_fixed_token_type_and_const_generic_index_application() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(
        r#"pub data Buffer<Element, const Count: u64> {
    value: Element
}

pub data Indexing {}
pub boundary operator [] Indexing::index<Element, const Count: u64>(
    items: Buffer<Element, Count>,
    index: u64
) -> Element;

pub data IndexingProvider {}
pub machine IndexingProvider::index<Value, const Length: u64>(
    items: Buffer<Value, Length>,
    index: u64
) -> Value
satisfies Indexing::index
{ items.value }

pub machine exercise(items: Buffer<i32, 4>, index: u64) -> i32
requires index == 0u64
{
    items[index]
}
"#,
    );
    let review = project_checked_package_review(&checked)
        .expect("fixed-token generic index application should project");
    let [application] = review.boundary_application_realizations() else {
        panic!("one exact fixed-token generic index application")
    };
    let PackageReviewBoundaryApplication::Exact(arguments) = application.application() else {
        panic!("fixed-token generic index use has a nonempty application")
    };
    let [
        PackageReviewBoundaryApplicationArgument::Type { type_identity, .. },
        PackageReviewBoundaryApplicationArgument::Const {
            binder_ordinal,
            declared_carrier,
            value_type,
            value_encoding,
        },
    ] = arguments.as_slice()
    else {
        panic!("fixed-token generic index use retains its type and const arguments")
    };
    assert!(type_identity.canonical().contains("i32"));
    assert_eq!(*binder_ordinal, 1);
    assert!(declared_carrier.canonical().contains("u64"));
    let expected = language_semantics::const_value::CanonicalConstIdentity::integer("u64", 4);
    assert_eq!(value_type, &expected.type_name);
    assert_eq!(value_encoding, &expected.encoding);
    assert_eq!(
        application.role(),
        PackageReviewBoundaryApplicationRealizationRole::SpecializedCheckedBody
    );
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
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
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
        .intern(language_semantics::ExternalBindingIdentity::Syscall { number: 60 });
    let satisfies = external.typed.machines()[provider_index].satisfies;
    external.typed.machines_mut()[provider_index].supply_mode =
        language_semantics::MachineSupplyMode::ExternalRealization {
            binding: Some(binding),
            mechanism: Some(language_semantics::ExternalBindingMechanism::Syscall),
        };
    external.typed.machines_mut()[provider_index].body_is_present = false;
    external
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(binding);
    external.facts.operators.operator_realization_contracts =
        typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &external.typed,
        );
    let diagnostics = project_checked_package_review(&external)
        .expect_err("post-check externalization of a checked adapter must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("disagrees with its exact checked, legacy, or evaluated supply carrier")
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
        typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(&generic.typed);
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
        .push(typed_trees::name::Identifier::generated_static(
            "forged_lifetime",
        ));
    lifetime.facts.operators.operator_realization_contracts =
        typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &lifetime.typed,
        );
    let diagnostics = project_checked_package_review(&lifetime)
        .expect_err("post-check lifetime-bearing fixed-token adapter must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lifetime-parameterized fixed-token boundary operator")
    }));
}

#[test]
fn boundary_machine_checked_realization_retains_its_requirement_identity() {
    // A token machine is a machine requirement, not a legacy operator
    // declaration. Both forms must review, but their distinct declaration
    // roles and exact conformance associations must survive projection.
    if host_target_name().is_none() {
        return;
    }
    let operator_form = BINARY_SUBTRACT.replace(
        "pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32;",
        "pub boundary operator - CheckedMath::subtract(left: i32, right: i32) -> i32\nrequires 0i32 <= right;",
    );
    let machine_form = operator_form.replace(
        "pub boundary operator - CheckedMath::subtract",
        "pub boundary machine - CheckedMath::subtract",
    );
    let target = TargetProfile::MacosArm64;
    let compile = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", BUILD);
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target.target_name()))
        })
        .unwrap_or_else(|diagnostics| panic!("spelling fixture should check: {diagnostics:?}"))
    };
    let operator_checked = compile(&operator_form);
    let machine_checked = compile(&machine_form);
    let operator_policy =
        project_checked_package_policy(&operator_checked, target, package_identity())
            .expect("operator spelling policy");
    let machine_policy =
        project_checked_package_policy(&machine_checked, target, package_identity())
            .expect("machine spelling policy");
    assert_ne!(operator_policy, machine_policy);
    let callables = machine_policy.callables();
    let requirement = callables
        .callables()
        .iter()
        .find(|callable| callable.supply() == PackageReviewCallableSupply::TopLevelRequirement)
        .expect("top-level requirement");
    let provider = callables
        .callables()
        .iter()
        .find(|callable| !callable.requirement_realizations().is_empty())
        .expect("checked provider association");
    assert!(provider.operator_realizations().is_empty());
    assert!(provider.conformances().is_empty());
    let [realization] = provider.requirement_realizations() else {
        panic!("one exact requirement")
    };
    let typed_requirement = machine_checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "CheckedMath::subtract")
        .unwrap();
    assert_eq!(
        realization.requirement_identity().owner(),
        requirement.identity().owner()
    );
    assert_eq!(
        realization.requirement_identity().path(),
        machine_checked
            .typed
            .normalized_machine_overload_identity(typed_requirement)
            .unwrap()
            .identity()
    );
    assert_eq!(realization.alias(), None);
    let bytes = callables.canonical_bytes().unwrap();
    assert_eq!(
        package_evidence::record::PackagePolicyCallables::recover_canonical(
            &bytes,
            package_evidence::encoding::PackagePolicyRecoveryLimits::default(),
        )
        .unwrap(),
        *callables,
    );
    let review = project_checked_package_review(&machine_checked).unwrap();
    let provider = review
        .callables()
        .iter()
        .find(|callable| !callable.requirement_realizations().is_empty())
        .expect("fresh review includes checked association");
    assert_eq!(
        provider.requirement_realizations(),
        std::slice::from_ref(realization)
    );
    assert_ne!(
        project_checked_package_review(&operator_checked).unwrap(),
        review
    );
}

#[test]
fn checked_requirement_review_revalidates_the_exact_edge_and_body() {
    if host_target_name().is_none() {
        return;
    }
    let source = BINARY_SUBTRACT.replace("boundary operator", "boundary machine");
    let checked = compile_fixture(&source);
    let provider_index = checked
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "CheckedMathProvider::subtract_impl")
        .unwrap();
    let requirement = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "CheckedMath::subtract")
        .unwrap()
        .symbol;
    project_checked_package_review(&checked).expect("valid checked requirement");

    let mut missing_body = checked.clone();
    missing_body.typed.machines_mut()[provider_index].body_is_present = false;
    assert!(project_checked_package_review(&missing_body).is_err());

    let mut substituted = checked.clone();
    let satisfies = substituted.typed.machines()[provider_index].satisfies;
    substituted
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .requirement_symbol = substituted.typed.machines()[provider_index].symbol;
    assert!(project_checked_package_review(&substituted).is_err());

    let mut stronger_effect = checked.clone();
    stronger_effect.typed.machines_mut()[provider_index].suspends = true;
    assert!(project_checked_package_review(&stronger_effect).is_err());

    let mut hidden = checked;
    hidden
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == requirement)
        .unwrap()
        .is_public = false;
    assert!(project_checked_package_review(&hidden).is_err());
}

#[test]
fn named_generic_checked_requirement_reviews_without_an_operator_view() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(
        r#"
pub data Choice {}
pub boundary requirement Choice::choose<Element>(value: Element) -> Element;
pub data Provider {}
pub machine Provider::choose<Value>(value: Value) -> Value satisfies Choice::choose { value }
"#,
    );
    let review = project_checked_package_review(&checked).expect("generic requirement association");
    let provider = review
        .callables()
        .iter()
        .find(|callable| !callable.requirement_realizations().is_empty())
        .unwrap();
    assert!(provider.operator_realizations().is_empty());
    assert_eq!(provider.requirement_realizations().len(), 1);
}

#[test]
fn overloaded_checked_requirements_have_distinct_canonical_review_rows() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(
        r#"
pub data Choice {}
pub boundary machine - Choice::choose(left: i32, right: i32) -> i32;
pub boundary machine - Choice::choose(left: u32, right: u32) -> u32;
pub data Provider {}
pub machine Provider::signed(left: i32, right: i32) -> i32 satisfies Choice::choose { right }
pub machine Provider::unsigned(left: u32, right: u32) -> u32 satisfies Choice::choose { right }
"#,
    );
    let review = project_checked_package_review(&checked).unwrap();
    let requirements = review
        .callables()
        .iter()
        .filter(|callable| callable.supply() == PackageReviewCallableSupply::TopLevelRequirement)
        .collect::<Vec<_>>();
    assert_eq!(requirements.len(), 2);
    assert_ne!(requirements[0].identity(), requirements[1].identity());
    for provider in review
        .callables()
        .iter()
        .filter(|callable| !callable.requirement_realizations().is_empty())
    {
        let [realization] = provider.requirement_realizations() else {
            panic!("one exact requirement")
        };
        assert_eq!(
            requirements
                .iter()
                .filter(|requirement| {
                    requirement.identity() == realization.requirement_identity()
                })
                .count(),
            1
        );
    }
    review
        .canonical_rows()
        .expect("overloaded requirements must not collide in persisted rows");
}
