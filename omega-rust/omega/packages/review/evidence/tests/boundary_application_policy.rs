//! Receipt-free D29 relationships derived from checked compiler consumers.
mod support;

use omega_package_evidence::record::*;
use omega_package_evidence::{
    project_checked_boundary_application_policy, project_checked_callable_policy,
    project_checked_selected_provider_policy,
};
use omega_target::TargetProfile;
use support::*;

fn compile(source: &str) -> (TempPackage, CheckedCompilation) {
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
    .unwrap_or_else(|diagnostics| panic!("D29 source should check: {diagnostics:#?}"));
    (package, checked)
}

fn project_source(source: &str) -> PackagePolicyBoundaryApplications {
    let (_package, checked) = compile(source);
    project(&checked)
}

fn project(checked: &CheckedCompilation) -> PackagePolicyBoundaryApplications {
    project_checked_boundary_application_policy(
        checked,
        TargetProfile::WindowsX64,
        package_identity(),
    )
    .expect("exact receipt-free D29 policy")
}

#[test]
fn symbolic_demand_producer_joins_full_callable_identity_and_exact_binders() {
    let source = r#"pub data Math {}
pub boundary operator Math::same<Value>(left: Value, right: Value) -> bool;
pub machine compare<Element>(left: Element, right: Element) -> bool { Math::same(left, right) }
"#;
    let (_package, checked) = compile(source);
    let policy = project(&checked);
    let [demand] = policy.demands() else {
        panic!("one open producer demand")
    };
    assert!(policy.realizations().is_empty());
    let callables =
        project_checked_callable_policy(&checked, TargetProfile::WindowsX64, package_identity())
            .unwrap();
    let producers = callables
        .callables()
        .iter()
        .filter(|callable| callable.role() == PackagePolicyCallableRole::Public)
        .collect::<Vec<_>>();
    let [producer] = producers.as_slice() else {
        panic!("one public producer")
    };
    assert_eq!(demand.producer_callable(), producer.identity());
    assert_eq!(
        demand.arguments(),
        &[
            PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                requirement_binder_ordinal: 0,
                producer_binder_ordinal: 0
            }
        ]
    );
    assert_eq!(
        policy,
        project_source(
            &source
                .replace("Element", "Item")
                .replace("Value", "Operand")
        )
    );
    let mut stale = checked.clone();
    let psi_checked_trees::CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder {
        machine_binder_ordinal,
        ..
    } = &mut stale.facts.operators.symbolic_boundary_applications[0].arguments[0];
    *machine_binder_ordinal = 99;
    assert!(
        project_checked_boundary_application_policy(
            &stale,
            TargetProfile::WindowsX64,
            package_identity()
        )
        .is_err()
    );
}

const CLOSED: &str = r#"pub data GenericMath {}
pub boundary operator GenericMath::identity<Element>(value: Element) -> Element;
pub data GenericProvider {}
pub machine GenericProvider::identity<Value>(value: Value) -> Value satisfies GenericMath::identity { value }
pub machine first(value: i32) -> i32 { GenericMath::identity(value) }
pub machine second(value: u64) -> u64 { GenericMath::identity(value) }
pub data ScalarMath {}
pub boundary operator ScalarMath::identity(value: i32) -> i32;
pub data ScalarProvider {}
pub machine ScalarProvider::identity(value: i32) -> i32 satisfies ScalarMath::identity { value }
pub machine scalar(value: i32) -> i32 { ScalarMath::identity(value) }
"#;

#[test]
fn generic_operator_binder_renaming_preserves_selected_rows_and_closed_applications() {
    let (_package, checked) = compile(CLOSED);
    let (_renamed_package, renamed) = compile(
        &CLOSED
            .replace("Element", "Item")
            .replace("Value", "Operand"),
    );
    let providers = |compilation: &CheckedCompilation| {
        project_checked_selected_provider_policy(
            compilation,
            TargetProfile::WindowsX64,
            package_identity(),
        )
        .unwrap()
    };
    let first = providers(&checked);
    let second = providers(&renamed);
    assert_eq!(first, second);
    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    assert_eq!(project(&checked), project(&renamed));
}

#[test]
fn closed_roles_rejoin_canonical_selected_plans_and_authored_templates() {
    let (_package, checked) = compile(CLOSED);
    let policy = project(&checked);
    let providers = project_checked_selected_provider_policy(
        &checked,
        TargetProfile::WindowsX64,
        package_identity(),
    )
    .unwrap();
    assert_eq!(policy.realizations().len(), 3);
    let mut specialized = 0;
    let mut nongeneric = 0;
    for application in policy.realizations() {
        let plan = &providers.plans()[application.selected_plan_index() as usize];
        assert_eq!(
            plan.schema_declaration(),
            application.operator_coordinate().identity()
        );
        let [row] = plan.rows() else {
            panic!("one exact realization")
        };
        assert_eq!(row.requirement().path(), application.requirement_identity());
        match application.realization() {
            PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                declaration,
                template,
            } => {
                specialized += 1;
                assert_eq!(declaration, row.realization());
                assert_eq!(declaration.path(), "GenericProvider::identity");
                assert_eq!(declaration.owner(), template.owner());
                assert!(
                    matches!(application.application(), PackageReviewBoundaryApplication::Exact(arguments) if arguments.len() == 1)
                );
            }
            PackagePolicyBoundaryRealization::NongenericCheckedBody {
                declaration,
                realization,
            } => {
                nongeneric += 1;
                assert_eq!(declaration, row.realization());
                assert_eq!(declaration.owner(), realization.owner());
                assert_eq!(
                    application.application(),
                    &PackageReviewBoundaryApplication::Empty
                );
            }
            PackagePolicyBoundaryRealization::ExactCompilerIntrinsic { .. } => {
                panic!("checked fixtures select no intrinsic")
            }
        }
    }
    assert_eq!((specialized, nongeneric), (2, 1));
    let changed = CLOSED.replace("{ value }", "{ transition { _ -> value } }");
    assert_eq!(
        policy,
        project_source(&changed),
        "private state lowering must not become application policy identity"
    );
}

#[test]
fn exact_intrinsic_application_retains_closed_execution_not_a_report_digest() {
    let (_package, checked) = compile(
        r#"pub data F32 {}
pub boundary operator F32::negate(value: f32) -> f32;
pub data FloatProvider {}
pub machine FloatProvider::negate(value: f32) -> f32 satisfies F32::negate via Binding::CompilerIntrinsic;
machine exercise() { let negative: f32 = F32::negate(1.0f32); }
"#,
    );
    let policy = project(&checked);
    let [application] = policy.realizations() else {
        panic!("one intrinsic application")
    };
    assert_eq!(
        application.realization(),
        &PackagePolicyBoundaryRealization::ExactCompilerIntrinsic {
            execution: PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(
                psi_numerics::literals::FloatFormat::F32
            )
        }
    );
    assert_eq!(
        application.application(),
        &PackageReviewBoundaryApplication::Empty
    );
}
