//! Compile an authorized Psi program for one receiving target.
#![forbid(unsafe_code)]

#[path = "replay/bridges.rs"]
pub mod bridge_replay;
mod pipeline;

use diagnostics::Diagnostic;
use terminal_installation::{AuthorizedTerminalArtifact, PrivateEntryRamInitialization};

/// Emit the requested native image from a published Psi program.
pub fn emit(
    program: AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
    entry_memory: &[PrivateEntryRamInitialization],
) -> Result<native_artifact::FinishedNativeImage, Vec<Diagnostic>> {
    with_published_request(
        program,
        target,
        entry_memory,
        &mut Vec::new(),
        |terminal, request| pipeline::compile(&terminal, request).map(|product| product.image),
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
        artifact_lowering::RetainedTerminalArtifact,
        NativeCompileRequest<'_>,
    ) -> Result<T, (target::ExecutableImageEmissionRequest, Vec<Diagnostic>)>,
) -> Result<T, Vec<Diagnostic>> {
    let subsystem = target.application.subsystem;
    let (terminal, permissions) = prepare_saved_program(authorization, target)?;

    let scalars = native_scalar_inputs::complete_native(&terminal, accounting)
        .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])?;
    let proposal = terminal.native_realization_proposal().ok_or_else(|| {
        vec![Diagnostic::error(
            "saved Psi compilation lost its native proposal",
        )]
    })?;
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

pub use backend_plan::{InstalledPackageProviders, TargetProviderInstallation};
pub use native_artifact::AdmittedRuntimeProgram;
pub use pipeline::prepare::program_entry::PublishedNativeEntry;
pub use pipeline::prepare::published_application::{
    PublishedApplicationInputs, SavedProgramTarget, prepare_published_application,
    prepare_saved_program,
};
pub use published_object_verification::NativeStageReplay;

pub use artifact_lowering::{
    BoundaryApplicationSettlement, ComponentExportInput, ComponentNativeInputs,
    NativeProgramEntrySettlement, TerminalNativeProposalRequest,
};
pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use pipeline::components::ComponentCompilation;
pub use pipeline::observations::{
    NativePipelineInvocation, NativePipelineUnit, native_pipeline_invocations,
};
pub use pipeline::prepare::application::{TerminalApplication, admit_package_permissions};
pub use pipeline::realization::RealizedNativeProduct;
pub use pipeline::request::NativeCompileRequest;
pub use pipeline::{admit_runtime, compile};
pub use provider_admission::provider_admission::{
    CompilerIntrinsicPlanProvenance, NativeBindingInput, SourceEvaluatedImportSettlement,
    TerminalAuthorityPermissionPolicy, current_terminal_authority_policy,
};
pub use verified_optimization_input::{
    decode_installation_assumptions, encode_installation_assumptions,
    retain_native_derivation_inputs,
};

pub use provider_realization::{
    PublishedSelection, SelectedExecution, SelectedProviderRealization, SelectedProviderResult,
    select_published_providers,
};

// Published native entry selection and explicitly requested provider-input inspection.
pub use provider_realization::{
    plan_provider_execution_inputs, target_supply::TargetSupplyExecutionInputs,
};

pub use provider_realization::program_entry::{
    PublishedEntryNomination, SelectedCompilerProgramEntry,
    plan_published_entry_calling_conventions,
};
