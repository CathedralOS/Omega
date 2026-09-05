mod support;

use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::project_checked_conformance_policy;
use package_evidence::record::PackagePolicyClosedConformanceApplication;
use support::*;
use typed_trees::expression::StaticMachineArgument;
use typed_trees_to_checked_trees::close_conformance_application;

fn compile_source(source: &str, digest: u8) -> (TempPackage, CheckedCompilation) {
    let package = TempPackage::new();
    package.write("main.omg", source);
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let identity = PackageKeyIdentity::from_digest([digest; 32]).unwrap();
    let inputs = PackageCompilationInputs::new_package(
        identity,
        vec![PackageSourceBinding::new(
            identity,
            "review-fixture",
            package.0.clone(),
        )],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        inputs,
    )
    .expect("source-derived conformance policy fixture should check");
    (package, checked)
}

fn assert_policy_round_trip(policy: &PackagePolicyClosedConformanceApplication) {
    let bytes = policy
        .canonical_bytes()
        .expect("encode exact conformance policy");
    let recovered = PackagePolicyClosedConformanceApplication::recover_canonical(
        &bytes,
        PackagePolicyRecoveryLimits::default(),
    )
    .expect("recover exact conformance policy without source or receipts");
    assert_eq!(&recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
}

#[test]
fn same_spelled_package_types_have_distinct_conformance_policy_arguments() {
    const SOURCE: &str = r#"
pub trait Ranked {}
pub data Card {}
pub data Wrapper<Value> { value: Value; }
pub FieldOrder<Element, Nested>: Element satisfies Ranked {}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 { 0 }
boundary machine trusted() -> u64
ensures result == tag<Card, FieldOrder<Card, Wrapper<Card>>>();
"#;
    let (_first_source, first) = compile_source(SOURCE, 0x61);
    let (_second_source, second) = compile_source(SOURCE, 0x62);
    let application = |checked: &CheckedCompilation| {
        let [occurrence] = checked
            .facts
            .proof
            .contract_expression_static_conformance_applications
            .as_slice()
        else {
            panic!("one source-checked generic conformance occurrence")
        };
        occurrence.application.clone()
    };
    let first_application = application(&first);
    let second_application = application(&second);
    assert_eq!(
        first_application.type_arguments,
        second_application.type_arguments
    );
    let first_policy = project_checked_conformance_policy(&first, &first_application, &[]).unwrap();
    let second_policy =
        project_checked_conformance_policy(&second, &second_application, &[]).unwrap();
    assert_eq!(first_policy.type_arguments().len(), 2);
    assert_eq!(second_policy.type_arguments().len(), 2);
    assert_ne!(
        first_policy.type_arguments()[0],
        second_policy.type_arguments()[0]
    );
    assert_ne!(
        first_policy.type_arguments()[1],
        second_policy.type_arguments()[1]
    );
    assert_ne!(first_policy.subject(), second_policy.subject());
    assert_ne!(
        first_policy.canonical_bytes().unwrap(),
        second_policy.canonical_bytes().unwrap()
    );
    assert_policy_round_trip(&first_policy);
    assert_policy_round_trip(&second_policy);
}

#[test]
fn private_named_callback_slot_projects_exact_policy_without_public_exposure() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .join("tests/omega/pass/layouts/private_callback_slot_demand_compile/main.omg");
    let (_source, checked) = compile_source(&fs::read_to_string(source).unwrap(), 0x63);
    let layout = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_name == "Spread<ForeignRecord>")
        .unwrap();
    let [demand] = layout.private_callback_demands.as_slice() else {
        panic!("one real evaluated named callback slot")
    };
    let declaration = checked
        .conformances()
        .iter()
        .find(|declaration| declaration.symbol == demand.slot_application.declaration)
        .unwrap();
    assert!(!declaration.is_public);
    let policy =
        project_checked_conformance_policy(&checked, &demand.slot_application, &[]).unwrap();
    assert_eq!(policy.declaration().path(), "WndClassWindowProcedureSlot");
    assert!(matches!(
        policy.declaration().owner(),
        PackageReviewNominalOwner::Package(_)
    ));
    assert!(policy.subject().is_some());
    assert_eq!(policy.trait_arguments().len(), 1);
    assert!(policy.rows().is_empty());
    assert_policy_round_trip(&policy);
}

#[test]
fn subjectless_checked_proof_witness_has_no_invented_policy_carrier() {
    let (_source, checked) = compile_source(
        r#"
trait Evidence<Value> {}
proposition ready<Value>() evidence Evidence<Value>;
ConcreteEvidence: satisfies Evidence<u64> {}
data Root {}
machine Root::produce()
ensures outgoing: ready<u64>()
{ outgoing = ConcreteEvidence; }
"#,
        0x64,
    );
    let declaration = checked
        .conformances()
        .iter()
        .find(|declaration| {
            declaration
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "ConcreteEvidence")
        })
        .unwrap();
    let selected = StaticMachineArgument {
        path: vec![declaration.alias.clone().unwrap()].into_boxed_slice(),
        application: None,
        const_literal: None,
        evidence_projection: None,
        symbol: declaration.symbol,
    };
    let application = close_conformance_application(&checked.typed, &selected).unwrap();
    assert!(application.subject_identity.is_none());
    let policy = project_checked_conformance_policy(&checked, &application, &[]).unwrap();
    assert_eq!(policy.declaration().path(), "ConcreteEvidence");
    assert!(policy.subject().is_none());
    assert_eq!(policy.trait_identity().path(), "Evidence");
    assert_eq!(policy.trait_arguments().len(), 1);
    assert!(policy.trait_arguments()[0].canonical().contains("u64"));
    assert!(policy.rows().is_empty());
    assert_policy_round_trip(&policy);
}
