use omega_effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    TerminalAuthorityClass, TerminalAuthorityDisposition, TerminalAuthorityPolicyIdentity,
    TerminalMechanismIdentity, terminal_mechanism_identity_bytes,
};
use psi_numerics::{arithmetic::ArithmeticDomain, literals::FloatFormat};
use psi_symbols::BuiltinFunction;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

/// Version of the receiving-realization policy table over D45's shared
/// role-tagged terminal-mechanism identity.
pub const TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 2;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"omega.terminal-authority.policy.v2\0";
const CLOSED_POLICY_ROW_COUNT: u32 = 494;
const FLOAT_FORMATS: [FloatFormat; 2] = [FloatFormat::F32, FloatFormat::F64];
const ARITHMETIC_DOMAINS: [ArithmeticDomain; 4] = [
    ArithmeticDomain::Exact,
    ArithmeticDomain::Wrapping,
    ArithmeticDomain::Saturating,
    ArithmeticDomain::Trapping,
];

/// One explicit receiving-policy row outside the compiler-owned intrinsic
/// inventory. An empty disposition remains an authored row, never a default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPolicyRow {
    mechanism: TerminalMechanismIdentity,
    disposition: TerminalAuthorityDisposition,
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

/// Current receiving-realization policy. The closed 494-row compiler-
/// intrinsic inventory is always present; exact foreign rows are explicitly
/// supplied by the receiving authority. This table classifies physical
/// authority only. It does not yet traverse selected provider closures or join
/// service/schema permissions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPolicy {
    identity: TerminalAuthorityPolicyIdentity,
    explicit_rows: Vec<TerminalAuthorityPolicyRow>,
}

/// The requested closed mechanism has no row in this exact policy version.
/// This is distinct from a committed row with an empty class disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnclassifiedTerminalMechanism {
    mechanism: TerminalMechanismIdentity,
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
    DuplicateMechanism(TerminalMechanismIdentity),
}

impl TerminalAuthorityPolicy {
    pub const fn identity(&self) -> TerminalAuthorityPolicyIdentity {
        self.identity
    }

    pub fn explicit_rows(&self) -> &[TerminalAuthorityPolicyRow] {
        &self.explicit_rows
    }

    /// Classify one exact role-tagged mechanism. Compiler intrinsics use the
    /// exhaustive closed inventory; all other roles require one exact row.
    pub fn classify(
        &self,
        mechanism: impl Into<TerminalMechanismIdentity>,
    ) -> Result<TerminalAuthorityDisposition, UnclassifiedTerminalMechanism> {
        let mechanism = mechanism.into();
        match mechanism {
            TerminalMechanismIdentity::CompilerIntrinsic(intrinsic) => {
                classify_from_inventory(committed_policy_mechanisms(), intrinsic)
            }
            TerminalMechanismIdentity::NormalizedForeign(_) => self
                .explicit_rows
                .iter()
                .find(|row| row.mechanism == mechanism)
                .map(|row| row.disposition.clone())
                .ok_or(UnclassifiedTerminalMechanism { mechanism }),
        }
    }
}

/// Build one accepted receiving policy from explicit exact foreign rows.
/// Compiler-intrinsic rows cannot be overridden, duplicate physical identities
/// reject, and an empty strong implementation contract is never policy key.
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
    Ok(TerminalAuthorityPolicy {
        identity,
        explicit_rows,
    })
}

pub fn current_terminal_authority_policy() -> TerminalAuthorityPolicy {
    static IDENTITY: OnceLock<TerminalAuthorityPolicyIdentity> = OnceLock::new();
    TerminalAuthorityPolicy {
        identity: *IDENTITY.get_or_init(|| {
            TerminalAuthorityPolicyIdentity::from_parts(
                TERMINAL_AUTHORITY_POLICY_VERSION,
                complete_policy_commitment(&[]),
            )
        }),
        explicit_rows: Vec::new(),
    }
}

/// Transitional name retained for callers with no normalized foreign demand.
pub type CompilerIntrinsicTerminalAuthorityPolicy = TerminalAuthorityPolicy;
pub type UnclassifiedCompilerIntrinsicTerminalMechanism = UnclassifiedTerminalMechanism;
pub const COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION: u32 =
    TERMINAL_AUTHORITY_POLICY_VERSION;

pub fn current_compiler_intrinsic_terminal_authority_policy() -> TerminalAuthorityPolicy {
    current_terminal_authority_policy()
}

/// Reconstruct the exact normalized-foreign role from the retained locator and
/// canonical admitted boundary plan. The plan is revalidated and must already
/// be canonical; its domain-separated strong contract digest, never its report
/// fingerprint, enters mechanism identity.
pub fn normalized_foreign_terminal_mechanism(
    locator: &omega_target::NormalizedForeignLocator,
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
) -> Result<TerminalMechanismIdentity, String> {
    let signature = omega_calling_conventions::CallSignature {
        parameters: boundary_entry_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: boundary_entry_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    if validated.plan() != boundary_entry_plan {
        return Err("normalized foreign boundary plan is not canonical".to_owned());
    }
    Ok(
        omega_effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            locator,
            omega_effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                validated.contract_commitment_digest(),
            ),
        )
        .into(),
    )
}

fn classify_compiler_intrinsic(
    mechanism: CompilerIntrinsicExecutionIdentity,
) -> TerminalAuthorityDisposition {
    match mechanism {
        CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => {
            disposition([TerminalAuthorityClass::ProcessTermination])
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            classify_builtin_function(function)
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            classify_primitive_float_binary(operation, format)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            classify_named_float_negation(format)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => classify_named_float_conversion(source, target, domain),
    }
}

fn classify_from_inventory(
    inventory: &[CompilerIntrinsicExecutionIdentity],
    mechanism: CompilerIntrinsicExecutionIdentity,
) -> Result<TerminalAuthorityDisposition, UnclassifiedTerminalMechanism> {
    if !inventory.contains(&mechanism) {
        return Err(UnclassifiedTerminalMechanism {
            mechanism: mechanism.into(),
        });
    }
    Ok(classify_compiler_intrinsic(mechanism))
}

fn classify_primitive_float_binary(
    operation: CompilerPrimitiveFloatBinaryOperation,
    format: FloatFormat,
) -> TerminalAuthorityDisposition {
    let _closed_coordinates = (
        primitive_float_operation_tag(operation),
        float_format_tag(format),
    );
    empty_disposition()
}

fn classify_named_float_negation(format: FloatFormat) -> TerminalAuthorityDisposition {
    let _closed_format = float_format_tag(format);
    empty_disposition()
}

fn classify_named_float_conversion(
    source: CompilerNumericType,
    target: CompilerNumericType,
    domain: ArithmeticDomain,
) -> TerminalAuthorityDisposition {
    let _closed_coordinates = (
        numeric_type_tag(source),
        numeric_type_tag(target),
        arithmetic_domain_tag(domain),
    );
    empty_disposition()
}

fn classify_builtin_function(function: BuiltinFunction) -> TerminalAuthorityDisposition {
    match function {
        BuiltinFunction::AsmHlt
        | BuiltinFunction::AsmDisableInterrupts
        | BuiltinFunction::AsmEnableInterrupts
        | BuiltinFunction::AsmRestoreFlags
        | BuiltinFunction::AsmReadMsr
        | BuiltinFunction::AsmWriteMsr
        | BuiltinFunction::AsmReadCr0
        | BuiltinFunction::AsmReadCr2
        | BuiltinFunction::AsmReadCr3
        | BuiltinFunction::AsmReadCr4
        | BuiltinFunction::AsmWriteCr0
        | BuiltinFunction::AsmWriteCr3
        | BuiltinFunction::AsmWriteCr4 => disposition([TerminalAuthorityClass::MachineControl]),
        BuiltinFunction::AsmPortOut | BuiltinFunction::AsmPortIn => {
            disposition([TerminalAuthorityClass::PortIo])
        }
        BuiltinFunction::Max
        | BuiltinFunction::Min
        | BuiltinFunction::Sqrt
        | BuiltinFunction::AsmLoadFence
        | BuiltinFunction::AsmStoreFence
        | BuiltinFunction::AsmFullFence
        | BuiltinFunction::AsmSnapshotFlags
        | BuiltinFunction::FloatIsNan
        | BuiltinFunction::FloatMultiplyThenAddF32
        | BuiltinFunction::FloatMultiplyThenAddF64
        | BuiltinFunction::FloatFusedMultiplyAddF32
        | BuiltinFunction::FloatFusedMultiplyAddF64
        | BuiltinFunction::FloatIsFinite
        | BuiltinFunction::FloatIsInfinite
        | BuiltinFunction::FloatIsNormal
        | BuiltinFunction::FloatIsSubnormal
        | BuiltinFunction::FloatClassifyF32
        | BuiltinFunction::FloatClassifyF64
        | BuiltinFunction::ContentOld
        | BuiltinFunction::ContentSeparate
        | BuiltinFunction::FloatAddTowardZeroF32
        | BuiltinFunction::FloatAddTowardZeroF64
        | BuiltinFunction::FloatAddTowardPositiveF32
        | BuiltinFunction::FloatAddTowardPositiveF64
        | BuiltinFunction::FloatAddTowardNegativeF32
        | BuiltinFunction::FloatAddTowardNegativeF64
        | BuiltinFunction::FloatSubtractTowardZeroF32
        | BuiltinFunction::FloatSubtractTowardZeroF64
        | BuiltinFunction::FloatSubtractTowardPositiveF32
        | BuiltinFunction::FloatSubtractTowardPositiveF64
        | BuiltinFunction::FloatSubtractTowardNegativeF32
        | BuiltinFunction::FloatSubtractTowardNegativeF64
        | BuiltinFunction::FloatMultiplyTowardZeroF32
        | BuiltinFunction::FloatMultiplyTowardZeroF64
        | BuiltinFunction::FloatMultiplyTowardPositiveF32
        | BuiltinFunction::FloatMultiplyTowardPositiveF64
        | BuiltinFunction::FloatMultiplyTowardNegativeF32
        | BuiltinFunction::FloatMultiplyTowardNegativeF64
        | BuiltinFunction::FloatDivideTowardZeroF32
        | BuiltinFunction::FloatDivideTowardZeroF64
        | BuiltinFunction::FloatDivideTowardPositiveF32
        | BuiltinFunction::FloatDivideTowardPositiveF64
        | BuiltinFunction::FloatDivideTowardNegativeF32
        | BuiltinFunction::FloatDivideTowardNegativeF64
        | BuiltinFunction::FloatSqrtTowardZeroF32
        | BuiltinFunction::FloatSqrtTowardZeroF64
        | BuiltinFunction::FloatSqrtTowardPositiveF32
        | BuiltinFunction::FloatSqrtTowardPositiveF64
        | BuiltinFunction::FloatSqrtTowardNegativeF32
        | BuiltinFunction::FloatSqrtTowardNegativeF64
        | BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32
        | BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64
        | BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32
        | BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64
        | BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32
        | BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64 => empty_disposition(),
    }
}

fn disposition<const N: usize>(
    classes: [TerminalAuthorityClass; N],
) -> TerminalAuthorityDisposition {
    TerminalAuthorityDisposition::from_classes(classes)
}

fn empty_disposition() -> TerminalAuthorityDisposition {
    disposition([])
}

fn complete_policy_commitment(explicit_rows: &[TerminalAuthorityPolicyRow]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_COMMITMENT_DOMAIN);
    hasher.update(TERMINAL_AUTHORITY_POLICY_VERSION.to_be_bytes());
    hasher.update(
        (CLOSED_POLICY_ROW_COUNT
            + u32::try_from(explicit_rows.len()).expect("policy row count fits u32"))
        .to_be_bytes(),
    );
    for &mechanism in committed_policy_mechanisms() {
        encode_mechanism(&mut hasher, mechanism.into());
        let disposition = classify_compiler_intrinsic(mechanism);
        encode_disposition(&mut hasher, &disposition);
    }
    for row in explicit_rows {
        encode_mechanism(&mut hasher, row.mechanism);
        encode_disposition(&mut hasher, &row.disposition);
    }
    hasher.finalize().into()
}

fn encode_disposition(hasher: &mut Sha256, disposition: &TerminalAuthorityDisposition) {
    hasher.update(
        u32::try_from(disposition.classes().len())
            .expect("terminal-authority class count fits u32")
            .to_be_bytes(),
    );
    for class in disposition.classes() {
        hasher.update([class.canonical_tag()]);
    }
}

fn committed_policy_mechanisms() -> &'static [CompilerIntrinsicExecutionIdentity] {
    static MECHANISMS: OnceLock<Vec<CompilerIntrinsicExecutionIdentity>> = OnceLock::new();
    MECHANISMS.get_or_init(closed_policy_mechanisms)
}

fn closed_policy_mechanisms() -> Vec<CompilerIntrinsicExecutionIdentity> {
    let mut mechanisms = Vec::with_capacity(CLOSED_POLICY_ROW_COUNT as usize);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32);
    mechanisms.extend(
        BuiltinFunction::ALL
            .into_iter()
            .map(CompilerIntrinsicExecutionIdentity::BuiltinFunction),
    );
    for operation in CompilerPrimitiveFloatBinaryOperation::ALL {
        for format in FLOAT_FORMATS {
            mechanisms.push(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                operation,
                format,
            });
        }
    }
    mechanisms.extend(
        FLOAT_FORMATS
            .into_iter()
            .map(CompilerIntrinsicExecutionIdentity::NamedFloatNegation),
    );
    for source in CompilerNumericType::ALL {
        for target in CompilerNumericType::ALL {
            for domain in ARITHMETIC_DOMAINS {
                mechanisms.push(CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                    source,
                    target,
                    domain,
                });
            }
        }
    }
    assert_eq!(mechanisms.len(), CLOSED_POLICY_ROW_COUNT as usize);
    mechanisms
}

fn encode_mechanism(hasher: &mut Sha256, mechanism: TerminalMechanismIdentity) {
    let bytes = terminal_mechanism_identity_bytes(mechanism);
    hasher.update(
        u32::try_from(bytes.len())
            .expect("terminal-mechanism identity length fits u32")
            .to_be_bytes(),
    );
    hasher.update(bytes);
}

const fn primitive_float_operation_tag(operation: CompilerPrimitiveFloatBinaryOperation) -> u8 {
    match operation {
        CompilerPrimitiveFloatBinaryOperation::Add => 0,
        CompilerPrimitiveFloatBinaryOperation::Subtract => 1,
        CompilerPrimitiveFloatBinaryOperation::Multiply => 2,
        CompilerPrimitiveFloatBinaryOperation::Divide => 3,
        CompilerPrimitiveFloatBinaryOperation::Equal => 4,
        CompilerPrimitiveFloatBinaryOperation::NotEqual => 5,
        CompilerPrimitiveFloatBinaryOperation::Less => 6,
        CompilerPrimitiveFloatBinaryOperation::LessOrEqual => 7,
        CompilerPrimitiveFloatBinaryOperation::Greater => 8,
        CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual => 9,
    }
}

const fn numeric_type_tag(numeric_type: CompilerNumericType) -> u8 {
    match numeric_type {
        CompilerNumericType::I8 => 0,
        CompilerNumericType::I16 => 1,
        CompilerNumericType::I32 => 2,
        CompilerNumericType::I64 => 3,
        CompilerNumericType::U8 => 4,
        CompilerNumericType::U16 => 5,
        CompilerNumericType::U32 => 6,
        CompilerNumericType::U64 => 7,
        CompilerNumericType::F32 => 8,
        CompilerNumericType::F64 => 9,
    }
}

const fn float_format_tag(format: FloatFormat) -> u8 {
    match format {
        FloatFormat::F32 => 0,
        FloatFormat::F64 => 1,
    }
}

const fn arithmetic_domain_tag(domain: ArithmeticDomain) -> u8 {
    match domain {
        ArithmeticDomain::Exact => 0,
        ArithmeticDomain::Wrapping => 1,
        ArithmeticDomain::Saturating => 2,
        ArithmeticDomain::Trapping => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foreign_mechanism(
        candidate: omega_target::ForeignLocatorCandidate,
        target: omega_target::TargetProfile,
        contract_byte: u8,
    ) -> TerminalMechanismIdentity {
        let locator = omega_target::normalize_foreign_locator(candidate, target)
            .expect("test locator must normalize");
        omega_effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            &locator,
            omega_effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                [contract_byte; 32],
            ),
        )
        .into()
    }

    fn row(
        mechanism: TerminalMechanismIdentity,
        classes: impl IntoIterator<Item = TerminalAuthorityClass>,
    ) -> TerminalAuthorityPolicyRow {
        TerminalAuthorityPolicyRow::new(
            mechanism,
            TerminalAuthorityDisposition::from_classes(classes),
        )
    }

    #[test]
    fn closed_policy_inventory_is_demand_complete() {
        let mechanisms = closed_policy_mechanisms();
        assert_eq!(mechanisms.len(), CLOSED_POLICY_ROW_COUNT as usize);
        let policy = current_compiler_intrinsic_terminal_authority_policy();
        for mechanism in mechanisms {
            policy
                .classify(mechanism)
                .expect("every committed mechanism must classify");
        }
    }

    #[test]
    fn builtin_partition_is_exact_and_explicit() {
        let policy = current_compiler_intrinsic_terminal_authority_policy();
        for function in BuiltinFunction::ALL {
            let actual = policy
                .classify(CompilerIntrinsicExecutionIdentity::BuiltinFunction(
                    function,
                ))
                .expect("every current builtin must have a committed policy row")
                .classes()
                .to_vec();
            let expected = match function {
                BuiltinFunction::AsmHlt
                | BuiltinFunction::AsmDisableInterrupts
                | BuiltinFunction::AsmEnableInterrupts
                | BuiltinFunction::AsmRestoreFlags
                | BuiltinFunction::AsmReadMsr
                | BuiltinFunction::AsmWriteMsr
                | BuiltinFunction::AsmReadCr0
                | BuiltinFunction::AsmReadCr2
                | BuiltinFunction::AsmReadCr3
                | BuiltinFunction::AsmReadCr4
                | BuiltinFunction::AsmWriteCr0
                | BuiltinFunction::AsmWriteCr3
                | BuiltinFunction::AsmWriteCr4 => vec![TerminalAuthorityClass::MachineControl],
                BuiltinFunction::AsmPortOut | BuiltinFunction::AsmPortIn => {
                    vec![TerminalAuthorityClass::PortIo]
                }
                BuiltinFunction::Max
                | BuiltinFunction::Min
                | BuiltinFunction::Sqrt
                | BuiltinFunction::AsmLoadFence
                | BuiltinFunction::AsmStoreFence
                | BuiltinFunction::AsmFullFence
                | BuiltinFunction::AsmSnapshotFlags
                | BuiltinFunction::FloatIsNan
                | BuiltinFunction::FloatMultiplyThenAddF32
                | BuiltinFunction::FloatMultiplyThenAddF64
                | BuiltinFunction::FloatFusedMultiplyAddF32
                | BuiltinFunction::FloatFusedMultiplyAddF64
                | BuiltinFunction::FloatIsFinite
                | BuiltinFunction::FloatIsInfinite
                | BuiltinFunction::FloatIsNormal
                | BuiltinFunction::FloatIsSubnormal
                | BuiltinFunction::FloatClassifyF32
                | BuiltinFunction::FloatClassifyF64
                | BuiltinFunction::ContentOld
                | BuiltinFunction::ContentSeparate
                | BuiltinFunction::FloatAddTowardZeroF32
                | BuiltinFunction::FloatAddTowardZeroF64
                | BuiltinFunction::FloatAddTowardPositiveF32
                | BuiltinFunction::FloatAddTowardPositiveF64
                | BuiltinFunction::FloatAddTowardNegativeF32
                | BuiltinFunction::FloatAddTowardNegativeF64
                | BuiltinFunction::FloatSubtractTowardZeroF32
                | BuiltinFunction::FloatSubtractTowardZeroF64
                | BuiltinFunction::FloatSubtractTowardPositiveF32
                | BuiltinFunction::FloatSubtractTowardPositiveF64
                | BuiltinFunction::FloatSubtractTowardNegativeF32
                | BuiltinFunction::FloatSubtractTowardNegativeF64
                | BuiltinFunction::FloatMultiplyTowardZeroF32
                | BuiltinFunction::FloatMultiplyTowardZeroF64
                | BuiltinFunction::FloatMultiplyTowardPositiveF32
                | BuiltinFunction::FloatMultiplyTowardPositiveF64
                | BuiltinFunction::FloatMultiplyTowardNegativeF32
                | BuiltinFunction::FloatMultiplyTowardNegativeF64
                | BuiltinFunction::FloatDivideTowardZeroF32
                | BuiltinFunction::FloatDivideTowardZeroF64
                | BuiltinFunction::FloatDivideTowardPositiveF32
                | BuiltinFunction::FloatDivideTowardPositiveF64
                | BuiltinFunction::FloatDivideTowardNegativeF32
                | BuiltinFunction::FloatDivideTowardNegativeF64
                | BuiltinFunction::FloatSqrtTowardZeroF32
                | BuiltinFunction::FloatSqrtTowardZeroF64
                | BuiltinFunction::FloatSqrtTowardPositiveF32
                | BuiltinFunction::FloatSqrtTowardPositiveF64
                | BuiltinFunction::FloatSqrtTowardNegativeF32
                | BuiltinFunction::FloatSqrtTowardNegativeF64
                | BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32
                | BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64
                | BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32
                | BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64
                | BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32
                | BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64 => Vec::new(),
            };
            assert_eq!(
                actual,
                expected,
                "wrong disposition for {}",
                function.name()
            );
        }
    }

    #[test]
    fn linux_exit_and_numeric_families_have_exact_dispositions() {
        let policy = current_compiler_intrinsic_terminal_authority_policy();
        assert_eq!(
            policy
                .classify(CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32)
                .expect("Linux exit must have a committed policy row")
                .classes(),
            &[TerminalAuthorityClass::ProcessTermination]
        );
        for mechanism in closed_policy_mechanisms()
            .into_iter()
            .skip(1 + BuiltinFunction::COUNT)
        {
            assert!(
                policy
                    .classify(mechanism)
                    .expect("every current numeric coordinate must have a committed policy row")
                    .is_authority_class_empty()
            );
        }
    }

    #[test]
    fn absent_policy_row_rejects_instead_of_becoming_empty() {
        let mechanism = CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32;
        let error = classify_from_inventory(&[], mechanism)
            .expect_err("an absent mechanism must not inherit an empty disposition");
        assert_eq!(error.mechanism(), mechanism.into());
    }

    #[test]
    fn all_normalized_foreign_locator_roles_classify_only_by_exact_row() {
        let cases = [
            (
                foreign_mechanism(
                    omega_target::ForeignLocatorCandidate::PeByName {
                        library: b"kernel32.dll".to_vec(),
                        export: b"FlushProcessWriteBuffers".to_vec(),
                    },
                    omega_target::TargetProfile::WindowsX64,
                    1,
                ),
                TerminalAuthorityClass::MachineControl,
            ),
            (
                foreign_mechanism(
                    omega_target::ForeignLocatorCandidate::PeByOrdinal {
                        library: b"fixture.dll".to_vec(),
                        ordinal: 7,
                    },
                    omega_target::TargetProfile::WindowsX64,
                    2,
                ),
                TerminalAuthorityClass::ProcessOutput,
            ),
            (
                foreign_mechanism(
                    omega_target::ForeignLocatorCandidate::ElfVersioned {
                        object: b"libc.so.6".to_vec(),
                        symbol: b"write".to_vec(),
                        version: b"GLIBC_2.2.5".to_vec(),
                    },
                    omega_target::TargetProfile::LinuxX64,
                    3,
                ),
                TerminalAuthorityClass::ProcessOutput,
            ),
            (
                foreign_mechanism(
                    omega_target::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                        symbol: b"_getpid".to_vec(),
                    },
                    omega_target::TargetProfile::MacosArm64,
                    4,
                ),
                TerminalAuthorityClass::ProcessTermination,
            ),
        ];
        let policy = terminal_authority_policy_with_rows(
            cases
                .iter()
                .map(|(mechanism, class)| row(*mechanism, [*class]))
                .collect(),
        )
        .expect("four exact foreign rows form one policy");
        for (mechanism, class) in cases {
            assert_eq!(policy.classify(mechanism).unwrap().classes(), &[class]);
        }
    }

    #[test]
    fn foreign_policy_rejects_missing_duplicate_locator_and_contract_substitution() {
        let exact = foreign_mechanism(
            omega_target::ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: b"FlushProcessWriteBuffers".to_vec(),
            },
            omega_target::TargetProfile::WindowsX64,
            9,
        );
        let policy = terminal_authority_policy_with_rows(vec![row(
            exact,
            [TerminalAuthorityClass::MachineControl],
        )])
        .unwrap();
        let locator_substitution = foreign_mechanism(
            omega_target::ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: b"GetCurrentProcessId".to_vec(),
            },
            omega_target::TargetProfile::WindowsX64,
            9,
        );
        let contract_substitution = foreign_mechanism(
            omega_target::ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: b"FlushProcessWriteBuffers".to_vec(),
            },
            omega_target::TargetProfile::WindowsX64,
            10,
        );
        assert!(current_terminal_authority_policy().classify(exact).is_err());
        assert!(policy.classify(locator_substitution).is_err());
        assert!(policy.classify(contract_substitution).is_err());
        assert!(matches!(
            terminal_authority_policy_with_rows(vec![row(exact, []), row(exact, [])]),
            Err(TerminalAuthorityPolicyBuildError::DuplicateMechanism(mechanism))
                if mechanism == exact
        ));
    }

    #[test]
    fn foreign_rows_and_dispositions_enter_the_complete_policy_commitment() {
        let exact = foreign_mechanism(
            omega_target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"write".to_vec(),
                version: b"GLIBC_2.2.5".to_vec(),
            },
            omega_target::TargetProfile::LinuxX64,
            12,
        );
        let empty = terminal_authority_policy_with_rows(vec![row(exact, [])]).unwrap();
        let output = terminal_authority_policy_with_rows(vec![row(
            exact,
            [TerminalAuthorityClass::ProcessOutput],
        )])
        .unwrap();
        assert_ne!(
            current_terminal_authority_policy().identity(),
            empty.identity()
        );
        assert_ne!(empty.identity(), output.identity());
    }

    #[test]
    fn policy_identity_binds_version_and_complete_table() {
        let identity = current_compiler_intrinsic_terminal_authority_policy().identity();
        assert_eq!(
            identity.version(),
            COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION
        );
        assert_eq!(identity.commitment(), complete_policy_commitment(&[]));
        assert_eq!(
            identity.commitment(),
            [
                84, 53, 183, 206, 20, 95, 125, 211, 166, 25, 16, 71, 143, 255, 159, 161, 78, 162,
                157, 72, 252, 99, 202, 138, 4, 108, 51, 45, 177, 251, 73, 33,
            ]
        );
    }
}
