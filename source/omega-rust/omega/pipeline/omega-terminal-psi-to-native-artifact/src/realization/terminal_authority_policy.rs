use omega_effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    TerminalAuthorityClass, TerminalAuthorityDisposition, TerminalAuthorityPolicyIdentity,
};
use psi_numerics::{arithmetic::ArithmeticDomain, literals::FloatFormat};
use psi_symbols::BuiltinFunction;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

/// Version of the first receiving-realization policy table over the closed
/// compiler-intrinsic terminal-mechanism family.
pub const COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION: u32 = 1;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"omega.terminal-authority.compiler-intrinsic-policy.v1\0";
const CLOSED_POLICY_ROW_COUNT: u32 = 494;
const FLOAT_FORMATS: [FloatFormat; 2] = [FloatFormat::F32, FloatFormat::F64];
const ARITHMETIC_DOMAINS: [ArithmeticDomain; 4] = [
    ArithmeticDomain::Exact,
    ArithmeticDomain::Wrapping,
    ArithmeticDomain::Saturating,
    ArithmeticDomain::Trapping,
];

/// Current receiving-realization policy for the closed compiler-intrinsic
/// mechanism family. This table classifies physical authority only. It does
/// not perform provider-closure traversal or service/schema permission joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerIntrinsicTerminalAuthorityPolicy {
    identity: TerminalAuthorityPolicyIdentity,
}

/// The requested closed mechanism has no row in this exact policy version.
/// This is distinct from a committed row with an empty class disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnclassifiedCompilerIntrinsicTerminalMechanism {
    mechanism: CompilerIntrinsicExecutionIdentity,
}

impl UnclassifiedCompilerIntrinsicTerminalMechanism {
    pub const fn mechanism(self) -> CompilerIntrinsicExecutionIdentity {
        self.mechanism
    }
}

impl CompilerIntrinsicTerminalAuthorityPolicy {
    pub const fn identity(self) -> TerminalAuthorityPolicyIdentity {
        self.identity
    }

    /// Classify one exact closed mechanism. The outer family and nested
    /// builtin family are matched exhaustively: future variants cannot inherit
    /// an authority-free disposition through a wildcard.
    pub fn classify(
        self,
        mechanism: CompilerIntrinsicExecutionIdentity,
    ) -> Result<TerminalAuthorityDisposition, UnclassifiedCompilerIntrinsicTerminalMechanism> {
        classify_from_inventory(committed_policy_mechanisms(), mechanism)
    }
}

pub fn current_compiler_intrinsic_terminal_authority_policy()
-> CompilerIntrinsicTerminalAuthorityPolicy {
    static IDENTITY: OnceLock<TerminalAuthorityPolicyIdentity> = OnceLock::new();
    CompilerIntrinsicTerminalAuthorityPolicy {
        identity: *IDENTITY.get_or_init(|| {
            TerminalAuthorityPolicyIdentity::from_parts(
                COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION,
                complete_policy_commitment(),
            )
        }),
    }
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
) -> Result<TerminalAuthorityDisposition, UnclassifiedCompilerIntrinsicTerminalMechanism> {
    if !inventory.contains(&mechanism) {
        return Err(UnclassifiedCompilerIntrinsicTerminalMechanism { mechanism });
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

fn complete_policy_commitment() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_COMMITMENT_DOMAIN);
    hasher.update(COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION.to_be_bytes());
    hasher.update(CLOSED_POLICY_ROW_COUNT.to_be_bytes());
    for &mechanism in committed_policy_mechanisms() {
        encode_mechanism(&mut hasher, mechanism);
        let disposition = classify_compiler_intrinsic(mechanism);
        hasher.update((disposition.classes().len() as u32).to_be_bytes());
        for class in disposition.classes() {
            hasher.update([class.canonical_tag()]);
        }
    }
    hasher.finalize().into()
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

fn encode_mechanism(hasher: &mut Sha256, mechanism: CompilerIntrinsicExecutionIdentity) {
    match mechanism {
        CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => hasher.update([0]),
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            hasher.update([1]);
            hasher.update((function.ordinal() as u32).to_be_bytes());
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            hasher.update([
                2,
                primitive_float_operation_tag(operation),
                float_format_tag(format),
            ]);
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            hasher.update([3, float_format_tag(format)]);
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            hasher.update([
                4,
                numeric_type_tag(source),
                numeric_type_tag(target),
                arithmetic_domain_tag(domain),
            ]);
        }
    }
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
        assert_eq!(error.mechanism(), mechanism);
    }

    #[test]
    fn policy_identity_binds_version_and_complete_table() {
        let identity = current_compiler_intrinsic_terminal_authority_policy().identity();
        assert_eq!(
            identity.version(),
            COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION
        );
        assert_eq!(identity.commitment(), complete_policy_commitment());
        assert_eq!(
            identity.commitment(),
            [
                54, 0, 134, 218, 118, 194, 238, 152, 101, 187, 117, 159, 16, 41, 186, 125, 209,
                161, 229, 253, 90, 112, 126, 56, 188, 118, 240, 166, 190, 146, 185, 26,
            ]
        );
    }
}
