//! Compile an authorized Psi program for one receiving target.
#![forbid(unsafe_code)]

mod pipeline;
#[path = "replay/stage_validators.rs"]
pub mod stage_validators;

pub use host_provider_inputs::{InstalledPackageProviders, TargetProviderInstallation};
pub use native_artifact::AdmittedRuntimeProgram;
pub use pipeline::prepare::published_application::SavedProgramTarget;

use verified_optimization_input::RetainedTerminalArtifact;
use diagnostics::Diagnostic;
use pipeline::prepare::published_application::{complete_entry_scalars, prepare_saved_program};
use pipeline::request::NativeCompileRequest;
use provider_admission::current_terminal_authority_policy;
use psi_authorization::AuthorizedTerminalArtifact;
use psi_entry_memory::PrivateEntryRamInitialization;

/// Integration tests can supply an already retained input to isolate admission or
/// observe how often ordinary compilation enters a stage.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::pipeline::observations::{
        NativePipelineInvocation, NativePipelineUnit, native_pipeline_invocations,
    };
    pub use crate::pipeline::prepare::published_application::complete_entry_scalars;
    pub use crate::pipeline::realization::compile_runtime_foreign_adapter;
    pub use crate::pipeline::request::NativeCompileRequest;
    pub use crate::pipeline::{admit_runtime, compile};
}

/// Emit the requested native image from a published Psi program.
pub fn emit(
    program: AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
    entry_memory: &[PrivateEntryRamInitialization],
) -> Result<native_artifact::FinishedNativeImage, Vec<Diagnostic>> {
    emit_image(program, target, entry_memory, false).map(|(image, _)| image)
}

/// Check the requested stages against originals from this emission before releasing its image.
pub fn emit_with_validators(
    program: AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
    entry_memory: &[PrivateEntryRamInitialization],
    validators: &stage_validators::StageValidatorRequest,
) -> Result<
    (
        native_artifact::FinishedNativeImage,
        Vec<stage_validators::StageValidationRow>,
    ),
    stage_validators::NativeValidationError,
> {
    let profile = target.profile;
    let (image, replay) = emit_image(program, target, entry_memory, !validators.is_empty())
        .map_err(stage_validators::NativeValidationError::Compilation)?;
    let results = validators.check(profile, &image, &replay)?;
    Ok((image, results))
}

fn emit_image(
    program: AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
    entry_memory: &[PrivateEntryRamInitialization],
    retain_native_stage_inputs: bool,
) -> Result<
    (
        native_artifact::FinishedNativeImage,
        published_object_verification::NativeStageReplay,
    ),
    Vec<Diagnostic>,
> {
    with_published_request(
        program,
        target,
        entry_memory,
        &mut Vec::new(),
        |terminal, mut request| {
            request.retain_native_stage_inputs = retain_native_stage_inputs;
            pipeline::compile(&terminal, request)
        },
    )
}

/// Stop after admitting the provisions needed by the Psi interpreter.
pub fn admit_published_runtime(
    program: AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
    entry_memory: &[PrivateEntryRamInitialization],
    accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
) -> Result<AdmittedRuntimeProgram, Vec<Diagnostic>> {
    with_published_request(
        program,
        target,
        entry_memory,
        accounting,
        pipeline::admit_runtime,
    )
}

fn with_published_request<T>(
    authorization: AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
    private_entry_ram: &[PrivateEntryRamInitialization],
    accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
    finish: impl FnOnce(
        RetainedTerminalArtifact,
        NativeCompileRequest<'_>,
    ) -> Result<T, (target::ExecutableImageEmissionRequest, Vec<Diagnostic>)>,
) -> Result<T, Vec<Diagnostic>> {
    let subsystem = target.application.subsystem;
    let (terminal, permissions) = prepare_saved_program(authorization, target)?;

    let proposal = terminal.native_realization_proposal().ok_or_else(|| {
        vec![Diagnostic::error(
            "saved Psi compilation lost its native proposal",
        )]
    })?;
    let scalars = complete_entry_scalars(terminal.authorization(), proposal, accounting)?;
    let optimizations = proposal.post_terminal_optimizations().selections().clone();
    let request = NativeCompileRequest {
        retain_native_stage_inputs: false,
        installation_assumptions: &[],
        private_entry_ram,
        optimization_selections: &optimizations,
        terminal_authority_policy: current_terminal_authority_policy(),
        accepted_package_terminal_authority_permission_policy: permissions,
        terminal_authority_permission_policy: None,
        image_request: target::ExecutableImageEmissionRequest::direct(subsystem),
        imports: &[],
        components: &[],
        scalars: scalars.as_ref(),
    };
    finish(terminal, request).map_err(|(_, diagnostics)| diagnostics)
}
