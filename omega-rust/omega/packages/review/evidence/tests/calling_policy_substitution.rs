//! Checked inherited substitutions use the same semantic identity as concrete types.

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
    .expect("calling substitution fixture checks");
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
fn inherited_array_type_matches_concrete_semantic_parameter() {
    let inherited = policy(
        r#"
boundary trait ProcedureBase<Value> {
    machine call(message: &Value) -> u64;
}

boundary trait HookProcedure: ProcedureBase<[u8; 7]> + Calling<HookProcedurePolicy> {}
"#,
    );
    let concrete = policy(
        r#"
boundary trait HookProcedure: Calling<HookProcedurePolicy> {
    machine call(message: &[u8; 7]) -> u64;
}
"#,
    );
    assert_eq!(
        inherited.semantic_parameters(),
        concrete.semantic_parameters()
    );
    assert_eq!(inherited.semantic_result(), concrete.semantic_result());
}

#[test]
fn inherited_nested_array_argument_uses_ancestor_type_application() {
    let declaration = r#"
boundary trait ProcedureBase<Value> {
    machine call(message: &Value) -> u64;
}
boundary trait ProcedureMiddle<Element>: ProcedureBase<[Element; 7]> {}
boundary trait HookProcedure: ProcedureMiddle<u8> + Calling<HookProcedurePolicy> {}
"#;
    let inherited = policy(declaration);
    let concrete =
        policy(&declaration.replace("ProcedureBase<[Element; 7]>", "ProcedureBase<[u8; 7]>"));
    assert_eq!(
        inherited.requirement_arguments(),
        concrete.requirement_arguments()
    );
    assert_eq!(
        inherited.semantic_parameters(),
        concrete.semantic_parameters()
    );
}

#[test]
fn inherited_nested_static_contract_keeps_private_nominal_and_outer_telescope() {
    let declaration = r#"
trait Hidden { machine apply(value: u64) -> u64; }
boundary trait ProcedureBase<Value> {
    machine call<machine Work, Later>(message: u64) -> u64
    where machine Work<machine Nested>(value: Value) -> Value
    where machine Nested satisfies Hidden::apply;
    ;
}
boundary trait HookProcedure: ProcedureBase<u64> + Calling<HookProcedurePolicy> {}
"#;
    let inherited = policy(declaration);
    let concrete = policy(&declaration.replace("value: Value) -> Value", "value: u64) -> u64"));
    assert_eq!(inherited.static_parameters(), concrete.static_parameters());
    assert_eq!(inherited.static_parameters().len(), 2);
}

#[test]
fn nested_static_lifetime_shadows_method_binder_without_collapsing_ordinals() {
    let declaration = r#"
boundary trait HookProcedure: Calling<HookProcedurePolicy> {
    machine call<'a, machine Work>(message: u64) -> u64
    where machine Work<'a>(value: &'a u64) -> u64;
    ;
}
"#;
    let shadowed = policy(declaration);
    let renamed = policy(&declaration.replace("Work<'a>(value: &'a", "Work<'b>(value: &'b"));
    let outer = policy(&declaration.replace("Work<'a>(value: &'a", "Work<'b>(value: &'a"));
    assert_eq!(shadowed.static_parameters(), renamed.static_parameters());
    assert_ne!(shadowed.static_parameters(), outer.static_parameters());
}
