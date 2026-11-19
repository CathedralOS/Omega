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
    "../../../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi"
);

fn with_published_target<T>(
    artifact: CanonicalTerminalArtifact,
    compile: impl FnOnce(psi_authorization::AuthorizedTerminalArtifact, SavedProgramTarget<'_>) -> T,
) -> T {
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
    compile(program, target)
}

fn emit(
    artifact: CanonicalTerminalArtifact,
) -> Result<FinishedNativeImage, Vec<diagnostics::Diagnostic>> {
    with_published_target(artifact, |program, target| {
        native_compiler::emit(program, target, &[])
    })
}

#[test]
fn requested_validators_check_the_emitted_linux_program() {
    use native_compiler::stage_validators::{
        StageValidationResult, StageValidator, StageValidatorRequest,
    };

    let validators = StageValidatorRequest::from_exact_names(
        ["FixedFrameCorrespondence", "FixedFrameCorrespondence"],
        true,
    )
    .unwrap();
    let (image, rows) = with_published_target(
        CanonicalTerminalArtifact::from_bytes(SAVED).unwrap(),
        |program, target| native_compiler::emit_with_validators(program, target, &[], &validators),
    )
    .unwrap();
    assert!(matches!(
        image.parts().0,
        RequestedNativeArtifact::Direct(_)
    ));
    assert_eq!(
        rows.iter().map(|row| row.validator).collect::<Vec<_>>(),
        StageValidator::ROSTER
    );
    assert!(
        rows.iter()
            .all(|row| row.target.as_deref() == Some("linux_x86_64"))
    );
    assert_eq!(rows[0].result, StageValidationResult::Passed);
    assert_eq!(rows[1].result, StageValidationResult::Passed);
    assert_eq!(
        rows[2].result,
        StageValidationResult::Inapplicable("no selected-instruction rewrite rule ran")
    );
}

#[test]
fn named_rewrite_without_a_selected_rule_refuses_even_with_every_validator() {
    use native_compiler::stage_validators::{NativeValidationError, StageValidatorRequest};

    let validators =
        StageValidatorRequest::from_exact_names(["SelectedInstructionRewriteReplay"], true)
            .unwrap();
    let result = with_published_target(
        CanonicalTerminalArtifact::from_bytes(SAVED).unwrap(),
        |program, target| native_compiler::emit_with_validators(program, target, &[], &validators),
    );
    assert!(
        matches!(result, Err(NativeValidationError::Unavailable(diagnostic))
        if diagnostic.message.contains("no selected-instruction rewrite rule ran"))
    );
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
fn saved_publication_rejects_evidence_from_another_module_or_obligation() {
    let artifact = CanonicalTerminalArtifact::from_bytes(SAVED).unwrap();
    let evidence = artifact.published_evidence().unwrap().as_ref();
    assert!(!evidence.seals.is_empty());
    let debug = artifact
        .debug_bytes()
        .map(|bytes| terminal_codec::decode_debug_map(artifact.semantic_module(), bytes).unwrap());
    let publish = |evidence| {
        let program = CanonicalTerminalArtifact::from_closed_parts(
            artifact.view().closed_module().clone(),
            artifact.optimization().clone(),
            debug.as_ref(),
        )
        .unwrap();
        program.with_publication(
            Arc::clone(artifact.publication_metadata().unwrap()),
            Some(Arc::new(evidence)),
        )
    };
    assert_eq!(publish(evidence.clone()).unwrap().to_bytes(), SAVED);

    let mut foreign = evidence.clone();
    foreign.module.program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([9; 32]);
    let error = publish(foreign).unwrap_err();
    assert!(format!("{error}").contains("InvalidPublication"), "{error}");

    let missing = semantic_vocabulary::MachineId::new(u64::MAX).unwrap();
    assert!(artifact.semantic_module().machine(missing).is_none());
    let mut unowned = evidence.clone();
    unowned.seals[0].machine = missing;
    unowned.seals[0].fingerprint = unowned.seals[0].recompute().unwrap();
    let encoded = publish(unowned).unwrap().to_bytes();
    let error = CanonicalTerminalArtifact::from_bytes(&encoded).unwrap_err();
    assert!(format!("{error}").contains("UnownedSeal"), "{error}");

    let mut damaged = SAVED.to_vec();
    let encoded_evidence = terminal_codec::encode_operation_evidence(evidence).unwrap();
    assert!(damaged.ends_with(&encoded_evidence));
    *damaged.last_mut().unwrap() ^= 1;
    let error = CanonicalTerminalArtifact::from_bytes(&damaged).unwrap_err();
    assert!(
        format!("{error}").contains("operation evidence seal fingerprint mismatch"),
        "{error}"
    );
}

#[test]
fn selected_entry_requires_its_checked_declaration() {
    use provider_realization::program_entry::SelectedCompilerProgramEntry;

    let artifact = CanonicalTerminalArtifact::from_bytes(SAVED).unwrap();
    let profile = TargetProfile::LinuxX64;
    let slot = profile.program_entry_slot();
    let mut module = artifact.semantic_module().clone();
    let entry = module
        .program_entry_contract(profile.target_name())
        .unwrap()
        .entry;
    SelectedCompilerProgramEntry::from_terminal(&module, entry, slot).unwrap();

    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap();
    assert!(machine.declaration.take().is_some());
    let refusal = SelectedCompilerProgramEntry::from_terminal(&module, entry, slot).unwrap_err();
    assert!(
        refusal.iter().any(|diagnostic| diagnostic.message == "the selected entry has no checked declaration"),
        "{refusal:?}"
    );
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
    let expected = include_str!("../../../../../tests/fixtures/calls/run/scalar_contract_route_exit/expected_native_exit.txt").trim().parse::<i32>().unwrap();
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stderr: {:?}",
        output.stderr
    );
}
