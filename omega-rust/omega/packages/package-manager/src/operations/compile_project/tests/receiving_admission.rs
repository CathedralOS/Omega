//! Explicit receiver-admission leg for accepted package production.
//!
//! The receiving permission policy is the second terminal-authority axis:
//! `compile_prepared_local_project_for_native` accepts it as `Option`, and an
//! absent policy makes no receiver-admission claim, so the emitted artifact
//! can never satisfy an explicit admission replay. This leg exercises that
//! contract end to end on one accepted customer whose demanded closure is a
//! package-owned `Console::exit_process` compiler-intrinsic leaf: the artifact
//! emits without any receiving policy, the same program rejects under every
//! denying policy — explicit empty, insufficient dispositions, or rows keyed
//! to foreign coordinates —
//! and admits any independently supplied policy covering the demanded leaves.
//! Package rows for unused operations do not become receiver obligations.
//! The emitted artifact binds and replays the supplied policy identity exactly.

use super::super::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native,
};
use super::{TemporaryProject, accepted_lock::accept_project};
use crate::operations::{LocalProjectPreparationOptions, prepare_local_project};
use std::collections::BTreeSet;
use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyRow,
    current_terminal_authority_permission_policy, current_terminal_authority_policy,
    terminal_authority_permission_policy_with_rows,
};

fn compile_with_receiving_policy(
    project: &TemporaryProject,
    target: target::TargetProfile,
    policy: Option<TerminalAuthorityPermissionPolicy>,
) -> Result<compiler::CompileReport, CompilePreparedLocalProjectNativeError> {
    let prepared = prepare_local_project(
        &project.entry(),
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .expect("prepare console project")
    .expect("application project");
    let mut request = PreparedLocalProjectNativeRequest::new(
        prepared,
        project.workspace.join("receiving-native"),
        target,
    );
    if let Some(policy) = policy {
        request = request.with_receiving_terminal_authority_permission_policy(policy);
    }
    compile_prepared_local_project_for_native(request, |_| ()).map(|(report, ())| report)
}

#[test]
fn unclassified_foreign_call_produces_but_cannot_claim_receiver_admission() {
    use terminal_psi_to_abstract_operations::provider_admission::{
        TerminalAuthorityPolicyRow, terminal_authority_policy_with_rows,
    };

    let project = TemporaryProject::new();
    let target = target::TargetProfile::WindowsX64;
    std::fs::write(
        project.source.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    Build::application(builder, "unclassified_foreign");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.entry(),
        r#"
use omega::language::core::external_binding;
boundary trait ProcessControl {}
data NativeExit {}
boundary requirement NativeExit::finish(code: u32)
requires code == 42
reaches ProcessControl;
windows_x86_64 machine exit_binding() -> ForeignBinding<12, 11, 0> {
    ForeignBinding::DllImport {
        import: DllImport::PeByName { library: "kernel32.dll", export: "ExitProcess" },
    }
}
windows_x86_64 machine NativeExit::finish_leaf(code: u32)
satisfies NativeExit::finish via exit_binding();
data Main {}
machine Main::main(&mut self) reaches ProcessControl {
    NativeExit::finish(42);
}
"#,
    )
    .unwrap();
    accept_project(&project, target);
    let compile = |physical_policy, permissions| {
        let prepared = prepare_local_project(
            &project.entry(),
            LocalProjectPreparationOptions {
                target,
                offline: false,
            },
        )
        .unwrap()
        .unwrap();
        let mut request = PreparedLocalProjectNativeRequest::new(
            prepared,
            project.workspace.join("foreign-native"),
            target,
        )
        .with_terminal_authority_policy(physical_policy);
        if let Some(permissions) = permissions {
            request = request.with_receiving_terminal_authority_permission_policy(permissions);
        }
        compile_prepared_local_project_for_native(request, |_| ()).map(|(report, ())| report)
    };
    let unclaimed = compile(current_terminal_authority_policy(), None)
        .expect("accepted import produces without a receiver's classification");
    let artifact = unclaimed.retained_native_artifact().unwrap();
    artifact.validate().unwrap();
    let receipt = artifact.terminal_authority_closure_review();
    let [leaf] = receipt.leaves() else {
        panic!("one demanded import")
    };
    assert!(matches!(
        leaf.mechanism(),
        effects::TerminalMechanismIdentity::NormalizedForeign(_)
    ));
    assert_eq!(leaf.exercised(), None);
    assert_eq!(leaf.permitted(), None);
    assert_eq!(receipt.permission_policy(), None);

    let disposition = effects::TerminalAuthorityDisposition::from_classes([
        effects::TerminalAuthorityClass::ProcessTermination,
    ]);
    let permissions = terminal_authority_permission_policy_with_rows(vec![
        TerminalAuthorityPermissionPolicyRow::new(
            leaf.service_schema(),
            leaf.requirement_identity(),
            disposition.clone(),
        ),
    ])
    .unwrap();
    let rejected = compile(
        current_terminal_authority_policy(),
        Some(permissions.clone()),
    )
    .expect_err("permission cannot substitute for classification");
    assert!(
        format!("{rejected:?}").contains("does not classify"),
        "{rejected:?}"
    );

    let physical_policy =
        terminal_authority_policy_with_rows(vec![TerminalAuthorityPolicyRow::new(
            leaf.mechanism(),
            disposition.clone(),
        )])
        .unwrap();
    let admitted = compile(physical_policy.clone(), Some(permissions.clone()))
        .expect("independent complete receiver policies admit the same import");
    let artifact = admitted.retained_native_artifact().unwrap();
    let receipt = artifact.terminal_authority_closure_review();
    assert_eq!(receipt.leaves()[0].exercised(), Some(&disposition));
    artifact
        .validate_for_terminal_authority_policies(
            physical_policy.identity(),
            permissions.identity(),
            receipt.identity(),
        )
        .unwrap();
    if target::TargetProfile::host_if_supported() == Some(target) {
        assert_native_exit(admitted, &project, 42);
    }
}

fn assert_native_exit(report: compiler::CompileReport, project: &TemporaryProject, expected: i32) {
    let published = compilation_products::publication::publish_compilation(
        report,
        &project.workspace.join("receiver-executable"),
    )
    .expect("publish the independently admitted program");
    let output = std::process::Command::new(published.checked_native_executable_path().unwrap())
        .output()
        .expect("execute the admitted program");
    assert_eq!(output.status.code(), Some(expected), "{output:?}");
}

#[test]
fn accepted_console_customer_receives_admission_only_under_a_sufficient_policy() {
    let host = target::TargetProfile::host_if_supported();
    // Source-inferred Console intrinsics are a Unix-hosted supply route.
    // Windows exercises its explicit DLL supply and native execution above.
    let target = host
        .filter(|profile| {
            matches!(
                profile,
                target::TargetProfile::LinuxX64
                    | target::TargetProfile::LinuxArm64
                    | target::TargetProfile::MacosArm64
            )
        })
        .unwrap_or(target::TargetProfile::LinuxX64);
    let project = TemporaryProject::console_project(target);
    let accepted = accept_project(&project, target);
    let package_permissions = accepted
        .target(target)
        .unwrap()
        .occurrences()
        .iter()
        .flat_map(|occurrence| occurrence.acceptance().rows())
        .filter(|row| {
            row.kind() == package_evidence::record::PackagePolicyRowKind::TerminalPermission
        })
        .collect::<Vec<_>>();
    assert!(
        package_permissions
            .iter()
            .any(|row| row.canonical_text().contains("write_byte")),
        "the accepted package includes the unused output operation"
    );
    let accepted_lock = std::fs::read(project.source.join("omega.lock")).unwrap();

    // Absent receiving policy: the customer emits, and the artifact records
    // no receiver-admission claim.
    let unclaimed = compile_with_receiving_policy(&project, target, None)
        .expect("accepted console customer emits without a receiving policy");
    assert_eq!(
        unclaimed.output_kind(),
        compiler::CompileOutputKind::RetainedNativeArtifact
    );
    let artifact = unclaimed.retained_native_artifact().unwrap();
    artifact.validate().unwrap();
    assert!(
        artifact
            .terminal_authority_permission_policy_identity()
            .is_none()
    );
    let receipt = artifact.terminal_authority_closure_review();
    receipt.validate().unwrap();
    assert!(
        !receipt.leaves().is_empty(),
        "the console customer demands terminal closure leaves"
    );
    // An unclaimed artifact cannot satisfy an explicit admission replay for
    // any receiving policy.
    assert!(
        artifact
            .validate_for_terminal_authority_policies(
                artifact.terminal_authority_policy_identity(),
                current_terminal_authority_permission_policy().identity(),
                receipt.identity(),
            )
            .is_err(),
        "an unclaimed artifact must not satisfy explicit admission replay"
    );

    // The receiving policy that suffices is exact: one row per demanded
    // closure leaf, permitting the classes each leaf exercises.
    let mut coordinates = BTreeSet::new();
    let rows = receipt
        .leaves()
        .iter()
        .filter(|leaf| {
            coordinates.insert((
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
            ))
        })
        .map(|leaf| {
            TerminalAuthorityPermissionPolicyRow::new(
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
                leaf.exercised().expect("known console mechanism").clone(),
            )
        })
        .collect::<Vec<_>>();
    let sufficient = terminal_authority_permission_policy_with_rows(rows)
        .expect("leaf-derived receiving policy is well formed");
    assert!(
        sufficient
            .rows()
            .iter()
            .all(|row| !row.requirement_identity().contains("write_byte")),
        "only demanded operations belong in the receiving policy"
    );

    // The canonical empty receiving policy denies: it grants no demanded leaf.
    let denied = compile_with_receiving_policy(
        &project,
        target,
        Some(current_terminal_authority_permission_policy()),
    );
    assert!(
        matches!(
            denied,
            Err(CompilePreparedLocalProjectNativeError::Native(_))
        ),
        "an explicit empty receiving policy must reject a demanding program"
    );

    // A policy holding a row at the same coordinate with a disposition that
    // does not cover the leaf's exercised classes cannot admit the program.
    let substituted = receipt
        .leaves()
        .iter()
        .map(|leaf| {
            TerminalAuthorityPermissionPolicyRow::new(
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
                effects::TerminalAuthorityDisposition::from_classes([]),
            )
        })
        .collect::<Vec<_>>();
    let substituted = terminal_authority_permission_policy_with_rows(substituted)
        .expect("substituted receiving policy is well formed");
    let denied = compile_with_receiving_policy(&project, target, Some(substituted));
    assert!(
        matches!(
            denied,
            Err(CompilePreparedLocalProjectNativeError::Native(_))
        ),
        "a receiving policy substituting a narrower permission must reject"
    );

    // A row keyed beside the demanded coordinate forges no grant: the leaf
    // still finds no exact row for its schema and requirement.
    let forged = terminal_authority_permission_policy_with_rows(vec![
        TerminalAuthorityPermissionPolicyRow::new(
            effects::provider_plan::ServiceSchemaDigest::from_digest([0xee; 32]),
            "console::Console::exit_process#forged",
            effects::TerminalAuthorityDisposition::from_classes([
                effects::TerminalAuthorityClass::ProcessTermination,
            ]),
        ),
    ])
    .expect("forged-coordinate receiving policy is well formed");
    assert!(
        matches!(
            compile_with_receiving_policy(&project, target, Some(forged)),
            Err(CompilePreparedLocalProjectNativeError::Native(_))
        ),
        "a receiving policy keyed to foreign coordinates must reject"
    );

    // The policy covering every demanded leaf admits: the artifact records
    // that policy's identity and replays the recorded admission exactly.
    let admitted = compile_with_receiving_policy(&project, target, Some(sufficient.clone()))
        .expect("the sufficient receiving policy admits the same program");
    let artifact = admitted.retained_native_artifact().unwrap();
    artifact.validate().unwrap();
    assert_eq!(
        artifact.terminal_authority_permission_policy_identity(),
        Some(sufficient.identity()),
    );
    let receipt = artifact.terminal_authority_closure_review();
    artifact
        .validate_for_terminal_authority_policies(
            current_terminal_authority_policy().identity(),
            sufficient.identity(),
            receipt.identity(),
        )
        .expect("the admitted artifact replays its receiver admission");
    // A receiver accepting a different permission policy than the artifact
    // recorded rejects the replay.
    assert!(
        artifact
            .validate_for_terminal_authority_policies(
                current_terminal_authority_policy().identity(),
                current_terminal_authority_permission_policy().identity(),
                receipt.identity(),
            )
            .is_err(),
        "replaying under a different permission policy must reject"
    );

    // Receiver permission is independent of package acceptance. A wider
    // receiving row still covers the demanded mechanism without modifying
    // the package's accepted row.
    let mut widened_coordinates = BTreeSet::new();
    let widened = receipt
        .leaves()
        .iter()
        .filter(|leaf| {
            widened_coordinates.insert((
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
            ))
        })
        .map(|leaf| {
            let mut classes = leaf
                .exercised()
                .expect("known console mechanism")
                .classes()
                .to_vec();
            classes.push(effects::TerminalAuthorityClass::ProcessInput);
            TerminalAuthorityPermissionPolicyRow::new(
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
                effects::TerminalAuthorityDisposition::from_classes(classes),
            )
        })
        .collect::<Vec<_>>();
    let widened = terminal_authority_permission_policy_with_rows(widened)
        .expect("widened receiving policy is well formed");
    assert_ne!(
        widened.identity(),
        sufficient.identity(),
        "the wider grant is a distinct supplied policy"
    );
    let admitted = compile_with_receiving_policy(&project, target, Some(widened.clone()))
        .expect("independent receiver permission may exceed the package row");
    assert_eq!(
        admitted
            .retained_native_artifact()
            .unwrap()
            .terminal_authority_permission_policy_identity(),
        Some(widened.identity()),
    );
    if host == Some(target) {
        assert_native_exit(admitted, &project, 70);
    } else {
        eprintln!(
            "Console receiver-policy checks compiled for {target:?}; native execution needs that host"
        );
    }

    // Surplus rows beside the demanded coordinates admit unchanged: the
    // admitted identity is the supplied policy itself, not a canonicalized
    // demanded subset.
    let mut surplus_rows = sufficient.rows().to_vec();
    surplus_rows.push(TerminalAuthorityPermissionPolicyRow::new(
        effects::provider_plan::ServiceSchemaDigest::from_digest([0x44; 32]),
        "console::Console::read_byte#surplus",
        effects::TerminalAuthorityDisposition::from_classes([
            effects::TerminalAuthorityClass::ProcessInput,
        ]),
    ));
    let surplus = terminal_authority_permission_policy_with_rows(surplus_rows)
        .expect("surplus receiving policy is well formed");
    let admitted = compile_with_receiving_policy(&project, target, Some(surplus.clone()))
        .expect("a surplus grant beside sufficient rows still admits");
    let artifact = admitted.retained_native_artifact().unwrap();
    assert_eq!(
        artifact.terminal_authority_permission_policy_identity(),
        Some(surplus.identity()),
    );
    artifact
        .validate_for_terminal_authority_policies(
            current_terminal_authority_policy().identity(),
            surplus.identity(),
            artifact.terminal_authority_closure_review().identity(),
        )
        .expect("the surplus-grant artifact replays its receiver admission");
    assert_eq!(
        std::fs::read(project.source.join("omega.lock")).unwrap(),
        accepted_lock,
        "receiver policies never rewrite package acceptance"
    );
}
