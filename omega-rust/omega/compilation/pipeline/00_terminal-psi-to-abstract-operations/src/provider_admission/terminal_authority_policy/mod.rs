//! Optimizer module role: executable entrance. Exact receiving-authority policy construction.

use std::sync::OnceLock;

use effects::TerminalAuthorityPolicyIdentity;

mod classification;
mod commitment;
mod console_host;
mod construction;
mod darwin_clock;
mod filesystem;
mod inventory;
mod model;
mod normalized_foreign;
mod parameterized_intrinsics;
mod syscall;
mod time_host;

pub(crate) use console_host::console_mechanism_row;
pub(crate) use darwin_clock::darwin_clock_mechanism_row;
pub use filesystem::filesystem_host_permission_rows;
pub(crate) use filesystem::filesystem_mechanism_row;
pub use model::TerminalAuthorityPolicy;
pub(crate) use model::{TerminalAuthorityPolicyBuildError, TerminalAuthorityPolicyRow};

pub(crate) use normalized_foreign::{
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
};
pub(crate) use syscall::conservative_syscall_terminal_mechanism;
pub(crate) use time_host::time_host_mechanism_row;
pub use time_host::time_host_permission_rows;

use commitment::complete_policy_commitment;

/// Version of the receiving-realization policy table over D45's shared
/// role-tagged terminal-mechanism identity.
pub(crate) const TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 8;

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
