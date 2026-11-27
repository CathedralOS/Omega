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

// A checked service names the operations its package may request. Native
// mechanism rows describe what the target will do for a particular binding.

#[path = "terminal_authority_policy/classification.rs"]
mod classification;
#[path = "terminal_authority_policy/darwin_poll.rs"]
pub(crate) mod darwin_poll;
#[path = "terminal_authority_policy/external_mechanisms.rs"]
mod external_mechanisms;
#[path = "terminal_authority_policy/host_cohorts.rs"]
mod host_cohorts;
#[path = "terminal_authority_policy/posix_operations.rs"]
mod posix_operations;
pub(crate) use posix_operations::private_linux_syscall_authority;
#[path = "terminal_authority_policy/windows_imports.rs"]
mod windows_imports;

use std::sync::OnceLock;

use crate::digest_encoder::DomainDigestEncoder;
use classification::{
    CLOSED_POLICY_ROW_COUNT, classify_compiler_intrinsic, classify_from_inventory,
    committed_policy_mechanisms,
};
use native_intrinsics::{CompilerAccessorScalarOperation, CompilerIntrinsicExecutionIdentity};
use semantic_vocabulary::{TerminalAuthorityClass, TerminalAuthorityDisposition};
use terminal_authority_receipts::{
    CheckedPhysicalOperationIdentity, NormalizedForeignArgumentContract,
    TerminalAuthorityPolicyIdentity, TerminalMechanismIdentity, terminal_mechanism_identity_bytes,
};

pub(crate) use external_mechanisms::{
    conservative_syscall_terminal_mechanism, normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
};
pub(crate) use host_cohorts::darwin_clock_mechanism_row;
pub use host_cohorts::{filesystem_host_permission_rows, time_host_permission_rows};

pub(crate) const TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 12;
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

    /// The receiver's own rows take precedence. Both provider selections and
    /// direct Build bindings contribute the mechanisms reached by this program.
    pub(crate) fn with_reviewed_operations(
        &self,
        selected: Vec<TerminalAuthorityPolicyRow>,
        reached: impl IntoIterator<Item = TerminalMechanismIdentity>,
    ) -> Result<Option<Self>, TerminalAuthorityPolicyBuildError> {
        let reviewed = selected
            .into_iter()
            .chain(reached.into_iter().filter_map(reviewed_host_operation));
        let mut effective: Option<Vec<TerminalAuthorityPolicyRow>> = None;
        for row in reviewed {
            let rows = effective.as_deref().unwrap_or(&self.explicit_rows);
            if !rows
                .iter()
                .any(|existing| existing.mechanism() == row.mechanism())
            {
                effective
                    .get_or_insert_with(|| self.explicit_rows.clone())
                    .push(row);
            }
        }
        effective
            .map(terminal_authority_policy_with_rows)
            .transpose()
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
                calling, authority, ..
            } => {
                if calling.is_zero() || authority.commitment() == &[0; 32] {
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

pub(crate) fn reviewed_host_operation(
    mechanism: TerminalMechanismIdentity,
) -> Option<TerminalAuthorityPolicyRow> {
    match mechanism {
        TerminalMechanismIdentity::NormalizedForeign(import)
            if import.target().imports_through_pe() =>
        {
            windows_imports::mechanism_row(import)
        }
        // Darwin poll gains its wait classification from checked call arguments.
        TerminalMechanismIdentity::NormalizedForeign(import) if darwin_poll::is_poll(import) => {
            None
        }
        _ => posix_operations::mechanism_row(mechanism),
    }
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
    use numerics::intrinsics::NumericIntrinsic;
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
        hasher.raw(&(numerics::intrinsics::NumericType::ENCODED_SIZE as u32).to_be_bytes());
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
    use native_intrinsics::{
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

#[cfg(test)]
mod interrupt_control_tests {
    use super::super::terminal_authority_permission_policy::{
        TerminalAuthorityPermissionPolicyRow, terminal_authority_permission_policy_with_rows,
    };
    use super::*;
    use language_core::BuiltinFunction;
    use build_providers::plans::{ProviderPlanDigest, ServiceSchemaDigest};
    use build_providers::selections::SelectedProviderPlanFacts;
    use terminal_authority_receipts::{
        TerminalAuthorityClosureLeaf, TerminalAuthorityClosureLeafOrigin,
        TerminalAuthorityClosureReviewReceipt,
    };

    #[test]
    fn mask_intrinsics_have_only_interrupt_control() {
        use BuiltinFunction::*;
        use TerminalAuthorityClass::*;
        let policy = current_terminal_authority_policy();
        for (function, expected) in [
            (AsmDisableInterrupts, Some(InterruptControl)),
            (AsmEnableInterrupts, Some(InterruptControl)),
            (AsmHlt, Some(MachineControl)),
            (AsmRestoreFlags, Some(MachineControl)),
            (AsmWriteCr3, Some(MachineControl)),
            (AsmPortIn, Some(PortIo)),
            (AsmPortOut, Some(PortIo)),
            (AsmFullFence, None),
            (AsmSnapshotFlags, None),
        ] {
            let mechanism = CompilerIntrinsicExecutionIdentity::BuiltinFunction(function);
            assert_eq!(
                policy.classify(mechanism).unwrap().classes(),
                expected.as_slice(),
                "{function:?} under {:?}",
                policy.identity(),
            );
        }
    }

    #[test]
    fn mask_receiving_permissions_use_exact_classes_and_rows() {
        use BuiltinFunction::*;
        use TerminalAuthorityClass::*;
        let physical = current_terminal_authority_policy();
        let mut names = terminal_psi::TerminalIdentityTable::default().into_builder();
        let requirement = names.intern("Mask::update");
        let names = names.build();
        let schema = ServiceSchemaDigest::from_digest([1; 32]);
        let plan = ProviderPlanDigest::from_digest([2; 32]);
        for (function, allowed, missing) in [
            (AsmDisableInterrupts, vec![InterruptControl], None),
            (AsmEnableInterrupts, vec![InterruptControl], None),
            (
                AsmDisableInterrupts,
                vec![MachineControl],
                Some(InterruptControl),
            ),
            (AsmDisableInterrupts, vec![], Some(InterruptControl)),
            (
                AsmDisableInterrupts,
                vec![MachineControl, InterruptControl],
                None,
            ),
            (AsmWriteCr3, vec![InterruptControl], Some(MachineControl)),
        ] {
            let permissions = terminal_authority_permission_policy_with_rows(
                vec![TerminalAuthorityPermissionPolicyRow::new(
                    schema,
                    requirement,
                    TerminalAuthorityDisposition::from_classes(allowed),
                )],
                names.clone(),
            )
            .unwrap();
            assert!(
                permissions
                    .permission_for(schema, "OtherMask::update")
                    .is_err()
            );
            assert!(
                permissions
                    .permission_for(ServiceSchemaDigest::from_digest([9; 32]), "Mask::update",)
                    .is_err()
            );
            let mechanism = CompilerIntrinsicExecutionIdentity::BuiltinFunction(function);
            let leaf = TerminalAuthorityClosureLeaf::new(
                schema,
                requirement,
                requirement,
                plan,
                mechanism.into(),
                TerminalAuthorityClosureLeafOrigin::SelectedBoundary,
                Some(physical.classify(mechanism).unwrap()),
                Some(permissions.permission_for(schema, "Mask::update").unwrap()),
            );
            if let Some(class) = missing {
                assert_eq!(
                    format!("{:?}", leaf.unwrap_err()),
                    format!("ExercisedAuthorityNotPermitted {{ class: {class:?} }}")
                );
            } else {
                TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
                    [3; 32],
                    target::TargetProfile::LinuxX64.native_target(),
                    SelectedProviderPlanFacts::default().identity_digest(),
                    physical.identity(),
                    Some(permissions.identity()),
                    vec![leaf.unwrap()],
                    &names,
                )
                .unwrap();
            }
        }
        let absent = terminal_authority_permission_policy_with_rows(vec![], names.clone()).unwrap();
        assert!(absent.permission_for(schema, "Mask::update").is_err());
        // Classification without a requested receiver policy grants no receiving admission.
        let mechanism = CompilerIntrinsicExecutionIdentity::BuiltinFunction(AsmDisableInterrupts);
        let classified = TerminalAuthorityClosureLeaf::new(
            schema,
            requirement,
            requirement,
            plan,
            mechanism.into(),
            TerminalAuthorityClosureLeafOrigin::SelectedBoundary,
            Some(physical.classify(mechanism).unwrap()),
            None,
        )
        .unwrap();
        TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
            [3; 32],
            target::TargetProfile::LinuxX64.native_target(),
            SelectedProviderPlanFacts::default().identity_digest(),
            physical.identity(),
            None,
            vec![classified],
            &names,
        )
        .unwrap();
    }

    #[test]
    fn mask_compiler_rows_cannot_be_overridden() {
        for function in [
            BuiltinFunction::AsmDisableInterrupts,
            BuiltinFunction::AsmEnableInterrupts,
        ] {
            let intrinsic = CompilerIntrinsicExecutionIdentity::BuiltinFunction(function);
            let result =
                terminal_authority_policy_with_rows(vec![TerminalAuthorityPolicyRow::new(
                    intrinsic.into(),
                    TerminalAuthorityDisposition::from_classes([]),
                )]);
            assert!(matches!(result,
                Err(TerminalAuthorityPolicyBuildError::CompilerIntrinsicRowIsReserved(actual))
                if actual == intrinsic));
        }
    }

    #[test]
    fn mask_policy_epoch_and_commitment_bind_reviewed_identity() {
        let policy = current_terminal_authority_policy();
        let identity = policy.identity();
        assert_eq!(identity.version(), 12);
        assert_eq!(
            identity,
            terminal_authority_policy_with_rows(vec![])
                .unwrap()
                .identity()
        );
        let review = |physical| {
            TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
                [3; 32],
                target::TargetProfile::LinuxX64.native_target(),
                SelectedProviderPlanFacts::default().identity_digest(),
                physical,
                None,
                vec![],
                &terminal_psi::TerminalIdentityTable::default(),
            )
            .unwrap()
        };
        let current = review(identity);
        let old_epoch = TerminalAuthorityPolicyIdentity::from_parts(11, identity.commitment());
        assert_ne!(current.identity(), review(old_epoch).identity());
        let mut changed = identity.commitment();
        changed[0] ^= 1;
        let wrong_commitment = TerminalAuthorityPolicyIdentity::from_parts(12, changed);
        assert_ne!(current.identity(), review(wrong_commitment).identity());
    }
}
