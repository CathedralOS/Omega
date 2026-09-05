//! Const custody and caller-relative lifetime policy projection.

mod support;

use omega_package_evidence::project_checked_conformance_policy;
use omega_package_evidence::record::PackagePolicyConformanceConstArgument;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::typed_trees::ClosedConformanceConstArgument;
use support::*;

fn checked(source: &str) -> (TempPackage, CheckedCompilation) {
    let package = TempPackage::new();
    package.write("main.omg", source);
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("conformance policy fixture checks");
    (package, checked)
}

#[test]
fn conformance_policy_retains_const_carriers_and_value_without_receipts() {
    let (_package, checked) = checked(
        r#"
pub trait Ranked {}
pub data Card {}
pub FieldOrder<Element, const Rank: u64>: Element satisfies Ranked {}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 { 0 }
boundary machine trusted() -> u64
ensures result == tag<Card, FieldOrder<Card, 7>>();
"#,
    );
    let application = &checked
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .application;
    let policy = project_checked_conformance_policy(&checked, application, &[]).unwrap();
    let [
        PackagePolicyConformanceConstArgument::Evaluated {
            parameter_carrier,
            declared_carrier,
            canonical_value_encoding,
        },
    ] = policy.const_arguments()
    else {
        panic!("one evaluated const")
    };
    assert_eq!(parameter_carrier, declared_carrier);
    assert!(declared_carrier.canonical().contains("u64"));
    let ClosedConformanceConstArgument::Evaluated { value, .. } = &application.const_arguments[0]
    else {
        panic!("checked evaluated const")
    };
    assert_eq!(canonical_value_encoding, &value.encoding);
    let mut receipt_changed = application.clone();
    receipt_changed.report_fingerprint = receipt_changed.report_fingerprint.wrapping_add(1);
    receipt_changed.commitment = Default::default();
    assert_eq!(
        policy,
        project_checked_conformance_policy(&checked, &receipt_changed, &[]).unwrap()
    );
    let mut changed = application.clone();
    let ClosedConformanceConstArgument::Evaluated { value, .. } = &mut changed.const_arguments[0]
    else {
        unreachable!()
    };
    value.type_name = "Bool".to_owned();
    assert!(
        project_checked_conformance_policy(&checked, &changed, &[]).is_err(),
        "independent checked value carrier must be rejoined before omission"
    );
    let mut changed = application.clone();
    let ClosedConformanceConstArgument::Evaluated { value, .. } = &mut changed.const_arguments[0]
    else {
        unreachable!()
    };
    value.encoding.push('0');
    assert!(project_checked_conformance_policy(&checked, &changed, &[]).is_err());
    let mut changed = application.clone();
    changed.arguments.swap(0, 1);
    assert!(project_checked_conformance_policy(&checked, &changed, &[]).is_err());
    let mut changed = application.clone();
    let mut nested = changed.arguments[0].clone();
    for _ in 0..65 {
        nested = psi_typed_trees::expression::StaticMachineArgument {
            application: Some(Box::new(
                psi_typed_trees::expression::StaticSymbolApplication {
                    lifetime_arguments: Box::new([]),
                    arguments: vec![nested].into_boxed_slice(),
                },
            )),
            ..changed.arguments[0].clone()
        };
    }
    changed.arguments[0] = nested;
    let diagnostics = project_checked_conformance_policy(&checked, &changed, &[]).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("over-deep")),
        "reject depth before reclosure clones or renders the retained tree"
    );
}

#[test]
fn conformance_policy_lifetimes_preserve_containing_telescope_coordinates() {
    let (_package, checked) = checked(
        r#"
trait Borrows<'borrow, Source> {}
data Card {}
data Borrow<'scope, Element> { value: &'scope Element }
Scoped<'scope, Element>: Element satisfies Borrows<'scope, Borrow<'scope, Element>> {}
machine choose<'call, Element, Evidence: Element satisfies Borrows<Borrow<'call, Element>>>(value: &'call Element) {}
machine caller<'view>(value: &'view Card) { choose<Card, Scoped<'view, Card>>(value); }
"#,
    );
    let application = checked
        .machine_specializations
        .iter()
        .find_map(|specialization| specialization.conformance_applications.first())
        .expect("checked lifetime conformance");
    let binders = [
        Identifier::generated("other"),
        Identifier::generated("view"),
    ];
    let policy = project_checked_conformance_policy(&checked, application, &binders).unwrap();
    assert_eq!(policy.lifetime_arguments(), &[1]);
    assert_eq!(policy.trait_lifetime_arguments(), &[1]);
    let reordered = [
        Identifier::generated("view"),
        Identifier::generated("other"),
    ];
    let changed = project_checked_conformance_policy(&checked, application, &reordered).unwrap();
    assert_eq!(changed.lifetime_arguments(), &[0]);
    assert_ne!(
        policy, changed,
        "caller binders must not collapse to first occurrence"
    );
    assert!(project_checked_conformance_policy(&checked, application, &[]).is_err());
    assert!(
        project_checked_conformance_policy(
            &checked,
            application,
            &[Identifier::generated("view"), Identifier::generated("view")]
        )
        .is_err()
    );
}

#[test]
fn overloaded_generic_callers_retain_distinct_type_and_const_binder_coordinates() {
    let (_package, checked) = checked(
        r#"
pub trait Encodes<Output> {}
pub data Card {}
pub Encoding<Output, const Rank: u64>: Card satisfies Encodes<Output> {}
pub machine inspect<Output, const Rank: u64, Element>(value: &Element) -> u64
where Element satisfies Card::Encoding<Output, Rank>
{ 0 }
pub machine inspect<Output, const Rank: u64, Element>(value: &Element) -> u64 in Saturating
where Element satisfies Card::Encoding<Output, Rank>
{ 0 as u64 in Saturating }
"#,
    );
    let machines = checked
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str() == "inspect")
        .collect::<Vec<_>>();
    assert_eq!(machines.len(), 2);
    let applications = machines
        .iter()
        .map(|machine| {
            let selected = machine.conformance_bounds[0]
                .selected_conformance
                .as_ref()
                .unwrap();
            psi_typed_trees_to_checked_trees::close_conformance_application(
                &checked.typed,
                selected,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for index in 0..2 {
        let first = applications[0].arguments[index].symbol;
        let second = applications[1].arguments[index].symbol;
        assert_ne!(first, second);
        assert_eq!(
            checked.symbols.display_path(first, "::"),
            checked.symbols.display_path(second, "::"),
            "regression requires distinct actual caller binders with identical diagnostic paths"
        );
    }
    let first = project_checked_conformance_policy(&checked, &applications[0], &[]).unwrap();
    let second = project_checked_conformance_policy(&checked, &applications[1], &[]).unwrap();
    assert_ne!(first.type_arguments(), second.type_arguments());
    assert_ne!(first.const_arguments(), second.const_arguments());
    assert_ne!(first.trait_arguments(), second.trait_arguments());
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}

#[test]
fn conformance_policy_retains_distinct_overloaded_requirement_and_realization_rows() {
    let (_package, checked) = checked(
        r#"
trait Shape {
    machine code(value: u64) -> u64;
    machine code(value: u64) -> u64 in Saturating;
    machine finish();
}
data Card {}
Primary: Card satisfies Shape {
    machine code(value: u64) -> u64 { value }
    machine code(value: u64) -> u64 in Saturating { value as u64 in Saturating }
    machine finish() {}
}
"#,
    );
    let declaration = checked
        .conformances()
        .iter()
        .find(|declaration| {
            declaration
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "Primary")
        })
        .unwrap();
    let selected = psi_typed_trees::expression::StaticMachineArgument {
        symbol: declaration.symbol,
        path: vec![declaration.alias.clone().unwrap()].into_boxed_slice(),
        application: None,
        const_literal: None,
        evidence_projection: None,
    };
    let application =
        psi_typed_trees_to_checked_trees::close_conformance_application(&checked.typed, &selected)
            .unwrap();
    assert_eq!(application.rows.len(), 3);
    let policy = project_checked_conformance_policy(&checked, &application, &[]).unwrap();
    assert_eq!(policy.rows().len(), 3);
    assert_ne!(
        policy.rows()[0].requirement(),
        policy.rows()[1].requirement()
    );
    assert_ne!(
        policy.rows()[0].realization_machine(),
        policy.rows()[1].realization_machine()
    );
    assert_ne!(
        policy.rows()[0].realization_state(),
        policy.rows()[1].realization_state()
    );
}
