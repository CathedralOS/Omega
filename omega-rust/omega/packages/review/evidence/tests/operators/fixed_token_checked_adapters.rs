use crate::support::*;

const BUILD: &str = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    let [application] = review.boundary_application_realizations() else {
        panic!("one exact fixed-token application realization")
    };
    assert_eq!(
        application.application(),
        PackageReviewBoundaryApplication::Empty
    );
    assert_eq!(
        application.role(),
        PackageReviewBoundaryApplicationRealizationRole::NongenericCheckedBody
    );
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
    let expected = psi_language_semantics::const_value::CanonicalConstIdentity::integer("u64", 4);
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
) -> Element
requires index == 0u64;

pub data IndexingProvider {}
pub machine IndexingProvider::index<Value, const Length: u64>(
    items: Buffer<Value, Length>,
    index: u64
) -> Value
satisfies Indexing::index
requires index == 0u64
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
    let expected = psi_language_semantics::const_value::CanonicalConstIdentity::integer("u64", 4);
    assert_eq!(value_type, &expected.type_name);
    assert_eq!(value_encoding, &expected.encoding);
    assert_eq!(
        application.role(),
        PackageReviewBoundaryApplicationRealizationRole::SpecializedCheckedBody
    );
}

#[test]
fn fixed_token_index_source_custody_checks_both_original_operands() {
    if host_target_name().is_none() {
        return;
    }
    let checked = compile_fixture(
        r#"pub data Buffer { value: i32; }
pub data Indexing {}
pub boundary operator [] Indexing::index(items: Buffer, index: u64) -> i32
requires index == 0u64;
pub data Provider {}
pub machine Provider::index(items: Buffer, index: u64) -> i32
satisfies Indexing::index
requires index == 0u64 { items.value }
pub machine exercise(items: Buffer, index: u64) -> i32
requires index == 0u64 { items[index] }
"#,
    );
    let source = checked
        .pre_selected_dispatch_source_trees()
        .expect("fixed-token indexing has an exact source view");
    let machine = source
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .unwrap();
    let entry = &source.machine_states(machine)[0];
    let indexed = source
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .find_map(|statement| {
            let psi_typed_trees::statement::StatementNode::Expression(handle) = statement else {
                return None;
            };
            let psi_typed_trees::expression::ExpressionNode::Indexed(indexed) =
                source.expression_table.expression(*handle)
            else {
                return None;
            };
            Some(*indexed)
        })
        .expect("restored source indexing, not the selected direct call");
    for operand in [indexed.collection, indexed.index] {
        let mut altered = checked.clone();
        *altered.typed.expression_table.expression_mut(operand) =
            psi_typed_trees::expression::ExpressionNode::Boolean(false);
        assert!(
            altered.pre_selected_dispatch_source_trees().is_err(),
            "changing either original indexing operand invalidates custody"
        );
    }
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
            .contains("lifetime-parameterized fixed-token boundary operator")
    }));
}
