//! A completed native product carries the runtime inputs for the same published Psi.

use compiler::{Artifact, CompileRequest, NativeTarget, Program, TargetProviderInstallation};
use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
use std::sync::Arc;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalRuntimeError, TerminalRuntimeRequest,
};

#[test]
fn retained_execution_preserves_identity_and_resumes_without_decoding() {
    let bytes = include_bytes!(
        "../../../../../tests/fixtures/run/calls/scalar_contract_route_exit/linux_x86_64.psi"
    );
    let artifact = Arc::new(CanonicalTerminalArtifact::from_bytes(bytes).unwrap());
    let profile = target::TargetProfile::LinuxX64;
    let installation = TargetProviderInstallation::new(
        artifact.manifest().identity(),
        profile,
        Default::default(),
    );
    let producer = HostProducerTrust::accept("retained runtime fixture");
    let program = authorize_terminal_artifact(artifact, Some(&producer)).unwrap();
    let Artifact::Native(report) = compiler::compile(CompileRequest::Native {
        program: Program::Psi {
            program,
            target: NativeTarget {
                profile,
                installation,
                optimizations: &Default::default(),
                requested_boundaries: Vec::new(),
                application: Default::default(),
                behavior_exclusions: &[],
                x86_scalar_fma_provider: None,
            },
        },
        entry_memory: &[],
    })
    .unwrap() else {
        panic!("native compilation must retain the completed image");
    };
    let native = report.retained_native_artifact().unwrap();
    let artifact = native.psi_artifact();
    assert!(native.inspect_root_code_layouts().unwrap().is_empty());
    let request = |target| TerminalRuntimeRequest {
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
        native_provider: None,
        artifact,
        target: Some(target),
        provisions: report.runtime_provisions().map(|runtime| runtime.execution()),
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
    let other = other
        .with_publication(
            artifact.publication_metadata().unwrap().clone(),
            artifact.published_evidence().cloned(),
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
        project_build::published_terminal::execute_published_terminal(
            other_image,
            report.retained_native_artifact()
        ),
        Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection names another artifact or target"
        ))
    ));
    let before = terminal_codec::module_decode_count();
    let wrong_target = "macos_arm64";
    assert!(matches!(
        terminal_runtime::execute(request(wrong_target)),
        Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another deployment target"
        ))
    ));
    let mut wrong_image_target = request(wrong_target);
    wrong_image_target.provisions = None;
    assert!(matches!(
        project_build::published_terminal::execute_published_terminal(
            wrong_image_target,
            Some(native)
        ),
        Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection names another artifact or target"
        ))
    ));
    let normal = terminal_runtime::execute(request(profile.target_name())).unwrap();
    let sliced = terminal_runtime::execute_sliced(
        request(profile.target_name()),
        std::num::NonZeroU64::new(1).unwrap(),
    )
    .unwrap();
    let published = project_build::published_terminal::execute_published_terminal(
        request(profile.target_name()),
        Some(native),
    )
    .unwrap();
    let published_sliced = project_build::published_terminal::execute_published_terminal_sliced(
        request(profile.target_name()),
        Some(native),
        std::num::NonZeroU64::new(1).unwrap(),
    )
    .unwrap();
    for observed in [&normal, &sliced, &published, &published_sliced] {
        assert!(matches!(
            observed.status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        ));
        assert!(observed.stdout.is_empty());
        assert!(observed.stderr.is_empty());
    }
    assert!(sliced.work_suspensions > 0);
    assert_eq!(sliced.work_suspensions, published_sliced.work_suspensions);
    assert_eq!(terminal_codec::module_decode_count(), before);
}
