//! Execute the retained Psi artifact with the provisions admitted for its target.
use super::compilation::RunProduct;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeError, TerminalRuntimeObservation,
    TerminalRuntimeRequest,
};

pub(super) fn execute(
    report: &RunProduct,
    input: Option<Box<dyn std::io::Read + Send>>,
    psi_work_budget: u64,
    directories: platform_custody::directory_mappings::DirectoryMappings,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let target = report.target.target_name();
    let program = report
        .artifact()
        .ok_or(TerminalRuntimeError::InvalidProvision(
            "run lost its produced Psi artifact before execution",
        ))?;
    let runtime = report.runtime();
    if let Some(contained) = report.contained() {
        return super::contained::execute(
            contained,
            program.artifact,
            target,
            runtime.map(|provisions| provisions.execution()),
            input,
            psi_work_budget,
            directories,
        );
    }
    use terminal_runtime::{
        TerminalFilesystemDelegation as Filesystem, TerminalNativeServiceDelegation as Services,
    };
    // The working-directory view applies wherever the interpreter performs the
    // Product's filesystem calls itself: a Linux program on its matching
    // Linux host, with no native provider. Native code and another target's
    // filesystem cannot be confined to that view, so those runs keep the host
    // namespace unless the caller maps directories explicitly
    // (OWNER_QUESTIONS.md#unconfinable-standalone-filesystem).
    let confinable = directories.is_empty()
        && Some(report.target) == target::TargetProfile::host_if_supported()
        && matches!(
            report.target,
            target::TargetProfile::LinuxX64 | target::TargetProfile::LinuxArm64
        )
        && runtime.is_none_or(|runtime| runtime.published_native_adapters().next().is_none());
    let directories = if directories.is_empty() && !confinable {
        None
    } else {
        super::contained::standalone_directories(directories)?
    };
    let (filesystem, native_services, native_provider) = if let Some(directories) = directories {
        (Filesystem::Mapped(directories), Services::Unprovided, None)
    } else {
        (
            Filesystem::HostNamespace,
            Services::MatchingHost,
            Some(native_provider_sponsor::for_adapters(
                std::process::Command::new(std::env::current_exe().map_err(|error| {
                    TerminalRuntimeError::MissingProvision {
                        capability: "native-provider-sponsor",
                        reason: error.to_string(),
                    }
                })?),
                runtime
                    .into_iter()
                    .flat_map(|runtime| runtime.published_native_adapters()),
            )),
        )
    };
    retained_image_runtime::execute(
        TerminalRuntimeRequest {
            entry: None,
            filesystem,
            native_services,
            native_provider,
            x86_flags: terminal_runtime::capture_host_x86_flags(target)?,
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(target),
            provider_proposal: None,
            artifact: program.artifact,
            target: Some(target),
            provisions: runtime.map(|provisions| provisions.execution()),
            work_budget: psi_work_budget,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            output_limits: None,
            standard_input: input,
            clock: Some(terminal_runtime::TerminalClockEnvironment::Host),
            containment: None,
        },
        program.image,
    )
}
