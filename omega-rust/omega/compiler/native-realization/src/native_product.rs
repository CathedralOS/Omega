//! Native product preparation and realization after checked-Psi admission.

mod admission;
mod input_reuse;
mod prepared;
mod realization;
mod receipt;
pub use input_reuse::NativeInputReuse;
#[cfg(any(test, feature = "test-support"))]
pub use prepared::NativeInputReuseKey;
pub use prepared::PreparedNativeCompilation;

use assembled_syntax_to_checked_compilation::{CheckedCompilation, OptimizationRollback};
use diagnostics::Diagnostic;

/// One target's native product request: where the source root is, the
/// admission profile the Terminal artifact is verified under, the receiving
/// mechanism-classification policy, the optional service-permission admission
/// policy, and the optimization rollback to settle.
///
/// The terminal-authority policy is the receiving authority's explicit
/// mechanism table: the closed compiler-intrinsic inventory classifies itself,
/// while every demanded normalized-foreign, syscall, or checked-physical leaf
/// needs exactly one exact explicit row. It is an independent authority axis --
/// trust admissions, service permissions, and provider execution custody
/// never substitute for or widen its rows.
///
/// `terminal_authority_permission_policy` is the second, receiver-admission
/// axis. `None` means ordinary production makes no receiver-admission claim:
/// accepted package permissions are never projected into receiver rows, and
/// the emitted artifact cannot satisfy an explicit admission replay. `Some`
/// (including an explicit empty policy) adjudicates every closure leaf
/// against exact supplied rows.
pub struct NativeProductRequest {
    pub root_path: std::path::PathBuf,
    pub terminal_admission_profile: proof_admission::AdmissionProfile,
    pub terminal_authority_policy: crate::TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: Option<crate::TerminalAuthorityPermissionPolicy>,
    pub optimization_rollback: OptimizationRollback,
}

/// Admit one checked compilation for native production and produce its
/// program-entry Terminal artifact. Physical realization follows through
/// [`NativeInputReuse`], which prepares each exact Terminal input once.
pub fn prepare_native_product(
    request: NativeProductRequest,
    checked: CheckedCompilation,
) -> Result<PreparedNativeCompilation, Vec<Diagnostic>> {
    admission::reject_unconsumed_callbacks(&checked)?;
    let production_subject = checked.production_subject()?;
    let source_file_count = checked.source_file_count();
    let admission = admission::admit(&checked)?;
    let rollback = request
        .optimization_rollback
        .settle(checked.optimization_selections());
    realization::validate_terminal_authority_permissions(
        &checked,
        request.terminal_authority_permission_policy.as_ref(),
    )?;
    let terminal =
        checked_compilation_to_terminal_artifact::produce_program_entry_terminal_artifact(
            &checked,
            &admission.program_entry,
            rollback.effective(),
        )?;
    Ok(PreparedNativeCompilation::new(
        request,
        checked,
        admission,
        rollback,
        terminal,
        production_subject,
        source_file_count,
    ))
}
