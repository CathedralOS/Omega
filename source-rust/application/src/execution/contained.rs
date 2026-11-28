//! Interpret saved Psi the host does not trust, on the contained route.
//!
//! `omega run app.psi` admits saved Psi without producer trust
//! (`psi_authorization::admit_contained_interpretation`). Runtime admission
//! selects its host realizations and stops before optimization and native
//! code. The interpreter then runs the contained artifact under an invocation
//! account, output ceilings and a live-wait ceiling, with no native provider,
//! native services or components. Standard input is supplied as for source,
//! and the working directory is mounted read/write at `/` unless explicit run
//! directory mappings replace it.

use build_model::accounting::BuildEvaluationSponsor;
use build_model::contracts::{
    BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use terminal_runtime::{
    TerminalClockEnvironment, TerminalDomainExitDelegation, TerminalFilesystemDelegation,
    TerminalNativeServiceDelegation, TerminalOutputLimits, TerminalRuntimeContainment,
    TerminalRuntimeError, TerminalRuntimeObservation, TerminalRuntimeRequest,
};

/// Output a contained guest may produce before it is stopped.
pub(crate) const CONTAINED_OUTPUT_LIMITS: TerminalOutputLimits = TerminalOutputLimits {
    stdout_bytes: 16 << 20,
    stderr_bytes: 16 << 20,
    combined_bytes: 32 << 20,
};
/// Live interpreter storage a contained guest may hold at once.
pub(crate) const CONTAINED_LIVE_STORAGE_CELLS: u64 = 1 << 22;
pub(crate) const CONTAINED_LIVE_PAYLOAD_BYTES: u64 = 256 << 20;
/// Total time a contained guest may spend in live clock waits.
pub(crate) const CONTAINED_LIVE_WAIT_NANOSECONDS: u64 = 60_000_000_000;

<<<<<<< HEAD
/// The standalone Product grant (`wiki/spec/build/command_line.md#runtime-directory-mappings`)
/// for source and saved Psi alike: the caller's working directory read/write
/// at `/`, unless explicit run directory mappings replace it. Mapped
/// directories confine every path beneath their retained handles. `None`
/// means this host has no mapped namespace and no explicit mapping was asked.
pub(super) fn standalone_directories(
    mut directories: platform_custody::directory_mappings::DirectoryMappings,
) -> Result<Option<platform_custody::directory_mappings::DirectoryMappings>, TerminalRuntimeError> {
    if directories.is_empty() {
        if !cfg!(target_os = "linux") {
            return Ok(None);
=======
/// The standalone Product grant (`wiki/spec/build/command_line.md#runtime-directory-mappings`):
/// the caller's working directory read/write at `/`, unless explicit run
/// directory mappings replace it. Mapped directories confine every path
/// beneath their retained handles; a host without that namespace gets
/// standard streams alone.
fn filesystem(
    mut directories: platform_custody::directory_mappings::DirectoryMappings,
) -> Result<TerminalFilesystemDelegation, TerminalRuntimeError> {
    if directories.is_empty() {
        if !cfg!(target_os = "linux") {
            return Ok(TerminalFilesystemDelegation::StandardStreamsOnly);
>>>>>>> b84beb8852d955865ae5e2209793554e8aa7eedc
        }
        let unavailable = |error: std::io::Error| TerminalRuntimeError::MissingProvision {
            capability: "working directory mount",
            reason: error.to_string(),
        };
        let cwd = std::env::current_dir().map_err(unavailable)?;
        directories
            .grant(
                &cwd,
                "/",
                platform_custody::directory_mappings::DirectoryAccess::ReadWrite,
            )
            .map_err(unavailable)?;
    }
<<<<<<< HEAD
    Ok(Some(directories))
}

/// A contained guest on a host without the mapped namespace gets standard
/// streams alone.
fn filesystem(
    directories: platform_custody::directory_mappings::DirectoryMappings,
) -> Result<TerminalFilesystemDelegation, TerminalRuntimeError> {
    Ok(standalone_directories(directories)?.map_or(
        TerminalFilesystemDelegation::StandardStreamsOnly,
        TerminalFilesystemDelegation::Mapped,
    ))
=======
    Ok(TerminalFilesystemDelegation::Mapped(directories))
>>>>>>> b84beb8852d955865ae5e2209793554e8aa7eedc
}

pub(super) fn execute(
    contained: &psi_authorization::ContainedTerminalArtifact,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    target: &str,
    provisions: Option<&published_runtime_input::RuntimeExecutionProvisions>,
    input: Option<Box<dyn std::io::Read + Send>>,
    psi_work_budget: u64,
    directories: platform_custody::directory_mappings::DirectoryMappings,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let filesystem = filesystem(directories)?;
    let refused =
        |_| TerminalRuntimeError::InvalidProvision("contained run limits are not admissible");
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    let limits = BuildEvaluationSponsorLimits::new(
        psi_work_budget,
        1,
        1,
        1,
        CONTAINED_LIVE_STORAGE_CELLS,
        CONTAINED_LIVE_PAYLOAD_BYTES,
        CONTAINED_LIVE_STORAGE_CELLS,
        CONTAINED_LIVE_PAYLOAD_BYTES,
    )
    .ok_or(TerminalRuntimeError::InvalidProvision(
        "contained run limits are not admissible",
    ))?;
    let sponsor =
        BuildEvaluationSponsor::new_with_measurement(limits, measurement).map_err(refused)?;
    let account = sponsor
        .begin_invocation(sponsor.measurement(), psi_work_budget)
        .map_err(refused)?;
    terminal_runtime::execute(TerminalRuntimeRequest {
        entry: None,
        native_provider: None,
        native_services: TerminalNativeServiceDelegation::Unprovided,
        filesystem,
        x86_flags: None,
        memory_ordering: terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(
            target,
        ),
        artifact,
        target: Some(target),
        provisions,
        provider_proposal: None,
        work_budget: psi_work_budget,
        domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
        output_limits: Some(CONTAINED_OUTPUT_LIMITS),
        standard_input: input,
        clock: Some(TerminalClockEnvironment::HostWithin {
            remaining_wait_nanoseconds: CONTAINED_LIVE_WAIT_NANOSECONDS,
        }),
        containment: Some(TerminalRuntimeContainment {
            artifact: contained,
            account: &account,
        }),
    })
}
