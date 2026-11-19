//! Compile an authorized Psi program for one receiving target.
#![forbid(unsafe_code)]

mod pipeline;
#[path = "replay/stage_validators.rs"]
pub mod stage_validators;

pub use host_provider_inputs::{InstalledPackageProviders, TargetProviderInstallation};
pub use native_artifact::AdmittedRuntimeProgram;
pub use pipeline::prepare::published_application::{
    NativeApplicationSettings, SavedProgramTarget, SourceNativeTarget,
    prepare_saved_program, prepare_source_program,
};

use diagnostics::Diagnostic;
use pipeline::prepare::published_application::complete_entry_scalars;
use pipeline::request::NativeCompileRequest;
use provider_admission::current_terminal_authority_policy;
use psi_authorization::AuthorizedTerminalArtifact;
use psi_entry_memory::PrivateEntryRamInitialization;
use verified_optimization_input::RetainedTerminalArtifact;

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
    prepare_saved_program(program, target)?.emit(entry_memory)
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
    prepare_saved_program(program, target)?.emit_with_validators(entry_memory, validators)
}

/// Stop after admitting the provisions needed by the Psi interpreter.
pub fn admit_published_runtime(
    program: AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
    entry_memory: &[PrivateEntryRamInitialization],
    accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
) -> Result<AdmittedRuntimeProgram, Vec<Diagnostic>> {
    prepare_saved_program(program, target)?.admit_runtime(entry_memory, accounting)
}

/// A native invocation whose target provider demand was matched to its authorized
/// program. Its reviewed assumptions travel to the same entry and installation checks.
pub struct NativeProgram {
    terminal: RetainedTerminalArtifact,
    permissions: provider_admission::TerminalAuthorityPermissionPolicy,
    profile: target::TargetProfile,
    subsystem: u16,
    installation_assumptions: Vec<installation_conditions::InstallationEnvironmentAssumption>,
}

impl NativeProgram {
    /// The settled permissions and binding selections used by both execution paths.
    pub fn native_realization_proposal(
        &self,
    ) -> Result<&verified_optimization_input::TerminalNativeRealizationProposal, Vec<Diagnostic>> {
        self.terminal.native_realization_proposal().ok_or_else(|| {
            vec![Diagnostic::error("prepared Psi compilation lost its native proposal")]
        })
    }

    pub fn emit(
        self,
        entry_memory: &[PrivateEntryRamInitialization],
    ) -> Result<native_artifact::FinishedNativeImage, Vec<Diagnostic>> {
        self.emit_image(entry_memory, false).map(|(image, _)| image)
    }

    pub fn emit_with_validators(
        self,
        entry_memory: &[PrivateEntryRamInitialization],
        validators: &stage_validators::StageValidatorRequest,
    ) -> Result<
        (
            native_artifact::FinishedNativeImage,
            Vec<stage_validators::StageValidationRow>,
        ),
        stage_validators::NativeValidationError,
    > {
        let profile = self.profile;
        let (image, replay) = self.emit_image(entry_memory, !validators.is_empty())?;
        let rows = validators.check(profile, &image, &replay)?;
        Ok((image, rows))
    }

    fn emit_image(
        self,
        entry_memory: &[PrivateEntryRamInitialization],
        retain_native_stage_inputs: bool,
    ) -> Result<
        (
            native_artifact::FinishedNativeImage,
            published_object_verification::NativeStageReplay,
        ),
        Vec<Diagnostic>,
    > {
        self.finish(
            entry_memory,
            &mut Vec::new(),
            retain_native_stage_inputs,
            |terminal, request| pipeline::compile(&terminal, request),
        )
    }

    pub fn admit_runtime(
        self,
        entry_memory: &[PrivateEntryRamInitialization],
        accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
    ) -> Result<AdmittedRuntimeProgram, Vec<Diagnostic>> {
        self.finish(entry_memory, accounting, false, pipeline::admit_runtime)
    }

    fn finish<T>(
        self,
        private_entry_ram: &[PrivateEntryRamInitialization],
        accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
        retain_native_stage_inputs: bool,
        finish: impl FnOnce(
            RetainedTerminalArtifact,
            NativeCompileRequest<'_>,
        )
            -> Result<T, (target::ExecutableImageEmissionRequest, Vec<Diagnostic>)>,
    ) -> Result<T, Vec<Diagnostic>> {
        let Self {
            terminal,
            permissions,
            subsystem,
            installation_assumptions,
            ..
        } = self;
        let proposal = terminal.native_realization_proposal().ok_or_else(|| {
            vec![Diagnostic::error(
                "prepared Psi compilation lost its native proposal",
            )]
        })?;
        let scalars = complete_entry_scalars(terminal.authorization(), proposal, accounting)?;
        let optimizations = proposal.post_terminal_optimizations().selections().clone();
        let request = NativeCompileRequest {
            retain_native_stage_inputs,
            installation_assumptions: &installation_assumptions,
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
}
