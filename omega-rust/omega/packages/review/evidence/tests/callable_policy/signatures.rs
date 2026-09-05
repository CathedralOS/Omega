use super::*;

#[test]
fn full_static_lifetime_and_contract_signatures_preserve_relations_not_binder_names() {
    let source = r#"
pub machine borrow<'source, 'temporary>(source: &'source [u8], temporary: &'temporary [u8]) -> &'source [u8] { source }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
pub machine inspect<Element, const Count: u64, machine Work>(value: &[Element; Count]) -> u64
where machine Work(input: Element) -> u64;
{ 0 }
pub machine retain(value: u64) -> u64
requires value >= 1
ensures value >= 1
{ value }
"#;
    let original = project(&Fixture::local(source));
    let renamed_source = source
        .replace("'source", "'origin")
        .replace("'temporary", "'scratch")
        .replace("Element", "Item")
        .replace("Count", "Width")
        .replace("Work", "Operation");
    let renamed = project(&Fixture::local(&renamed_source));
    assert_eq!(original, renamed);
    assert_eq!(callable(&original, "borrow").lifetime_parameter_count(), 2);
    assert_eq!(callable(&original, "identity").type_parameters().len(), 1);
    let inspect = callable(&original, "inspect");
    assert_eq!(inspect.type_parameters().len(), 3);
    assert!(matches!(
        inspect.type_parameters()[1].kind(),
        PackageReviewTypeParameterKind::Const(_)
    ));
    assert!(matches!(
        inspect.type_parameters()[2].kind(),
        PackageReviewTypeParameterKind::Machine(_)
    ));
    assert_eq!(callable(&original, "retain").contracts().len(), 2);
    let changed_source = source.replace(
        "-> &'source [u8] { source }",
        "-> &'temporary [u8] { temporary }",
    );
    let changed = project(&Fixture::local(&changed_source));
    assert_ne!(
        callable(&original, "borrow").return_type(),
        callable(&changed, "borrow").return_type()
    );
    assert_ne!(
        original.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}

#[test]
fn unused_private_admission_claim_remains_an_explicit_assumption() {
    let zero = project(&Fixture::local(
        "boundary machine trusted_zero() -> u64 ensures result == 0;\n",
    ));
    let one = project(&Fixture::local(
        "boundary machine trusted_zero() -> u64 ensures result == 1;\n",
    ));
    let claim = callable(&zero, "trusted_zero");
    assert_eq!(claim.role(), PackagePolicyCallableRole::PrivateAssumption);
    assert_eq!(claim.supply(), PackageReviewCallableSupply::AdmissionClaim);
    assert_eq!(
        claim.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert_eq!(claim.contracts().len(), 1);
    assert_ne!(
        claim.contracts(),
        callable(&one, "trusted_zero").contracts()
    );
    assert_ne!(
        zero.canonical_bytes().unwrap(),
        one.canonical_bytes().unwrap()
    );
}

#[test]
fn callable_conformance_bounds_and_exact_requirement_meaning_are_retained() {
    let source = r#"
pub trait Ranked {}
pub machine identity<Element>(value: Element) -> Element
where Element satisfies Ranked
{ value }
pub trait Echo { machine echo(value: u64) -> u64; }
pub data EchoProvider {}
pub EchoProviderEcho: EchoProvider satisfies Echo;
pub machine EchoProvider::echo(value: u64) -> u64 satisfies Echo::echo { value }
"#;
    let original = project(&Fixture::local(source));
    let [bound] = callable(&original, "identity").conformance_bounds() else {
        panic!("one binder-free generic conformance bound")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(bound.trait_identity().path(), "Ranked");
    assert_eq!(
        bound.trait_identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let [conformance] = callable(&original, "EchoProvider::echo").conformances() else {
        panic!("one exact callable requirement")
    };
    assert_eq!(conformance.trait_identity().path(), "Echo");
    assert_eq!(
        conformance.requirement_identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let changed = project(&Fixture::local(&source.replace("Ranked", "Ordered")));
    assert_ne!(
        callable(&original, "identity").conformance_bounds(),
        callable(&changed, "identity").conformance_bounds()
    );
    assert_ne!(
        original.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}

#[test]
fn same_named_generic_overloads_keep_distinct_normalized_identities() {
    let source = r#"
pub machine inspect<'borrow, Element, const Count: u64>(value: &'borrow [Element; Count]) -> u64 { 0 }
pub machine inspect<'borrow, Element, const Count: u64>(value: &'borrow [Element; Count]) -> u64 in Saturating { 0 as u64 in Saturating }
"#;
    let fixture = Fixture::local(source);
    let checked = fixture
        .checked
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str() == "inspect")
        .collect::<Vec<_>>();
    assert_eq!(checked.len(), 2);
    assert_ne!(checked[0].symbol, checked[1].symbol);
    assert_eq!(
        fixture
            .checked
            .symbols
            .display_path(checked[0].symbol, "::"),
        fixture
            .checked
            .symbols
            .display_path(checked[1].symbol, "::")
    );
    let policy = project(&fixture);
    let overloads = policy
        .callables()
        .iter()
        .filter(|callable| callable_has_name(callable, "inspect"))
        .collect::<Vec<_>>();
    assert_eq!(overloads.len(), 2);
    assert_ne!(overloads[0].identity(), overloads[1].identity());
    assert_eq!(overloads[0].parameters(), overloads[1].parameters());
    assert_ne!(overloads[0].return_type(), overloads[1].return_type());
    for overload in &overloads {
        assert_eq!(overload.lifetime_parameter_count(), 1);
        assert_eq!(overload.type_parameters().len(), 2);
    }
    let renamed_source = source
        .replace("'borrow", "'view")
        .replace("Element", "Item")
        .replace("Count", "Length");
    let renamed = project(&Fixture::local(&renamed_source));
    assert_eq!(policy, renamed);
    assert_eq!(
        policy.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );
}

#[test]
fn exact_callable_conformance_retains_actual_caller_lifetime_not_only_partition() {
    let source = r#"
pub boundary trait LifetimeSlot<'scope> { machine perform(value: u64) -> u64; }
pub data Provider {}
pub machine Provider::perform<'left, 'right>(value: u64) -> u64
satisfies LifetimeSlot<'left>::perform
{ value }
"#;
    let first = project(&Fixture::local(source));
    let second = project(&Fixture::local(
        &source.replace("LifetimeSlot<'left>", "LifetimeSlot<'right>"),
    ));
    let first_callable = callable(&first, "Provider::perform");
    let second_callable = callable(&second, "Provider::perform");
    assert_eq!(first_callable.parameters(), second_callable.parameters());
    assert_eq!(first_callable.return_type(), second_callable.return_type());
    let [first_application] = first_callable.conformances() else {
        panic!("one checked conformance application")
    };
    let [second_application] = second_callable.conformances() else {
        panic!("one changed conformance application")
    };
    assert_eq!(first_application.requirement_lifetime_partition(), &[0]);
    assert_eq!(
        first_application.requirement_lifetime_partition(),
        second_application.requirement_lifetime_partition()
    );
    assert_eq!(first_application.trait_lifetime_arguments(), &[0]);
    assert_eq!(second_application.trait_lifetime_arguments(), &[1]);
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    let renamed = project(&Fixture::local(
        &source
            .replace("'left", "'first")
            .replace("'right", "'second")
            .replace("'scope", "'view"),
    ));
    assert_eq!(first, renamed);
}
