//! Optimizer module role: executable entrance. Exact receiving-authority policy construction.

use std::sync::OnceLock;

use omega_effects::{
    CheckedPhysicalOperationIdentity, TerminalAuthorityClass, TerminalAuthorityPolicyIdentity,
    TerminalMechanismIdentity, terminal_mechanism_identity_bytes,
};

mod classification;
mod commitment;
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
pub const TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 5;

/// Build one accepted receiving policy from explicit exact non-intrinsic rows.
/// Compiler-intrinsic rows cannot be overridden, duplicate physical identities
/// reject, and a normalized foreign row's empty strong implementation contract
/// is never a policy key.
pub fn terminal_authority_policy_with_rows(
    mut explicit_rows: Vec<TerminalAuthorityPolicyRow>,
) -> Result<TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError> {
    for row in &explicit_rows {
        match row.mechanism {
            TerminalMechanismIdentity::CompilerIntrinsic(intrinsic) => {
                return Err(
                    TerminalAuthorityPolicyBuildError::CompilerIntrinsicRowIsReserved(intrinsic),
                );
            }
            TerminalMechanismIdentity::NormalizedForeign(foreign)
                if foreign.implementation_contract().is_zero() =>
            {
                return Err(
                    TerminalAuthorityPolicyBuildError::EmptyImplementationContract(row.mechanism),
                );
            }
            TerminalMechanismIdentity::NormalizedForeign(_) => {}
            TerminalMechanismIdentity::Syscall(syscall)
                if !matches!(
                    syscall.target(),
                    omega_target::TargetProfile::LinuxX64 | omega_target::TargetProfile::LinuxArm64
                ) =>
            {
                return Err(TerminalAuthorityPolicyBuildError::UnsupportedSyscallTarget(
                    row.mechanism,
                ));
            }
            TerminalMechanismIdentity::Syscall(syscall)
                if syscall.checked_argument_contract().is_zero() =>
            {
                return Err(
                    TerminalAuthorityPolicyBuildError::EmptyCheckedSyscallArgumentContract(
                        row.mechanism,
                    ),
                );
            }
            TerminalMechanismIdentity::Syscall(_) => {}
            TerminalMechanismIdentity::CheckedPhysical(physical) => match physical.operation() {
                CheckedPhysicalOperationIdentity::PortWrite { .. }
                    if row.disposition.classes() != [TerminalAuthorityClass::PortIo] =>
                {
                    return Err(
                        TerminalAuthorityPolicyBuildError::CheckedPortWriteRequiresExactPortIo(
                            row.mechanism,
                        ),
                    );
                }
                CheckedPhysicalOperationIdentity::PortWrite { .. } => {}
            },
        }
    }
    explicit_rows.sort_by_key(|row| terminal_mechanism_identity_bytes(row.mechanism));
    if let Some(duplicate) = explicit_rows
        .windows(2)
        .find(|rows| rows[0].mechanism == rows[1].mechanism)
        .map(|rows| rows[0].mechanism)
    {
        return Err(TerminalAuthorityPolicyBuildError::DuplicateMechanism(
            duplicate,
        ));
    }
    let identity = TerminalAuthorityPolicyIdentity::from_parts(
        TERMINAL_AUTHORITY_POLICY_VERSION,
        complete_policy_commitment(&explicit_rows),
    );
    Ok(TerminalAuthorityPolicy::new(identity, explicit_rows))
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
