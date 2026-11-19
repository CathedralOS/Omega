//! Compile an authorized portable program with the receiving host's target settings.
use crate::{BuildError, BuildTargets};
use diagnostics::Diagnostic;
pub use native_compiler::TargetProviderInstallation;
use package_compilation::build_execution::build_value::EvaluatedBuildSettings;
use std::path::{Path, PathBuf};
use terminal_installation::AuthorizedTerminalArtifact;

/// The host chooses the reviewed package installation independently of the Psi
/// file. Only Build results and its approved declaration permissions enter
/// native planning. The producer's source and its boundary numbers stay behind.
pub fn installed_package_providers(
    profile: target::TargetProfile,
    build: &compiler::PackageBuild,
) -> Result<native_compiler::InstalledPackageProviders, BuildError> {
    let packages = build.package_inputs();
    let mut bindings = Vec::new();
    for handoff in packages
        .product_dependency_native_handoffs()
        .chain([build.build_handoff()])
    {
        handoff
            .append_native_bindings(profile, &mut bindings)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
    }
    Ok(native_compiler::InstalledPackageProviders::new(
        profile,
        bindings,
        packages.dependency_closure().declaration_permissions(),
        packages.accepted_semantic_bindings().cloned().collect(),
    ))
}

/// Host configuration may point at one approved application lock for many Psi
/// files. Pin the exact bytes before reading any package permissions.
pub fn configured_host_packages()
-> Result<Option<package_manager::operations::ReviewedHostPackages>, BuildError> {
    let path = std::env::var_os("OMEGA_HOST_PACKAGE_LOCK");
    let digest = std::env::var_os("OMEGA_HOST_PACKAGE_LOCK_SHA256");
    match (path, digest) {
        (None, None) => Ok(None),
        (Some(path), Some(digest)) => package_manager::operations::ReviewedHostPackages::open(
            Path::new(&path), digest.to_str().unwrap_or_default(),
        ).map(Some).map_err(|error| vec![Diagnostic::error(error)].into()),
        _ => Err(vec![Diagnostic::error("set OMEGA_HOST_PACKAGE_LOCK and OMEGA_HOST_PACKAGE_LOCK_SHA256 together to select reviewed host packages")].into()),
    }
}

pub fn saved_program_installation(
    authorization: &AuthorizedTerminalArtifact,
    profile: target::TargetProfile,
    host: Option<&package_manager::operations::ReviewedHostPackages>,
) -> Result<TargetProviderInstallation, BuildError> {
    match host {
        Some(host) => host
            .providers(profile)
            .map(|providers| providers.for_program(authorization))
            .map_err(|error| vec![Diagnostic::error(error)].into()),
        None => Ok(TargetProviderInstallation::new(
            authorization.identity(),
            profile,
            Default::default(),
        )),
    }
}

pub fn build_saved_program(
    authorization: AuthorizedTerminalArtifact,
    root: &Path,
    build_dir: &Path,
    targets: BuildTargets,
) -> Result<Vec<PathBuf>, BuildError> {
    let profiles = crate::build::target_profiles(&targets)?;
    let host = configured_host_packages()?;
    let (outputs, _leases) = crate::project::build_directory::admit_outputs(
        &profiles,
        build_dir,
        root,
        matches!(targets, BuildTargets::Batch(_)),
        false,
    );
    let mut paths = Vec::with_capacity(outputs.len());
    for output in outputs {
        let directory = output.directory?;
        let report = prepare_saved_program(
            authorization.clone(),
            root,
            output.profile,
            saved_program_installation(&authorization, output.profile, host.as_ref())?,
            crate::NativeDestination::Published,
            &[],
        )?;
        let (published, _) = crate::publication::publish_compilation(report, &directory)
            .map_err(BuildError::Publication)?;
        let path = published.checked_native_executable_path().ok_or_else(|| {
            BuildError::Publication("saved Psi compilation produced no executable".into())
        })?;
        paths.push(path.to_owned());
    }
    Ok(paths)
}

pub fn prepare_saved_program(
    authorization: AuthorizedTerminalArtifact,
    root: &Path,
    profile: target::TargetProfile,
    installation: TargetProviderInstallation,
    destination: crate::NativeDestination,
    private_entry_ram: &[products::PrivateEntryRamInitialization],
) -> Result<products::CompileReport, BuildError> {
    compiler::checked::run_on_compile_thread(|| {
        compile_saved(
            authorization,
            root,
            profile,
            installation,
            destination,
            private_entry_ram,
        )
    })
}

fn compile_saved(
    authorization: AuthorizedTerminalArtifact,
    root: &Path,
    profile: target::TargetProfile,
    installation: TargetProviderInstallation,
    destination: crate::NativeDestination,
    private_entry_ram: &[products::PrivateEntryRamInitialization],
) -> Result<products::CompileReport, BuildError> {
    let settings = EvaluatedBuildSettings::default();
    let application = target::NativeApplicationMetadata {
        intent: settings.application_intent,
        identifier: settings.application_identifier,
        subsystem: settings.subsystem,
        elf_interpreter: settings.elf_interpreters.effective(profile),
        name: None,
    };
    let mut accounting = products::SourceCompletionAccounting::default();
    let product = published_program::compile(
        authorization,
        native_compiler::SavedProgramTarget {
            profile,
            requested_boundaries: Vec::new(),
            installation,
            application,
            optimizations: &settings.optimizations,
            behavior_exclusions: &[],
            x86_scalar_fma_provider: settings
                .x86_deployment_claim
                .admit_for(Some(profile))
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
        },
        destination.image_demand(),
        private_entry_ram,
        &mut accounting,
    )?;
    Ok(products::CompileReport::from_saved_program(
        root.to_owned(),
        product,
        accounting,
    ))
}
