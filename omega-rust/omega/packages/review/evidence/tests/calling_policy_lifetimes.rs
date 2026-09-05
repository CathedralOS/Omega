//! Calling lifetime applications remain relative to their containing telescope.

mod support;

use omega_package_evidence::encoding::PackagePolicyRecoveryLimits;
use omega_package_evidence::{project_checked_calling_policy, record::PackagePolicyCallingPlan};
use support::*;

fn policy(declaration: &str) -> PackagePolicyCallingPlan {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap();
    let fixture = fs::read_to_string(
        repository.join("source/library/std/tests/direct_callback_parameter.omg"),
    )
    .unwrap();
    let prefix = fixture
        .split_once("boundary trait HookProcedure:")
        .unwrap()
        .0;
    let package = TempPackage::new();
    package.write("main.omg", &format!("{prefix}\n{declaration}"));
    package.write(
        "calling.omg",
        &fs::read_to_string(repository.join("source/library/std/calling.omg")).unwrap(),
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("calling lifetime fixture checks");
    let owner = checked
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "HookProcedure")
        .unwrap();
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| realization.boundary_trait == owner.symbol)
        .unwrap();
    let policy = project_checked_calling_policy(&checked, realization).unwrap();
    let bytes = policy.canonical_bytes().unwrap();
    assert_eq!(
        PackagePolicyCallingPlan::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .unwrap(),
        policy
    );
    policy
}

#[test]
fn inherited_lifetime_application_keeps_root_ordinal() {
    let declaration = r#"
boundary trait ProcedureBase<'scope> {
    machine call(message: &'scope u64) -> u64;
}
boundary trait HookProcedure<'first, 'second>:
    ProcedureBase<'second> + Calling<HookProcedurePolicy> {}
"#;
    let second = policy(declaration);
    let first = policy(&declaration.replace("ProcedureBase<'second>", "ProcedureBase<'first>"));
    assert_eq!(second.boundary_lifetime_parameter_count(), 2);
    assert_eq!(second.requirement_lifetime_arguments(), &[1]);
    assert_eq!(first.requirement_lifetime_arguments(), &[0]);
    assert_eq!(second.requirement_lifetime_parameter_count(), 0);
    assert_eq!(first.physical(), second.physical());
    assert_ne!(first.semantic_parameters(), second.semantic_parameters());
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}

#[test]
fn method_lifetime_is_separate_from_trait_lifetime() {
    let declaration = r#"
boundary trait HookProcedure<'outer>: Calling<HookProcedurePolicy> {
    machine call<'inner>(message: &'inner u64) -> u64;
}
"#;
    let inner = policy(declaration);
    let outer = policy(&declaration.replace("message: &'inner u64", "message: &'outer u64"));
    assert_eq!(inner.boundary_lifetime_parameter_count(), 1);
    assert_eq!(inner.requirement_lifetime_parameter_count(), 1);
    assert_eq!(inner.requirement_lifetime_arguments(), &[0]);
    assert_eq!(inner.physical(), outer.physical());
    assert_ne!(inner.semantic_parameters(), outer.semantic_parameters());
    assert_ne!(
        inner.canonical_bytes().unwrap(),
        outer.canonical_bytes().unwrap()
    );
}

#[test]
fn nested_static_lifetime_does_not_capture_an_unused_root_binder() {
    let declaration = r#"
boundary trait ProcedureBase<'scope> {
    machine call<machine Work>(message: u64) -> u64
    where machine Work<'unused>(value: &'unused u64) -> u64;
    ;
}
boundary trait HookProcedure<'unused, 'active>:
    ProcedureBase<'active> + Calling<HookProcedurePolicy> {}
"#;
    let shadowed = policy(declaration);
    let renamed = policy(&declaration.replace(
        "Work<'unused>(value: &'unused",
        "Work<'local>(value: &'local",
    ));
    assert_eq!(shadowed.requirement_lifetime_arguments(), &[1]);
    assert_eq!(shadowed.static_parameters(), renamed.static_parameters());
}

#[test]
fn recursively_nested_static_lifetimes_keep_each_scope() {
    let declaration = r#"
boundary trait HookProcedure: Calling<HookProcedurePolicy> {
    machine call<'scope, machine Work>(message: u64) -> u64
    where machine Work<'scope, machine Nested>(value: &'scope u64) -> u64
    where machine Nested<'scope>(value: &'scope u64) -> u64;
    ;
    ;
}
"#;
    let shadowed = policy(declaration);
    let renamed = policy(&declaration.replace(
        "Nested<'scope>(value: &'scope",
        "Nested<'inner>(value: &'inner",
    ));
    let outer = policy(&declaration.replace(
        "Nested<'scope>(value: &'scope",
        "Nested<'inner>(value: &'scope",
    ));
    assert_eq!(shadowed.static_parameters(), renamed.static_parameters());
    assert_ne!(shadowed.static_parameters(), outer.static_parameters());
}

#[test]
fn nested_static_result_absence_is_not_an_empty_nominal_result() {
    let declaration = r#"
data Unit {}
boundary trait HookProcedure: Calling<HookProcedurePolicy> {
    machine call<machine Work>(message: u64) -> u64
    where machine Work(value: u64);
    ;
}
"#;
    let absent = policy(declaration);
    let explicit = policy(&declaration.replace("Work(value: u64);", "Work(value: u64) -> Unit;"));
    use omega_package_evidence::record::PackagePolicyTypeParameterKind;
    let PackagePolicyTypeParameterKind::Machine(absent_contract) =
        absent.static_parameters()[0].kind()
    else {
        panic!("static machine contract")
    };
    let PackagePolicyTypeParameterKind::Machine(explicit_contract) =
        explicit.static_parameters()[0].kind()
    else {
        panic!("static machine contract")
    };
    let absent_signature = absent_contract.structural().unwrap();
    let explicit_signature = explicit_contract.structural().unwrap();
    assert!(absent_signature.return_type().is_none());
    assert!(explicit_signature.return_type().is_some());
    assert_eq!(absent.physical(), explicit.physical());
    assert_ne!(absent.static_parameters(), explicit.static_parameters());
}
