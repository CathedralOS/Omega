//! Optimizer module role: executable entrance. Exact receiving-authority policy construction.

use std::sync::OnceLock;

use omega_effects::TerminalAuthorityPolicyIdentity;

mod classification;
mod commitment;
mod construction;
mod inventory;
mod model;
mod normalized_foreign;
mod syscall;
#[cfg(test)]
mod tests;

pub use model::{
    TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError, TerminalAuthorityPolicyRow,
    UnclassifiedTerminalMechanism,
};
pub use normalized_foreign::{
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
};
pub(crate) use syscall::conservative_syscall_terminal_mechanism;

use commitment::complete_policy_commitment;

/// Version of the receiving-realization policy table over D45's shared
/// role-tagged terminal-mechanism identity.
pub const TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 7;

pub fn terminal_authority_policy_with_rows(
    explicit_rows: Vec<TerminalAuthorityPolicyRow>,
) -> Result<TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError> {
    construction::build_terminal_authority_policy(explicit_rows)
}

pub fn current_terminal_authority_policy() -> TerminalAuthorityPolicy {
    static IDENTITY: OnceLock<TerminalAuthorityPolicyIdentity> = OnceLock::new();
    TerminalAuthorityPolicy::new(
        *IDENTITY.get_or_init(|| {
            TerminalAuthorityPolicyIdentity::from_parts(
                TERMINAL_AUTHORITY_POLICY_VERSION,
                complete_policy_commitment(&[]),
            )
        }),
        Vec::new(),
    )
}

/// Transitional name retained for callers with no normalized foreign demand.
pub type CompilerIntrinsicTerminalAuthorityPolicy = TerminalAuthorityPolicy;
pub type UnclassifiedCompilerIntrinsicTerminalMechanism = UnclassifiedTerminalMechanism;
pub const COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION: u32 =
    TERMINAL_AUTHORITY_POLICY_VERSION;

pub fn current_compiler_intrinsic_terminal_authority_policy() -> TerminalAuthorityPolicy {
    current_terminal_authority_policy()
}
