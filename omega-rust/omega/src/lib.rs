//! Complete application operations, usable without CLI parsing or process exits.
//!
//! Package operations remain owned by `crate::package_manager::operations`. Compiler
//! stages remain ordinary functions below these project-level workflows.

pub mod artifacts;
pub mod backend_plan;
pub mod bounded_process;
pub mod build_declarations;
pub mod build_evaluation;
pub mod build_output;
pub mod compilation;
pub mod compiler;
pub mod component_candidate;
pub mod component_deployment;
pub mod component_description;
pub mod execution;
pub mod inspection;
pub mod package_compilation;
pub mod package_evidence;
pub mod package_manager;
pub mod package_source;
pub mod platform_custody;
pub mod provider_planning;
pub mod representation_planning;
pub mod resolver_execution;
pub mod selected_dispatch;
pub mod topology_plan;
pub mod trust_ledger;
pub mod trust_model;

mod temporary_directory;

use diagnostics::Diagnostic;

/// Resolve the deployment profile one project invocation selected: an
/// explicit target name parses exactly, and an absent name admits the
/// compiler host's catalogued profile. A host that owns none (macOS
/// x86-64, for one) earns an ordinary diagnostic naming the accepted
/// profiles rather than the panic `TargetProfile::host` retains for
/// callers that intrinsically require the host profile.
pub(crate) fn invocation_target_profile(
    target_name: Option<&str>,
) -> Result<target::TargetProfile, Diagnostic> {
    match target_name {
        Some(name) => target::TargetProfile::from_omega_target_name(Some(name)),
        None => target::TargetProfile::host_if_supported().ok_or_else(|| {
            Diagnostic::error(
                "no target was named and this host has no catalogued Omega deployment \
                 profile; name an exact target (linux_arm64, linux_x86_64, macos_arm64, \
                 macos_x86_64, windows_x86_64, uefi_x86_64, cross_platform_cli, \
                 local_unchecked, or alpha_bootstrap)",
            )
        }),
    }
}
pub mod build_time_evaluation;
pub mod checked_interpreter;
pub mod component_publication;
pub mod executable_installation;
pub mod external_roots;
#[cfg(feature = "installed-writer")]
pub mod post_handoff_writer;
pub mod terminal_fixed_fuel;
