//! Complete application operations, usable without CLI parsing or process exits.
//!
//! Package operations remain owned by `package_manager::operations`. Compiler
//! stages remain ordinary functions below these project-level workflows.

pub mod compilation;
pub mod execution;
pub mod inspection;

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
