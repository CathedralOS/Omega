//! Lossless external policy is captured from the exact checked source owner.

mod support;

use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::project_checked_external_supply_policy;
use package_evidence::record::*;
use support::*;

fn project(source: &str) -> PackagePolicyExternalExecutableSupply {
    project_with_foreign(source, None)
}

fn project_with_foreign(
    source: &str,
    foreign_owner: Option<PackageKeyIdentity>,
) -> PackagePolicyExternalExecutableSupply {
    let package = TempPackage::new();
    let dependency = TempPackage::new();
    package.write("main.omg", source);
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let inputs = if let Some(owner) = foreign_owner {
        dependency.write(
            "api.omg",
            "pub machine permitted(flag: bool) -> bool terminates; { flag }\n",
        );
        PackageCompilationInputs::new_package(
            package_identity(),
            vec![
                PackageSourceBinding::new(package_identity(), "review-fixture", package.0.clone()),
                PackageSourceBinding::new(owner, "dependency-package", dependency.0.clone()),
            ],
            vec![PackageDependencyBinding::new(
                package_identity(),
                "dependency",
                owner,
            )],
        )
        .unwrap()
    } else {
        package_inputs(&package.0)
    };
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target::TargetProfile::WindowsX64.target_name()),
        inputs,
    )
    .unwrap_or_else(|diagnostics| panic!("lossless external source checks: {diagnostics:#?}"));
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Provider::perform")
        .expect("exact external realization");
    let mut rows = project_checked_external_supply_policy(&checked, machine.symbol)
        .expect("capture checked external source without native emission");
    assert_eq!(rows.len(), 1);
    let policy = rows.remove(0);
    let bytes = policy.canonical_bytes().unwrap();
    let recovered = PackagePolicyExternalExecutableSupply::recover_canonical(
        &bytes,
        PackagePolicyRecoveryLimits::default(),
    )
    .expect("lossless external policy recovers without source");
    assert_eq!(recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    policy
}

#[test]
fn private_external_nested_static_crash_guards_keep_exact_foreign_owner() {
    let source = r#"
use dependency::api;
pub boundary trait Host {
    machine perform<machine Work>()
    where machine Work(flag: bool) crashes Trap permitted(flag);
    ;
}
data Provider {}
machine Provider::perform<machine Work>()
where machine Work(flag: bool) crashes Trap permitted(flag);
satisfies Host::perform via Binding::DllImport("policy-fixture", "perform");
"#;
    let first = project_with_foreign(
        source,
        Some(PackageKeyIdentity::from_digest([42; 32]).unwrap()),
    );
    let second = project_with_foreign(
        source,
        Some(PackageKeyIdentity::from_digest([43; 32]).unwrap()),
    );
    assert_eq!(first.callable(), second.callable());
    let PackagePolicyTypeParameterKind::Machine(PackagePolicyMachineParameterContract::Structural(
        signature,
    )) = first.signature().static_parameters()[0].kind()
    else {
        panic!("structural machine parameter")
    };
    assert_eq!(signature.published_crash().len(), 1);
    assert_ne!(
        first.signature().static_parameters(),
        second.signature().static_parameters()
    );
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}

#[test]
fn absent_and_explicit_empty_results_remain_distinct_at_both_external_signatures() {
    let source = r#"
pub data Unit {}
pub data Surface {}
pub data Provider {}
pub boundary requirement Surface::perform();
pub machine Provider::perform() satisfies Surface::perform
via Binding::DllImport("policy-fixture", "perform");
"#;
    let absent = project(source);
    let explicit = project(&source.replace("perform()", "perform() -> Unit"));
    assert!(absent.signature().return_type().is_none());
    assert!(explicit.signature().return_type().is_some());
    let PackagePolicyExternalRequirement::TopLevelRequirement {
        signature: absent_requirement,
        ..
    } = absent.requirement()
    else {
        panic!("top-level requirement")
    };
    let PackagePolicyExternalRequirement::TopLevelRequirement {
        signature: explicit_requirement,
        ..
    } = explicit.requirement()
    else {
        panic!("top-level requirement")
    };
    assert!(absent_requirement.return_type().is_none());
    assert!(explicit_requirement.return_type().is_some());
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        explicit.canonical_bytes().unwrap()
    );
}

#[test]
fn external_trait_application_retains_actual_caller_lifetime_not_only_partition() {
    let source = r#"
pub boundary trait LifetimeSlot<'scope> { machine perform(value: u64) -> u64; }
pub data Provider {}
pub machine Provider::perform<'left, 'right>(value: u64) -> u64
satisfies LifetimeSlot<'left>::perform
via Binding::DllImport("policy-fixture", "perform");
"#;
    let first = project(source);
    let second = project(&source.replace("LifetimeSlot<'left>", "LifetimeSlot<'right>"));
    let PackagePolicyExternalRequirement::Trait(first_application) = first.requirement() else {
        panic!("trait application")
    };
    let PackagePolicyExternalRequirement::Trait(second_application) = second.requirement() else {
        panic!("trait application")
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
    assert_eq!(
        first,
        project(
            &source
                .replace("'left", "'first")
                .replace("'right", "'second")
                .replace("'scope", "'view")
        )
    );
}

#[test]
fn external_nested_static_machine_result_absence_is_not_unit() {
    let source = r#"
pub data Unit {}
pub boundary trait Host {
    machine perform<machine Work>() where machine Work(); ;
}
pub data Provider {}
pub machine Provider::perform<machine Work>()
where machine Work();
satisfies Host::perform via Binding::DllImport("policy-fixture", "perform");
"#;
    let absent = project(source);
    let explicit = project(&source.replace("machine Work()", "machine Work() -> Unit"));
    let result = |policy: &PackagePolicyExternalExecutableSupply| {
        let PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::Structural(signature),
        ) = policy.signature().static_parameters()[0].kind()
        else {
            panic!("structural machine contract")
        };
        signature.return_type().cloned()
    };
    assert!(result(&absent).is_none());
    assert!(result(&explicit).is_some());
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        explicit.canonical_bytes().unwrap()
    );
}
