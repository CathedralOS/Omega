//! Compile a published Psi program under the receiving host's permissions.

use compiler::{
    Artifact, CompileRequest, CompilerRequestOutcome, NativeTarget, Program,
    TargetProviderInstallation,
};
use diagnostics::Diagnostic;
use package_compilation::build_execution::build_value::EvaluatedBuildSettings;
use package_manager::operations::ReviewedHostPackages;
use project_build::output_directory;
use psi_authorization::AuthorizedTerminalArtifact;
use std::path::{Path, PathBuf};
use target::TargetProfile;

pub(crate) fn configured_host_packages() -> Result<Option<ReviewedHostPackages>, Vec<Diagnostic>> {
    let path = std::env::var_os("OMEGA_HOST_PACKAGE_LOCK");
    let digest = std::env::var_os("OMEGA_HOST_PACKAGE_LOCK_SHA256");
    match (path, digest) {
        (None, None) => Ok(None),
        (Some(path), Some(digest)) => {
            ReviewedHostPackages::open(Path::new(&path), digest.to_str().unwrap_or_default())
                .map(Some)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        }
        _ => Err(vec![Diagnostic::error(
            "set OMEGA_HOST_PACKAGE_LOCK and OMEGA_HOST_PACKAGE_LOCK_SHA256 together to select reviewed host packages",
        )]),
    }
}

pub(crate) fn installation(
    program: &AuthorizedTerminalArtifact,
    profile: TargetProfile,
    packages: Option<&ReviewedHostPackages>,
) -> Result<TargetProviderInstallation, Vec<Diagnostic>> {
    match packages {
        Some(packages) => packages
            .providers(profile)
            .map(|providers| providers.for_program(program))
            .map_err(|reason| vec![Diagnostic::error(reason)]),
        None => Ok(TargetProviderInstallation::new(
            program.identity(),
            profile,
            Default::default(),
        )),
    }
}

pub(crate) enum SavedProgramError {
    Diagnostics(Vec<Diagnostic>),
    Output(String),
    Internal(&'static str),
}

impl std::fmt::Display for SavedProgramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                diagnostics::write_diagnostics(formatter, diagnostics)
            }
            Self::Output(reason) => formatter.write_str(reason),
            Self::Internal(reason) => formatter.write_str(reason),
        }
    }
}

impl SavedProgramError {
    pub(crate) fn outcome(&self) -> CompilerRequestOutcome {
        match self {
            Self::Diagnostics(diagnostics) => CompilerRequestOutcome::of_refusal(diagnostics),
            Self::Output(_) => CompilerRequestOutcome::Reject,
            Self::Internal(_) => CompilerRequestOutcome::InternalFailure,
        }
    }
}

impl From<Vec<Diagnostic>> for SavedProgramError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

pub(crate) fn publish(
    program: AuthorizedTerminalArtifact,
    root: &Path,
    requested_directory: Option<&Path>,
    profiles: &[TargetProfile],
    per_target: bool,
) -> Result<Vec<PathBuf>, SavedProgramError> {
    let packages = configured_host_packages()?;
    let base = output_directory::base(root, requested_directory);
    // Hold every target's lease until the entire invocation has finished publishing.
    let directories: Vec<_> = profiles
        .iter()
        .map(|profile| {
            let path = if per_target {
                base.join(profile.target_name())
            } else {
                base.clone()
            };
            output_directory::acquire(&path, root)
        })
        .collect();
    let settings = EvaluatedBuildSettings::default();
    let mut paths = Vec::with_capacity(profiles.len());
    for (&profile, directory) in profiles.iter().zip(&directories) {
        let directory = directory
            .as_ref()
            .map_err(|error| SavedProgramError::Output(error.to_string()))?;
        let target = NativeTarget {
            profile,
            installation: installation(&program, profile, packages.as_ref())?,
            requested_boundaries: Vec::new(),
            application: target::NativeApplicationMetadata {
                intent: settings.application_intent,
                identifier: settings.application_identifier.clone(),
                subsystem: settings.subsystem,
                elf_interpreter: settings.elf_interpreters.effective(profile),
                name: None,
            },
            optimizations: &settings.optimizations,
            behavior_exclusions: &[],
            x86_scalar_fma_provider: settings
                .x86_deployment_claim
                .admit_for(Some(profile))
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
        };
        let Artifact::Native(product) = compiler::compile(CompileRequest::Native {
            program: Program::Psi(program.clone()),
            target,
            entry_memory: &[],
        })?
        else {
            return Err(SavedProgramError::Internal(
                "saved Psi compilation produced no native product",
            ));
        };
        let published = product
            .publish(root, directory.directory())
            .map_err(SavedProgramError::Output)?;
        let path = published.checked_native_executable_path().ok_or_else(|| {
            SavedProgramError::Output("saved Psi compilation produced no executable".into())
        })?;
        paths.push(path.to_owned());
    }
    Ok(paths)
}
