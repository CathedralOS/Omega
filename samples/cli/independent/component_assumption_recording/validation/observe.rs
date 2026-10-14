//! Independent observations only: exact Build records under the repository
//! fixture policy; native/Terminal production under the actual project's policy.
use std::{collections::BTreeSet, path::Path};
#[path = "../../../../../omega-rust/omega/compilation/driver/tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

fn records(root: &Path, output: &Path) {
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        root,
        Some("linux_x86_64"),
    )
    .unwrap_or_else(|errors| panic!("fixture preparation: {errors:?}"));
    let mut request =
        compiler::CheckedCompileRequest::new(&prepared.root_path, Some("linux_x86_64"));
    request.package_inputs = prepared.package_inputs;
    request.build_snapshot =
        fixture_package_inputs::fixture_build_snapshot(&prepared.root_path).unwrap();
    request.build_dir = Some(output.join("record-check"));
    let checked = compiler::CompilationSession::default()
        .check(request)
        .result
        .unwrap_or_else(|errors| panic!("checked records: {errors:?}"));
    let expected: [u8; 32] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
        .repeat(4)
        .try_into()
        .unwrap();
    let rows = &checked.build().config.accepted_component_assumptions;
    if root.parent().unwrap().file_name().unwrap() == "component_assumption_user_declaration" {
        assert!(
            rows.is_empty(),
            "ordinary user call grants no assumption acceptance"
        );
        println!("same-named user declaration: zero accepted assumptions");
        return;
    }
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].digest, expected);
    assert_eq!(
        Some(rows[0].selecting_machine),
        checked.build().selected_build_machine_symbol
    );
    assert!(rows[0].source_span.span.start < rows[0].source_span.span.end);
    assert_eq!(
        checked
            .build()
            .config
            .accepted_component_assumption_digests(),
        BTreeSet::from([expected])
    );
    std::fs::write(output.join("recorded-digest.bin"), rows[0].digest).unwrap();
    std::fs::write(output.join("recorded-row.txt"), format!("{:?}\n", rows[0])).unwrap();
    println!(
        "fixture policy: exact 32-byte digest, one row, selected build and authored span agree"
    );
}

fn runtime(root: &Path, output: &Path) {
    use package_manager::operations::{
        LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
        compile_prepared_local_project_for_native, prepare_local_project,
    };
    let target = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let prepared = prepare_local_project(
        root,
        LocalProjectPreparationOptions {
            target,
            offline: true,
        },
    )
    .unwrap()
    .expect("package project");
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        output.join("product"),
        target,
    ))
    .unwrap();
    let artifact = report.retained_native_artifact().expect("native artifact");
    artifact.validate().unwrap();
    std::fs::write(
        output.join("terminal.psi"),
        artifact.psi_artifact().to_bytes(),
    )
    .unwrap();
    let native_path = output.join("captured-native");
    std::fs::write(&native_path, &artifact.image().output().bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&native_path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let native = std::process::Command::new(&native_path).output().unwrap();
    assert_eq!(native.status.code(), Some(70));
    assert!(native.stdout.is_empty() && native.stderr.is_empty());
    std::fs::write(output.join("native.exit"), b"70\n").unwrap();
    let result = compiler::runtime::execute(compiler::runtime::TerminalRuntimeRequest {
        provider_proposal: None,
        artifact: artifact.psi_artifact(),
        target: Some("linux_x86_64"),
        provisions: report.runtime_provisions(),
        fuel_allowance: 1_000_000,
        domain_exit: compiler::runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .unwrap();
    let terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(completion) =
        result.status
    else {
        panic!(
            "expected terminal process completion, got {:?}",
            result.status
        );
    };
    assert_eq!(completion.return_code, 70);
    assert!(result.stdout.is_empty() && result.stderr.is_empty());
    std::fs::write(output.join("terminal.exit"), b"70\n").unwrap();
    println!("captured Terminal: exact retained native artifact, exit 70, empty output");
}

fn component(root: &Path, output: &Path) {
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        root,
        Some("linux_x86_64"),
    )
    .unwrap();
    let mut request =
        compiler::CheckedCompileRequest::new(&prepared.root_path, Some("linux_x86_64"));
    request.package_inputs = prepared.package_inputs;
    request.build_snapshot =
        fixture_package_inputs::fixture_build_snapshot(&prepared.root_path).unwrap();
    request.build_dir = Some(output.join("component-inspection"));
    let artifact = compiler::CompilationSession::default()
        .produce(
            request,
            compiler::CompilationProduct::Inspection("emit".to_owned()),
        )
        .unwrap_or_else(|errors| panic!("component inspection: {errors:?}"))
        .into_inspection()
        .unwrap();
    let mut mechanisms = Vec::new();
    for machine in &artifact.semantic_module().machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                if let terminal_psi::OperationKind::PortWrite {
                    service,
                    port,
                    value,
                } = &operation.kind
                {
                    mechanisms.push(format!("{} {} {}", service.get(), port, value));
                }
            }
        }
    }
    assert_eq!(
        mechanisms,
        ["1 1016 65"],
        "the independent fixture policy permits this one physical mechanism"
    );
    std::fs::write(output.join("mechanism.txt"), mechanisms.join("\n")).unwrap();
    let (subject, bytes) = component_description::publish_component_description(
        component_description::ComponentDescriptionFacts {
            artifact: &artifact,
            selected_provider_plans: &effects::SelectedProviderPlanFacts::default(),
            component_progress: None,
            stack_demand: None,
            realization_identity: None,
        },
    )
    .unwrap();
    assert_eq!(
        subject,
        terminal_codec::terminal_psi_identity(artifact.semantic_module()).unwrap()
    );
    let description = component_description::decode_component_description(&bytes).unwrap();
    assert_eq!(description.assumptions.len(), 1);
    // Frozen independently with Python hashlib over domain || u64(service)
    // || u16(port) || u8(value), little endian. Never accept producer-chosen policy.
    let digest: [u8; 32] = [
        0x91, 0x1d, 0xf4, 0x06, 0xcf, 0x39, 0x28, 0xbc, 0x12, 0xf1, 0xdf, 0x49, 0x47, 0x2d, 0x75,
        0x5b, 0x4e, 0x66, 0xb2, 0x24, 0xbb, 0x36, 0xbf, 0x88, 0xfd, 0x18, 0xaa, 0x93, 0x99, 0x0b,
        0x71, 0x8e,
    ];
    assert_eq!(description.assumptions, [digest]);
    std::fs::write(output.join("component-description.bin"), &bytes).unwrap();
    std::fs::write(output.join("component-digest.bin"), digest).unwrap();
    let mut request = component_description::ComponentVerificationRequest {
        expected_subject: subject,
        accepted_schemas: BTreeSet::from([component_description::COMPONENT_DESCRIPTION_SCHEMA_V2]),
        accepted_assumptions: BTreeSet::from([digest]),
    };
    let verified = component_description::verify_component(&bytes, &request).unwrap();
    assert_eq!(verified.assumptions(), &[digest]);
    request.accepted_assumptions.clear();
    assert!(
        matches!(component_description::verify_component(&bytes, &request),
        Err(component_description::ComponentVerificationRejection::UnacceptedAssumption(actual)) if actual == digest)
    );
    let mut changed = digest;
    changed[0] ^= 1;
    request.accepted_assumptions.insert(changed);
    assert!(
        matches!(component_description::verify_component(&bytes, &request),
        Err(component_description::ComponentVerificationRejection::UnacceptedAssumption(actual)) if actual == digest)
    );
    for policy in ["accepted", "missing", "changed"] {
        let path = fixture_package_inputs::repo_root().join(format!(
            "tests/omega/pass/build/component_assumption_policy_{policy}/main.omg"
        ));
        let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &path,
            Some("linux_x86_64"),
        )
        .unwrap();
        let mut consumer =
            compiler::CheckedCompileRequest::new(&prepared.root_path, Some("linux_x86_64"));
        consumer.package_inputs = prepared.package_inputs;
        consumer.build_snapshot =
            fixture_package_inputs::fixture_build_snapshot(&prepared.root_path).unwrap();
        consumer.build_dir = Some(output.join(format!("consumer-{policy}")));
        let checked = compiler::CompilationSession::default()
            .check(consumer)
            .result
            .unwrap();
        request.accepted_assumptions = checked
            .build()
            .config
            .accepted_component_assumption_digests();
        let result = component_description::verify_component(&bytes, &request);
        if policy == "accepted" {
            assert_eq!(request.accepted_assumptions, BTreeSet::from([digest]));
            assert_eq!(result.unwrap().assumptions(), &[digest]);
        } else {
            assert!(matches!(result,
                Err(component_description::ComponentVerificationRejection::UnacceptedAssumption(actual)) if actual == digest));
        }
        println!("authored consumer {policy}: recorded policy verification agrees");
    }
    println!(
        "component: unchanged artifact accepted only by exact digest; missing/changed digests rejected"
    );
}

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    assert_eq!(arguments.len(), 4, "observe records|runtime ROOT OUTPUT");
    let root = Path::new(&arguments[2]);
    let output = Path::new(&arguments[3]);
    std::fs::create_dir_all(output).unwrap();
    match arguments[1].as_str() {
        "records" => records(root, output),
        "runtime" => runtime(root, output),
        "component" => component(root, output),
        _ => panic!("unknown observation"),
    }
}
