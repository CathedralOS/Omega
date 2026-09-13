//! Exact construction and validation of receiving-authority policy rows.

use effects::{
    CheckedPhysicalOperationIdentity, TerminalAuthorityClass, TerminalAuthorityPolicyIdentity,
    TerminalMechanismIdentity, terminal_mechanism_identity_bytes,
};

use super::commitment::complete_policy_commitment;
use super::{
    TERMINAL_AUTHORITY_POLICY_VERSION, TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError,
    TerminalAuthorityPolicyRow,
};

/// Build one accepted receiving policy from explicit exact non-intrinsic rows.
/// Compiler-intrinsic rows cannot be overridden, duplicate physical identities
/// reject, and a normalized foreign row's empty strong implementation contract
/// is never a policy key.
pub(super) fn build_terminal_authority_policy(
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
                    target::TargetProfile::LinuxX64 | target::TargetProfile::LinuxArm64
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
