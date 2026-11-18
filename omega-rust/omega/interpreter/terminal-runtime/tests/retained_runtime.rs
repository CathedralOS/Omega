// Integration test verifying that `orchestration::build` produces a single
// retained artifact and `terminal_runtime::execute` / `execute_sliced` borrows
// and runs it without re-decoding the Terminal module.

use orchestration::{BuildRequest, BuildTargets, NativeDestination, ProjectProduct};
use platform_custody::temporary_directory::TemporaryDirectory;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalRuntimeError, TerminalRuntimeRequest,
};

#[test]
fn retained_execution_preserves_identity_and_resumes_without_decoding() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let directory = TemporaryDirectory::create("retained-runtime", false).unwrap();
    let profile = target::TargetProfile::host_if_supported().expect("supported test host");
    let outcomes = orchestration::build(BuildRequest {
        stage_validators: Default::default(),
        root_path: repository
            .join("tests/fixtures/pass/calls/machine_self_call_recursion_compile/main.omg"),
        build_directory: Some(directory.path().to_path_buf()),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Native {
            installation_assumptions: Vec::new(),
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap();
    assert_eq!(outcomes.len(), 1);
    let report = outcomes
        .into_iter()
        .next()
        .unwrap()
        .result
        .unwrap()
        .into_report()
        .unwrap()
        .report;
    let artifact = report.retained_native_artifact().unwrap().psi_artifact();
    let request = |target| TerminalRuntimeRequest {
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
        native_provider: None,
        artifact,
        target: Some(target),
        provisions: report.runtime_provisions(),
        provider_proposal: None,
        x86_flags: terminal_runtime::capture_host_x86_flags(profile.target_name()).unwrap(),
        memory_ordering: None,
        work_budget: 100_000,
        domain_exit: TerminalDomainExitDelegation::Unprovided,
        standard_input: None,
        clock: None,
    };
    // Debug sections leave semantic code intact but change the canonical
    // manifest. Neither a retained runtime plan nor an image inspection can
    // transfer to this separately published artifact.
    let debug = terminal_codec::TerminalDebugMap {
        semantic: artifact.view().identity(),
        files: Vec::new(),
        sites: Vec::new(),
    };
    let other = terminal_codec::CanonicalTerminalArtifact::from_closed_parts(
        artifact.view().closed_module().clone(),
        artifact.optimization().clone(),
        artifact.debug_bytes().is_none().then_some(&debug),
    )
    .unwrap();
    assert_eq!(artifact.view().identity(), other.view().identity());
    assert_ne!(artifact.manifest().identity(), other.manifest().identity());
    let mut other_plan = request(profile.target_name());
    other_plan.artifact = &other;
    assert!(matches!(
        terminal_runtime::execute(other_plan),
        Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another Terminal program or entry"
        ))
    ));
    let mut other_image = request(profile.target_name());
    other_image.artifact = &other;
    other_image.provisions = None;
    assert!(matches!(
        orchestration::execute_published_terminal(other_image, report.retained_native_artifact()),
        Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection names another artifact or target"
        ))
    ));
    let before = terminal_codec::module_decode_count();
    let wrong_target = if profile.target_name() == "linux_x86_64" {
        "macos_arm64"
    } else {
        "linux_x86_64"
    };
    assert!(matches!(
        terminal_runtime::execute(request(wrong_target)),
        Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another deployment target"
        ))
    ));
    let normal = terminal_runtime::execute(request(profile.target_name())).unwrap();
    let sliced = terminal_runtime::execute_sliced(
        request(profile.target_name()),
        std::num::NonZeroU64::new(1).unwrap(),
    )
    .unwrap();
    for observed in [&normal, &sliced] {
        assert!(matches!(
            observed.status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        ));
        assert!(observed.stdout.is_empty());
        assert!(observed.stderr.is_empty());
    }
    assert!(sliced.work_suspensions > 0);
    assert_eq!(terminal_codec::module_decode_count(), before);
}

#[test]
fn published_layout_query_uses_the_selected_machines_linked_entry_and_regions() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let directory = TemporaryDirectory::create("published-runtime-layout", false).unwrap();
    let profile = target::TargetProfile::LinuxX64;
    let outcomes = orchestration::build(BuildRequest {
        stage_validators: Default::default(),
        root_path: repository.join("samples/cli/facts/selected_native_code_layout/local/main.omg"),
        build_directory: Some(directory.path().to_owned()),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Native {
            installation_assumptions: Vec::new(),
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap();
    let report = outcomes
        .into_iter()
        .next()
        .unwrap()
        .result
        .unwrap()
        .into_report()
        .unwrap()
        .report;
    let native = report.retained_native_artifact().unwrap();
    let layouts = native.inspect_root_code_layouts().unwrap();
    assert_eq!(layouts.len(), 1);
    let row = &layouts[0];
    assert_ne!(row.entry, 0);
    assert!(
        row.regions
            .iter()
            .any(|&(base, length)| (base..base + length).contains(&row.entry))
    );
    let request = || TerminalRuntimeRequest {
        artifact: native.psi_artifact(),
        target: Some(profile.target_name()),
        entry: None,
        provisions: report.runtime_provisions(),
        provider_proposal: None,
        native_provider: None,
        native_services: Default::default(),
        filesystem: Default::default(),
        x86_flags: None,
        memory_ordering: None,
        work_budget: 100_000,
        domain_exit: TerminalDomainExitDelegation::Unprovided,
        standard_input: None,
        clock: None,
    };
    assert!(matches!(terminal_runtime::execute(request()),
        Err(TerminalRuntimeError::Requirements(terminal_interpreter::TerminalInterpretError::UnsupportedCapability(
            terminal_interpreter::TerminalUnsupportedCapability::NativeCodeLayout { machine }
        ))) if machine == row.machine));
    let mut wrong_target = request();
    wrong_target.provisions = None;
    wrong_target.target = Some("windows_x86_64");
    assert!(matches!(
        orchestration::execute_published_terminal(wrong_target, Some(native)),
        Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection names another artifact or target"
        ))
    ));
    let observed = orchestration::execute_published_terminal_sliced(
        request(),
        Some(native),
        std::num::NonZeroU64::new(1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        observed.status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
    assert!(observed.work_suspensions > 0);
}
