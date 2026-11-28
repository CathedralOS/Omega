//! Compile a published Psi program under the receiving host's permissions.

pub(crate) mod input;

use compiler::{
    Artifact, CompileRequest, CompilerRequestOutcome, NativeTarget, Program,
    TargetProviderInstallation,
};
use diagnostics::Diagnostic;
use package_manager::operations::ReviewedHostPackages;
use product_publication::output_directory;
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
    program: &psi_authorization::AdmittedTerminalArtifact,
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
    let mut paths = Vec::with_capacity(profiles.len());
    for (&profile, directory) in profiles.iter().zip(&directories) {
        let directory = directory
            .as_ref()
            .map_err(|error| SavedProgramError::Output(error.to_string()))?;
        let product = prepare(
            program.clone().into(),
            profile,
            packages.as_ref(),
            &[],
            ExecutionProduct::Native,
        )?;
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

/// The caller requests an executable for publication or a runtime product for interpretation.
#[derive(Clone, Copy)]
pub(crate) enum ExecutionProduct {
    Native,
    Runtime,
}

pub(crate) fn prepare(
    program: psi_authorization::AdmittedTerminalArtifact,
    profile: TargetProfile,
    packages: Option<&ReviewedHostPackages>,
    entry_memory: &[psi_entry_memory::PrivateEntryRamInitialization],
    requested: ExecutionProduct,
) -> Result<product_publication::CompletedProduct, SavedProgramError> {
    let target = NativeTarget {
        profile,
        installation: installation(&program, profile, packages)?,
        requested_boundaries: Vec::new(),
        application: profile.standalone_application_metadata(),
        optimizations: &Default::default(),
        behavior_exclusions: &[],
        x86_scalar_fma_provider: None,
    };
    let program = match program {
        psi_authorization::AdmittedTerminalArtifact::Authorized(program) => {
            Program::Psi { program, target }
        }
        psi_authorization::AdmittedTerminalArtifact::Contained(program) => {
            Program::ContainedPsi { program, target }
        }
    };
    let request = match requested {
        ExecutionProduct::Native => CompileRequest::Native {
            program,
            entry_memory,
        },
        ExecutionProduct::Runtime => CompileRequest::Runtime {
            program,
            entry_memory,
        },
    };
    let compiled = compiler::compile(request, Default::default())
        .map_err(compiler::stage_validators::NativeValidationError::into_diagnostics)?;
    match (requested, compiled.artifact) {
        (ExecutionProduct::Native, Artifact::Native(product))
        | (ExecutionProduct::Runtime, Artifact::Runtime(product)) => Ok(product),
        _ => Err(SavedProgramError::Internal(
            "saved Psi compilation returned another product",
        )),
    }
}
