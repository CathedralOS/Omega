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

pub use filesystem::filesystem_host_permission_rows;
pub use model::TerminalAuthorityPolicy;
pub(crate) use model::{TerminalAuthorityPolicyBuildError, TerminalAuthorityPolicyRow};

pub(crate) use normalized_foreign::{
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
};
pub(crate) use syscall::conservative_syscall_terminal_mechanism;
pub use time_host::time_host_permission_rows;

use commitment::complete_policy_commitment;
use effects::{
    TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema},
};

/// Version of the receiving-realization policy table over D45's shared
/// role-tagged terminal-mechanism identity.
pub(crate) const TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 9;

pub fn terminal_authority_policy_with_rows(
    explicit_rows: Vec<TerminalAuthorityPolicyRow>,
) -> Result<TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError> {
    construction::build_terminal_authority_policy(explicit_rows)
}

/// One exact receiving-policy row from whichever settled host cohort owns
/// `method`'s requirement, or `None` when no canonical cohort does. The cohort
/// tables decide membership and disposition, so a new cohort joins this list in
/// one place instead of every settlement site re-listing the tables. Minting a
/// row is not admission: the caller still binds the mechanism this leaf
/// computed, and ordinary-release cohorts mint nothing here.
pub(crate) fn cohort_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    filesystem::filesystem_mechanism_row(mechanism, method)
        .or_else(|| time_host::time_host_mechanism_row(mechanism, method))
        .or_else(|| console_host::console_mechanism_row(mechanism, method))
        .or_else(|| darwin_clock::darwin_clock_mechanism_row(mechanism, schema, method))
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
