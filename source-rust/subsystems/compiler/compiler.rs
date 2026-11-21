//! Compile one program from settled inputs. Project builds call this once per stage.

#![forbid(unsafe_code)]

pub use compiler_identity::compiler_identity;

mod request_outcome;
mod runtime_consent;
pub use provider_correspondence::trust::selected_provider_grants;
pub use request_outcome::CompilerRequestOutcome;

use diagnostics::Diagnostic;
use psi_authorization::{
    AuthorizedTerminalArtifact, HostProducerTrust, authorize_terminal_artifact,
};

pub use native_compiler::SelectedProviderResult;
pub use native_compiler::{
    NativeApplicationSettings, SavedProgramTarget as NativeTarget, SourceNativeTarget,
    TargetProviderInstallation, stage_validators,
};
use source_compiler as source_to_psi;
pub use source_compiler::{PublishedPsi, StageSource, check, inspect};

/// Complete package declaration and target-supply findings from one publication.
pub fn review_package(
    source: StageSource<'_>,
    target: SourceNativeTarget<'_>,
    accounting: &mut source_compiler::compile_time_invocations::SourceCompletionAccounting,
) -> Result<
    (
        source_compiler::CheckedPackageDeclarations,
        SelectedProviderResult,
    ),
    Vec<Diagnostic>,
> {
    let (declarations, publication) =
        source_to_psi::review_package(source, target.profile.target_name(), accounting)?;
    let (artifact, providers, _) = publication.into_parts();
    let authorized = authorize_current_compilation(artifact)?;
    let selected = native_compiler::review_source_providers(authorized, providers, target)?;
    Ok((declarations, selected))
}

fn authorize_current_compilation(
    artifact: PublishedPsi,
) -> Result<AuthorizedTerminalArtifact, Vec<Diagnostic>> {
    let producer = HostProducerTrust::accept("current compiler invocation");
    authorize_terminal_artifact(artifact, Some(&producer))
        .map_err(|reason| vec![Diagnostic::error(reason.to_string())])
}

pub enum CompileRequest<'a> {
    Psi(StageSource<'a>),
    /// Admit the chosen entry and providers for execution on the Psi interpreter.
    Runtime {
        program: Program<'a>,
        entry_memory: &'a [psi_entry_memory::PrivateEntryRamInitialization],
    },
    Native {
        program: Program<'a>,
        entry_memory: &'a [psi_entry_memory::PrivateEntryRamInitialization],
    },
}

/// The source producer supplies its checked deployment; a saved program uses the
/// receiving host's installation already pinned to the authorized manifest.
pub enum Program<'a> {
    Source {
        source: StageSource<'a>,
        target: SourceNativeTarget<'a>,
        runtime_consent:
            Option<&'a review_evidence::permission_acceptance::TerminalPermissionAcceptance>,
    },
    CheckedSource {
        publication: source_compiler::PublishedTargetProgram,
        runtime_consent:
            Option<&'a review_evidence::permission_acceptance::TerminalPermissionAcceptance>,
        target: SourceNativeTarget<'a>,
    },
    Psi {
        program: AuthorizedTerminalArtifact,
        target: NativeTarget<'a>,
    },
}

pub enum Artifact {
    Psi(PublishedPsi),
    Runtime(product_publication::CompletedProduct),
    Native(product_publication::CompletedProduct),
}

#[derive(Default)]
pub struct CompileOptions<'a> {
    pub validators: Option<&'a stage_validators::StageValidatorRequest>,
    pub advisories: Option<&'a mut Vec<Diagnostic>>,
}

pub struct CompileOutput {
    pub artifact: Artifact,
    pub validations: Vec<stage_validators::StageValidationRow>,
}

/// Compile the requested product and check any selected native stages before
/// releasing their image. Earlier stopping points report stages they did not produce.
pub fn compile(
    request: CompileRequest<'_>,
    options: CompileOptions<'_>,
) -> Result<CompileOutput, stage_validators::NativeValidationError> {
    let none = stage_validators::StageValidatorRequest::default();
    let validators = options.validators.unwrap_or(&none);
    match request {
        CompileRequest::Psi(source) => {
            let validations = validators.stages_not_produced(
                source.product.calling_targets,
                |_| "this product stops at Terminal Psi before target operations",
            )?;
            let artifact = source_to_psi::compile(source, options.advisories)?;
            Ok(CompileOutput {
                artifact: Artifact::Psi(artifact),
                validations,
            })
        }
        CompileRequest::Runtime {
            program,
            entry_memory,
        } => {
            let validations = validators.stages_not_produced(
                &[program.profile().target_name()],
                |_| "runtime admission stops before target lowering",
            )?;
            let admitted = program
                .prepare(options.advisories)?
                .admit_runtime(entry_memory, &mut Vec::new())?;
            Ok(CompileOutput {
                artifact: Artifact::Runtime(product_publication::CompletedProduct::from_runtime(
                    admitted, None,
                )),
                validations,
            })
        }
        CompileRequest::Native {
            program,
            entry_memory,
        } => {
            let application = program.application().clone();
            let emitted = program
                .prepare(options.advisories)?
                .emit(entry_memory, validators)?;
            Ok(CompileOutput {
                artifact: Artifact::Native(product_publication::CompletedProduct::from_native(
                    emitted.image,
                    None,
                    application,
                )),
                validations: emitted.validations,
            })
        }
    }
}

impl Program<'_> {
    fn profile(&self) -> target::TargetProfile {
        match self {
            Self::Psi { target, .. } => target.profile,
            Self::Source { target, .. } | Self::CheckedSource { target, .. } => target.profile,
        }
    }

    fn application(&self) -> &target::NativeApplicationMetadata {
        match self {
            Self::Psi { target, .. } => &target.application,
            Self::Source { target, .. } | Self::CheckedSource { target, .. } => {
                &target.settings.application
            }
        }
    }

    fn prepare(
        self,
        advisories: Option<&mut Vec<Diagnostic>>,
    ) -> Result<native_compiler::NativeProgram, Vec<Diagnostic>> {
        match self {
            Self::Psi { program, target } => {
                native_compiler::prepare_saved_program(program, target)
            }
            Self::Source {
                source,
                target,
                runtime_consent,
            } => {
                let publication = source_to_psi::compile_for_target(
                    source,
                    target.profile.target_name(),
                    advisories,
                )?;
                Self::CheckedSource {
                    publication,
                    target,
                    runtime_consent,
                }
                .prepare(None)
            }
            Self::CheckedSource {
                publication,
                target,
                runtime_consent,
            } => {
                let subject = publication.subject();
                let consent = runtime_consent
                    .map(|accepted| {
                        let declarations = publication.runtime_permission_declarations()?;
                        Ok::<_, Vec<Diagnostic>>((
                            accepted,
                            declarations,
                            target.semantic_bindings.clone(),
                        ))
                    })
                    .transpose()?;
                let (artifact, providers, exclusions) = publication.into_parts();
                let authorized = authorize_current_compilation(artifact)?;
                let program = native_compiler::prepare_source_program(
                    authorized,
                    providers,
                    &exclusions,
                    target,
                )?;
                if let Some((accepted, declarations, bindings)) = consent {
                    runtime_consent::compare(
                        accepted,
                        subject,
                        &program,
                        &bindings,
                        &declarations,
                    )?;
                }
                Ok(program)
            }
        }
    }
}
