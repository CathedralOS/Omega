use psi_diagnostics::Diagnostic;

use super::checked_entry::compile_to_checked_for_terminal;
use super::{
    CheckedCompilation, CompileOptions, CompileReport, PackageCompilationInputs,
    TerminalComponentDeploymentInputOwner, TerminalComponentDriverError,
    TerminalComponentProviderSettlement, TerminalComponentStagingInputs,
    stage_acquire_and_deploy_terminal_component_output,
};

/// One typed ordinary-frontend handoff to an external terminal deployment
/// owner.
///
/// The request deliberately contains no installation factory. Its admission
/// profile and provider settlements remain borrowed from their external
/// owners, while `deployment_owner` is the sole source of installation,
/// provider-occurrence, progress, and profile-decision authority.
#[derive(Debug)]
#[must_use = "terminal compile requests retain external deployment authority"]
pub struct TerminalComponentCompileRequest<'evidence, Owner> {
    options: CompileOptions,
    package_inputs: Option<PackageCompilationInputs>,
    profile: &'evidence psi_proof_admission::AdmissionProfile,
    settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
    deployment_owner: Owner,
}

impl<'evidence, Owner> TerminalComponentCompileRequest<'evidence, Owner> {
    pub fn new(
        options: CompileOptions,
        profile: &'evidence psi_proof_admission::AdmissionProfile,
        settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
        deployment_owner: Owner,
    ) -> Self {
        Self {
            options,
            package_inputs: None,
            profile,
            settlements,
            deployment_owner,
        }
    }

    pub fn with_package_inputs(mut self, package_inputs: PackageCompilationInputs) -> Self {
        self.package_inputs = Some(package_inputs);
        self
    }

    pub const fn options(&self) -> &CompileOptions {
        &self.options
    }

    pub const fn package_inputs(&self) -> Option<&PackageCompilationInputs> {
        self.package_inputs.as_ref()
    }

    pub const fn profile(&self) -> &psi_proof_admission::AdmissionProfile {
        self.profile
    }

    pub fn settlements(&self) -> &[TerminalComponentProviderSettlement<'evidence>] {
        &self.settlements
    }

    pub const fn deployment_owner(&self) -> &Owner {
        &self.deployment_owner
    }

    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        CompileOptions,
        Option<PackageCompilationInputs>,
        &'evidence psi_proof_admission::AdmissionProfile,
        Vec<TerminalComponentProviderSettlement<'evidence>>,
        Owner,
    ) {
        (
            self.options,
            self.package_inputs,
            self.profile,
            self.settlements,
            self.deployment_owner,
        )
    }
}

/// A rejected typed terminal compile handoff.
///
/// Frontend and staging-input binding rejection retain the complete original
/// request. Binding rejection additionally retains the checked result so a
/// caller can inspect or reuse the exact frontend owner without rerunning it.
/// Once staging begins, the established driver error retains staging or linear
/// deployment custody; options and package inputs remain beside it.
#[derive(Debug)]
pub enum TerminalComponentCompileError<'evidence, Owner>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    Frontend {
        diagnostics: Vec<Diagnostic>,
        request: TerminalComponentCompileRequest<'evidence, Owner>,
    },
    StagingInputBinding {
        diagnostic: Diagnostic,
        checked: CheckedCompilation,
        request: TerminalComponentCompileRequest<'evidence, Owner>,
    },
    Driver {
        error: Box<TerminalComponentDriverError<'evidence, Owner>>,
        options: CompileOptions,
        package_inputs: Option<PackageCompilationInputs>,
    },
}

impl<Owner> std::fmt::Display for TerminalComponentCompileError<'_, Owner>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frontend { diagnostics, .. } => diagnostics
                .first()
                .map_or("terminal component frontend failed", |diagnostic| {
                    diagnostic.message.as_str()
                })
                .fmt(formatter),
            Self::StagingInputBinding { diagnostic, .. } => diagnostic.fmt(formatter),
            Self::Driver { error, .. } => error.fmt(formatter),
        }
    }
}

impl<Owner> std::error::Error for TerminalComponentCompileError<'_, Owner> where
    Owner: TerminalComponentDeploymentInputOwner
{
}

/// Run the ordinary checked frontend and hand its exact result to the existing
/// terminal staging/deployment driver.
///
/// This operation does not enter the legacy checked-tree backend or
/// `write_output`. It cannot complete production installation until its caller
/// supplies a concrete non-test [`TerminalComponentDeploymentInputOwner`].
pub fn compile_terminal_component_output<'evidence, Owner>(
    request: TerminalComponentCompileRequest<'evidence, Owner>,
) -> Result<CompileReport, Box<TerminalComponentCompileError<'evidence, Owner>>>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    let checked = match compile_to_checked_for_terminal(request.options(), request.package_inputs())
    {
        Ok(checked) => checked,
        Err(diagnostics) => {
            return Err(Box::new(TerminalComponentCompileError::Frontend {
                diagnostics,
                request,
            }));
        }
    };
    let source_file_count = checked.source_file_count();
    let (options, package_inputs, profile, settlements, deployment_owner) = request.into_parts();
    let staging_inputs =
        match TerminalComponentStagingInputs::from_checked(&checked, profile, settlements) {
            Ok(inputs) => inputs,
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let (_, profile, settlements) = error.into_parts();
                let request = TerminalComponentCompileRequest {
                    options,
                    package_inputs,
                    profile,
                    settlements,
                    deployment_owner,
                };
                return Err(Box::new(
                    TerminalComponentCompileError::StagingInputBinding {
                        diagnostic,
                        checked,
                        request,
                    },
                ));
            }
        };
    match stage_acquire_and_deploy_terminal_component_output(
        &options,
        source_file_count,
        &checked,
        staging_inputs,
        deployment_owner,
    ) {
        Ok(report) => Ok(report),
        Err(error) => Err(Box::new(TerminalComponentCompileError::Driver {
            error,
            options,
            package_inputs,
        })),
    }
}
