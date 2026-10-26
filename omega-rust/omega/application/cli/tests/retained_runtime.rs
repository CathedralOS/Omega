//! Build produces one artifact; hosted execution borrows it without source work.
//! One test owns the process-global decode counter for this binary.

use omega_build::{BuildRequest, BuildTargets, NativeDestination, ProjectProduct};
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
    let outcomes = omega_build::build(BuildRequest {
        root_path: repository
            .join("tests/omega/pass/calls/machine_self_call_recursion_compile/main.omg"),
        build_directory: Some(directory.path().to_path_buf()),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Native {
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
        native_provider: None,
        artifact,
        target: Some(target),
        provisions: report.runtime_provisions(),
        provider_proposal: None,
        x86_flags: terminal_runtime::capture_host_x86_flags(profile.target_name()).unwrap(),
        fuel_allowance: 100_000,
        // This program returns normally; possession of its artifact does not
        // supply process-domain death authority.
        domain_exit: TerminalDomainExitDelegation::Unprovided,
        standard_input: None,
        clock: None,
    };
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
    assert!(sliced.fuel_suspensions > 0);
    assert_eq!(terminal_codec::module_decode_count(), before);
}
