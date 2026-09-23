use super::{
    Fixture, PackagePolicyCallableRole, PackagePolicyCallables,
    PackagePolicyMachineParameterContract, PackagePolicyTypeParameterKind,
    PackageReviewCallableSupply, PackageReviewCheckedServiceReach, PackageReviewNominalOwner,
    callable, callable_has_name, package_identity, project,
};
#[test]
fn private_external_operational_policy_is_not_replaced_by_its_requirement() {
    let source = r#"
pub boundary trait Host {
    machine ping() suspends; blocks;
}
machine ping_leaf()
satisfies Host::ping via ForeignBinding::CompilerIntrinsic;
"#;
    let quiet = Fixture::local(source);
    let published = Fixture::local(&source.replace(
        "via ForeignBinding::CompilerIntrinsic;",
        "via ForeignBinding::CompilerIntrinsic suspends; blocks;",
    ));
    let quiet_supply = package_evidence::project_checked_external_supply_policy(
        &quiet.checked,
        quiet
            .checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "ping_leaf")
            .unwrap()
            .symbol,
    )
    .unwrap();
    let published_supply = package_evidence::project_checked_external_supply_policy(
        &published.checked,
        published
            .checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "ping_leaf")
            .unwrap()
            .symbol,
    )
    .unwrap();
    assert_eq!(
        quiet_supply, published_supply,
        "the supply signature and binding alone omit outer operational promises"
    );
    let quiet = project(&quiet);
    let published = project(&published);
    let quiet_leaf = callable(&quiet, "ping_leaf");
    let published_leaf = callable(&published, "ping_leaf");
    assert_eq!(
        quiet_leaf.role(),
        PackagePolicyCallableRole::PrivateExternal
    );
    assert_eq!(
        quiet_leaf.supply(),
        PackageReviewCallableSupply::ExternalRealization
    );
    assert_eq!(quiet_leaf.declared_may_suspend(), Some(false));
    assert_eq!(quiet_leaf.declared_may_block(), Some(false));
    assert_eq!(published_leaf.declared_may_suspend(), Some(true));
    assert_eq!(published_leaf.declared_may_block(), Some(true));
    assert_ne!(
        quiet.canonical_bytes().unwrap(),
        published.canonical_bytes().unwrap()
    );
}

#[test]
fn nested_machine_result_absence_is_not_a_dummy_type() {
    let source = "pub machine inspect<machine Work>() -> u64\nwhere machine Work();\n{ 0 }\n";
    let absent = project(&Fixture::local(source));
    let present = project(&Fixture::local(
        &source.replace("Work();", "Work() -> u64;"),
    ));
    let signature = |policy: &PackagePolicyCallables| {
        let PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::Structural(signature),
        ) = callable(policy, "inspect").type_parameters()[0].kind()
        else {
            panic!("one structural machine parameter")
        };
        signature.return_type().cloned()
    };
    assert!(signature(&absent).is_none());
    assert!(signature(&present).is_some());
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        present.canonical_bytes().unwrap()
    );
}

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
        PackagePolicyTypeParameterKind::Const(_)
    ));
    assert!(matches!(
        inspect.type_parameters()[2].kind(),
        PackagePolicyTypeParameterKind::Machine(_)
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

#[test]
fn fixed_token_binding_joins_the_callable_identity_and_only_when_present() {
    // A fixed operator token is part of a declaration's public signature:
    // adding, removing, or changing it is a breaking revision, so two
    // revisions that differ only in the binding must not review identically.
    // A tokenless declaration keeps the coordinate grammar it always had, so
    // retained locks see no churn.
    let tokenless = r#"
pub data Wrapped { value: u8; }
pub machine Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
    (left.value as u64) + (right.value as u64)
}
"#;
    let bound = tokenless.replace("pub machine Wrapped::add", "pub machine + Wrapped::add");
    let rebound = tokenless.replace("pub machine Wrapped::add", "pub machine * Wrapped::add");
    let unbound = project(&Fixture::local(tokenless));
    let bound = project(&Fixture::local(&bound));
    let rebound = project(&Fixture::local(&rebound));
    let unbound_callable = callable(&unbound, "Wrapped::add");
    let bound_callable = callable(&bound, "Wrapped::add");
    let rebound_callable = callable(&rebound, "Wrapped::add");
    assert_eq!(unbound_callable.parameters(), bound_callable.parameters());
    assert_eq!(unbound_callable.return_type(), bound_callable.return_type());
    assert_ne!(unbound_callable.identity(), bound_callable.identity());
    assert_ne!(bound_callable.identity(), rebound_callable.identity());
    assert_ne!(
        unbound.canonical_bytes().unwrap(),
        bound.canonical_bytes().unwrap()
    );
    assert!(bound_callable.identity().path().contains("token-bound"));
    assert!(bound_callable.identity().path().contains("1:+"));
    assert!(!unbound_callable.identity().path().contains("token-bound"));
    // The tokenless coordinate grammar is pinned byte-for-byte: this is the
    // path retained locks recorded before token bindings joined the identity.
    assert_eq!(
        unbound_callable.identity().path(),
        "20:conformance-callable12:Wrapped::add221:named-callable(path(Wrapped::add),parameters(parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,named\\(name\\(Wrapped\\)\\)\\)\\,parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,named\\(name\\(Wrapped\\)\\)\\)),result-dispatch())12:Wrapped::add7:machine188:9:parameter153:14:signature-type125:named(name(nominal\\(package-owner\\(32:2929292929292929292929292929292929292929292929292929292929292929\\)\\,path\\(Wrapped\\)\\)))5:named17:false:false:false188:9:parameter153:14:signature-type125:named(name(nominal\\(package-owner\\(32:2929292929292929292929292929292929292929292929292929292929292929\\)\\,path\\(Wrapped\\)\\)))5:named17:false:false:false85:11:result-type68:14:signature-type41:named(name(compiler-type\\(atom\\(u64\\)\\)))5:named"
    );
    assert_eq!(unbound, project(&Fixture::local(tokenless)));
}
