/*
Receiving-realization terminal-authority policy construction and classification.

`TerminalAuthorityPolicy` classifies role-tagged terminal mechanisms across the
closed compiler intrinsic inventory, committed width-indexed numeric families, and the caller-RAM
scalar access family (`Read`/`Write` with nonzero contract and placement
commitments), and explicit normalized-foreign, table, syscall, or
checked-physical rows.

Compiler-intrinsic rows are reserved and cannot be overridden in `explicit_rows`,
duplicate physical identities reject, and empty implementation or checked
argument contracts are never valid policy keys.
*/

/*
Cohort tables (`host_cohorts`) decide membership and disposition for canonical
settled host boundaries (`FilesystemHost`, `TimeHost`, `Console`, `ProcessExit`,
`DarwinClock`) in one place so settlement sites do not re-list those tables.
Minting a policy row is not admission: the caller still binds the mechanism the
leaf computed, and ordinary-release cohorts mint nothing without an
occurrence-specific release contract.
*/

mod classification;
mod external_mechanisms;
mod host_cohorts;
mod windows_imports;

use std::sync::OnceLock;

use crate::digest_encoder::DomainDigestEncoder;
use classification::{
    CLOSED_POLICY_ROW_COUNT, classify_compiler_intrinsic, classify_from_inventory,
    committed_policy_mechanisms,
};
use effects::{
    CheckedPhysicalOperationIdentity, CompilerAccessorScalarOperation,
    CompilerIntrinsicExecutionIdentity, NormalizedForeignArgumentContract, TerminalAuthorityClass,
    TerminalAuthorityDisposition, TerminalAuthorityPolicyIdentity, TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema},
    terminal_mechanism_identity_bytes,
};

pub(crate) use external_mechanisms::{
    conservative_syscall_terminal_mechanism, normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
};
pub use host_cohorts::{filesystem_host_permission_rows, time_host_permission_rows};

pub(crate) const TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 11;
const POLICY_COMMITMENT_DOMAIN: &[u8] = b"omega.terminal-authority.policy.v3\0";
const PARAMETERIZED_POLICY_FAMILY_COUNT: u32 = 3;
const ACCESS_CALLER_RAM_SCALAR_POLICY_V1: &[u8] =
    b"omega.compiler-policy.placed-caller-ram-scalar.v1\0";
const INTEGER_COMPARISON_POLICY_V1: &[u8] = b"omega.compiler-policy.integer-width-comparison.v1\0";
const FLOAT_CONVERSION_POLICY_V1: &[u8] =
    b"omega.compiler-policy.integer-width-float-conversion.v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPolicyRow {
    pub(super) mechanism: TerminalMechanismIdentity,
    pub(super) disposition: TerminalAuthorityDisposition,
}

impl TerminalAuthorityPolicyRow {
    pub fn new(
        mechanism: TerminalMechanismIdentity,
        disposition: TerminalAuthorityDisposition,
    ) -> Self {
        Self {
            mechanism,
            disposition,
        }
    }

    pub const fn mechanism(&self) -> TerminalMechanismIdentity {
        self.mechanism
    }

    pub const fn disposition(&self) -> &TerminalAuthorityDisposition {
        &self.disposition
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPolicy {
    identity: TerminalAuthorityPolicyIdentity,
    explicit_rows: Vec<TerminalAuthorityPolicyRow>,
}

impl TerminalAuthorityPolicy {
    pub(super) const fn new(
        identity: TerminalAuthorityPolicyIdentity,
        explicit_rows: Vec<TerminalAuthorityPolicyRow>,
    ) -> Self {
        Self {
            identity,
            explicit_rows,
        }
    }

    pub const fn identity(&self) -> TerminalAuthorityPolicyIdentity {
        self.identity
    }

    pub(crate) fn explicit_rows(&self) -> &[TerminalAuthorityPolicyRow] {
        &self.explicit_rows
    }

    pub fn classify(
        &self,
        mechanism: impl Into<TerminalMechanismIdentity>,
    ) -> Result<TerminalAuthorityDisposition, UnclassifiedTerminalMechanism> {
        let mechanism = mechanism.into();
        match mechanism {
            TerminalMechanismIdentity::CompilerIntrinsic(intrinsic) => {
                if let Some(disposition) = classify_parameterized_intrinsic(intrinsic) {
                    Ok(disposition)
                } else {
                    classify_from_inventory(committed_policy_mechanisms(), intrinsic)
                }
            }
            TerminalMechanismIdentity::NormalizedForeign(_)
            | TerminalMechanismIdentity::Table { .. }
            | TerminalMechanismIdentity::Syscall(_)
            | TerminalMechanismIdentity::CheckedPhysical(_) => self
                .explicit_rows
                .iter()
                .find(|row| row.mechanism == mechanism)
                .map(|row| row.disposition.clone())
                .ok_or(UnclassifiedTerminalMechanism { mechanism }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnclassifiedTerminalMechanism {
    pub(super) mechanism: TerminalMechanismIdentity,
}

impl UnclassifiedTerminalMechanism {
    pub const fn mechanism(self) -> TerminalMechanismIdentity {
        self.mechanism
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAuthorityPolicyBuildError {
    CompilerIntrinsicRowIsReserved(CompilerIntrinsicExecutionIdentity),
    EmptyImplementationContract(TerminalMechanismIdentity),
    UnsupportedSyscallTarget(TerminalMechanismIdentity),
    EmptyCheckedSyscallArgumentContract(TerminalMechanismIdentity),
    EmptyCheckedForeignArgumentContract(TerminalMechanismIdentity),
    CheckedPortWriteRequiresExactPortIo(TerminalMechanismIdentity),
    DuplicateMechanism(TerminalMechanismIdentity),
}

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
            TerminalMechanismIdentity::NormalizedForeign(foreign)
                if matches!(
                    foreign.argument_contract(),
                    NormalizedForeignArgumentContract::Checked(contract) if contract.is_zero()
                ) =>
            {
                return Err(
                    TerminalAuthorityPolicyBuildError::EmptyCheckedForeignArgumentContract(
                        row.mechanism,
                    ),
                );
            }
            TerminalMechanismIdentity::NormalizedForeign(_) => {}
            TerminalMechanismIdentity::Table {
                calling, provider, ..
            } => {
                if calling.is_zero() || provider.as_bytes() == &[0; 32] {
                    return Err(
                        TerminalAuthorityPolicyBuildError::EmptyImplementationContract(
                            row.mechanism,
                        ),
                    );
                }
            }
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

pub(crate) fn cohort_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    // Windows foreign imports have their own reviewed physical meanings.
    // A readable portable/Console/Time method name cannot classify an unknown
    // export or replace an import's complete admitted argument contract.
    if let TerminalMechanismIdentity::NormalizedForeign(foreign) = mechanism
        && foreign.target() == target::TargetProfile::WindowsX64
    {
        return windows_imports::mechanism_row(foreign);
    }
    host_cohorts::filesystem_mechanism_row(mechanism, method)
        .or_else(|| host_cohorts::time_host_mechanism_row(mechanism, method))
        .or_else(|| host_cohorts::console_mechanism_row(mechanism, method))
        .or_else(|| host_cohorts::darwin_clock_mechanism_row(mechanism, schema, method))
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

fn classify_parameterized_intrinsic(
    mechanism: CompilerIntrinsicExecutionIdentity,
) -> Option<TerminalAuthorityDisposition> {
    use language_semantics::numeric_intrinsics::NumericIntrinsic;
    match mechanism {
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
            operation,
            integer_type,
        } if (NumericIntrinsic::IntegerComparison {
            operation,
            integer_type,
        })
        .signature()
        .is_some() =>
        {
            Some(TerminalAuthorityDisposition::from_classes([]))
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } if (NumericIntrinsic::Conversion {
            source,
            target,
            domain,
        })
        .signature()
        .is_some() =>
        {
            Some(TerminalAuthorityDisposition::from_classes([]))
        }
        CompilerIntrinsicExecutionIdentity::AccessorScalarAccess {
            contract,
            placement_commitment,
            operation:
                CompilerAccessorScalarOperation::Read | CompilerAccessorScalarOperation::Write,
            ..
        } if contract != [0; 32] && placement_commitment != [0; 32] => {
            Some(TerminalAuthorityDisposition::from_classes([]))
        }
        _ => None,
    }
}

fn encode_parameterized_commitment(hasher: &mut DomainDigestEncoder) {
    hasher.raw(&(ACCESS_CALLER_RAM_SCALAR_POLICY_V1.len() as u32).to_be_bytes());
    hasher.raw(ACCESS_CALLER_RAM_SCALAR_POLICY_V1);
    hasher.raw(&[32, 32, 4, 2, 0, 2]);
    hasher.raw(&0_u32.to_be_bytes());
    // Numeric families classify exact checked signatures; arbitrary widths do not
    // imply that a target can execute them or supply any object authority.
    for family in [INTEGER_COMPARISON_POLICY_V1, FLOAT_CONVERSION_POLICY_V1] {
        hasher.raw(&(family.len() as u32).to_be_bytes());
        hasher.raw(family);
        hasher.raw(
            &(language_semantics::numeric_intrinsics::NumericType::ENCODED_SIZE as u32)
                .to_be_bytes(),
        );
        hasher.raw(&0_u32.to_be_bytes());
    }
}

fn complete_policy_commitment(explicit_rows: &[TerminalAuthorityPolicyRow]) -> [u8; 32] {
    let mut hasher = DomainDigestEncoder::new(POLICY_COMMITMENT_DOMAIN);
    hasher.raw(&TERMINAL_AUTHORITY_POLICY_VERSION.to_be_bytes());
    hasher.raw(
        &(CLOSED_POLICY_ROW_COUNT
            + PARAMETERIZED_POLICY_FAMILY_COUNT
            + u32::try_from(explicit_rows.len()).expect("policy row count fits u32"))
        .to_be_bytes(),
    );
    for &mechanism in committed_policy_mechanisms() {
        encode_mechanism(&mut hasher, mechanism.into());
        encode_disposition(&mut hasher, &classify_compiler_intrinsic(mechanism));
    }
    encode_parameterized_commitment(&mut hasher);
    for row in explicit_rows {
        encode_mechanism(&mut hasher, row.mechanism);
        encode_disposition(&mut hasher, &row.disposition);
    }
    hasher.finish()
}

fn encode_disposition(
    hasher: &mut DomainDigestEncoder,
    disposition: &TerminalAuthorityDisposition,
) {
    hasher.raw(
        &u32::try_from(disposition.classes().len())
            .expect("terminal-authority class count fits u32")
            .to_be_bytes(),
    );
    for class in disposition.classes() {
        hasher.byte(class.canonical_tag());
    }
}

fn encode_mechanism(hasher: &mut DomainDigestEncoder, mechanism: TerminalMechanismIdentity) {
    let bytes = terminal_mechanism_identity_bytes(mechanism);
    hasher.raw(
        &u32::try_from(bytes.len())
            .expect("terminal-mechanism identity length fits u32")
            .to_be_bytes(),
    );
    hasher.raw(&bytes);
}

#[cfg(test)]
mod integer_width_tests {
    use super::*;
    use effects::{
        CompilerNumericType as Numeric, CompilerPrimitiveIntegerComparisonOperation as Comparison,
    };
    use numerics::{arithmetic::ArithmeticDomain, integer_carrier::FixedInteger};

    #[test]
    fn integer_width_families_classify_only_valid_numeric_meanings() {
        let policy = current_terminal_authority_policy();
        for width in [1, 13, 257, u64::MAX] {
            for integer in [FixedInteger::signed(width), FixedInteger::unsigned(width)] {
                for mechanism in [
                    CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                        operation: Comparison::Less,
                        integer_type: Numeric::Integer(integer),
                    },
                    CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                        source: Numeric::F64,
                        target: Numeric::Integer(integer),
                        domain: ArithmeticDomain::Saturating,
                    },
                ] {
                    assert!(policy.classify(mechanism).unwrap().classes().is_empty());
                }
            }
        }
        for mechanism in [
            CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                operation: Comparison::Less,
                integer_type: Numeric::F32,
            },
            CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                source: Numeric::F64,
                target: Numeric::Integer(FixedInteger::unsigned(13)),
                domain: ArithmeticDomain::Wrapping,
            },
            CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                source: Numeric::F32,
                target: Numeric::F32,
                domain: ArithmeticDomain::Exact,
            },
        ] {
            assert!(policy.classify(mechanism).is_err());
        }
    }
}
