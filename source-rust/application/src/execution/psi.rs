//! Execute the retained Psi artifact with the provisions admitted for its target.
use super::compilation::RunProduct;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeError, TerminalRuntimeObservation,
    TerminalRuntimeRequest,
};

pub(super) fn execute(
    report: &RunProduct,
    input: Option<Box<dyn std::io::Read + Send>>,
    target: &str,
    psi_work_budget: u64,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let program = report
        .artifact()
        .ok_or(TerminalRuntimeError::InvalidProvision(
            "run lost its produced Psi artifact before execution",
        ))?;
    let runtime = report.runtime();
    retained_image_runtime::execute(
        TerminalRuntimeRequest {
            entry: None,
            filesystem: terminal_runtime::TerminalFilesystemDelegation::HostNamespace,
            native_services: terminal_runtime::TerminalNativeServiceDelegation::MatchingHost,
            native_provider: Some(native_provider_sponsor::for_adapters(
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
            x86_flags: terminal_runtime::capture_host_x86_flags(target)?,
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(target),
            provider_proposal: None,
            artifact: program.artifact,
            target: Some(target),
            provisions: runtime.map(|provisions| provisions.execution()),
            work_budget: psi_work_budget,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: input,
            clock: Some(terminal_runtime::TerminalClockEnvironment::Host),
        },
        program.image,
    )
}
