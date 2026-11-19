//! The saved specimen came from the fixture's directly supplied Linux/main nomination.
//! Its source publication and identity are recorded alongside its canonical bytes.

use native_artifact::{FinishedNativeImage, RequestedNativeArtifact};
use native_compiler::{SavedProgramTarget, TargetProviderInstallation};
use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use std::sync::Arc;
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;

const SAVED: &[u8] = include_bytes!(
    "../../../../../tests/fixtures/run/calls/scalar_contract_route_exit/linux_x86_64.psi"
);

fn emit(
    artifact: CanonicalTerminalArtifact,
) -> Result<FinishedNativeImage, Vec<diagnostics::Diagnostic>> {
    let profile = TargetProfile::LinuxX64;
    let identity = artifact.manifest().identity();
    let trust = HostProducerTrust::accept("repository scalar contract route fixture");
    let program = authorize_terminal_artifact(Arc::new(artifact), Some(&trust)).unwrap();
    let settings = Default::default();
    let target = SavedProgramTarget {
        profile,
        installation: TargetProviderInstallation::new(identity, profile, Default::default()),
        requested_boundaries: Vec::new(),
        application: Default::default(),
        optimizations: &settings,
        behavior_exclusions: &[],
        x86_scalar_fma_provider: None,
    };
    native_compiler::emit(program, target, &[])
}

#[test]
fn published_target_entry_emits_linux_image() {
    let artifact = CanonicalTerminalArtifact::from_bytes(SAVED).unwrap();
    assert_eq!(
        artifact.manifest().identity().to_string(),
        "b0e903a139095a3e99ccaf9771df7036d403983b81191df1615731fc21d1d721"
    );
    let entry = artifact
        .semantic_module()
        .program_entry_contract("linux_x86_64")
        .unwrap();
    assert_eq!(
        artifact
            .publication_metadata()
            .unwrap()
            .entry_schema_custody(entry.schema_declaration)
            .unwrap(),
        DeclarationCustody::ToolchainContract
    );
    let (image, _) = emit(artifact).unwrap().into_parts();
    let RequestedNativeArtifact::Direct(image) = image else {
        panic!("the fixture has no dynamic dependencies");
    };
    let bytes = &image.image().output().bytes;
    assert!(bytes.starts_with(b"\x7fELF"));
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    assert_native_exit(bytes);
}

#[test]
fn same_program_without_publication_refuses_native_emit() {
    let artifact = CanonicalTerminalArtifact::from_bytes(SAVED).unwrap();
    let debug = artifact
        .debug_bytes()
        .map(|bytes| terminal_codec::decode_debug_map(artifact.semantic_module(), bytes).unwrap());
    let optimization = artifact.optimization().clone();
    let unpublished = CanonicalTerminalArtifact::from_closed_parts(
        artifact.into_semantics(),
        optimization,
        debug.as_ref(),
    )
    .unwrap();
    let reopened = CanonicalTerminalArtifact::from_bytes(&unpublished.to_bytes()).unwrap();
    let Err(errors) = emit(reopened) else {
        panic!("the native receiver accepted a program without publication metadata");
    };
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("saved Psi lacks publication metadata")),
        "{errors:?}"
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn assert_native_exit(bytes: &[u8]) {
    use bounded_process::{
        BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessLimits,
        BoundedProcessPrepared, run_bounded_process,
    };
    use platform_custody::temporary_directory::TemporaryDirectory;
    use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::Duration};

    let scratch = TemporaryDirectory::create("saved-linux-exit", false).unwrap();
    let path = scratch.path().join("scalar_contract_route_exit");
    fs::write(&path, bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let mut command = Command::new(path);
    command.env_clear().current_dir(scratch.path());
    let memory = 128 << 20;
    let process = BoundedProcessPrepared::new(
        command,
        BoundedProcessLimits {
            cpu_seconds: 3,
            address_space_bytes: memory,
            file_size_bytes: 1024,
            open_files: 64,
            active_processes: 1,
            process_memory_bytes: memory,
            aggregate_memory_bytes: memory,
        },
        "saved Linux fixture",
    )
    .unwrap();
    let capture = BoundedCaptureLimits::new(
        1024,
        1024,
        Duration::from_secs(5),
        Duration::from_millis(500),
        Duration::from_millis(5),
    );
    let output = run_bounded_process(
        process,
        BoundedProcessInput::Null,
        capture,
        BoundedCaptureBudget::new(2048),
    )
    .unwrap();
    let expected = include_str!("../../../../../tests/fixtures/run/calls/scalar_contract_route_exit/expected_native_exit.txt").trim().parse::<i32>().unwrap();
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stderr: {:?}",
        output.stderr
    );
}
