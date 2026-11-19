// Sealed conversion from `CompilerIntrinsicExecutionIdentity` to the local
// `CompilerBuiltinExecution` catalog.
//
// `SettledCompilerBuiltin` has no public constructor, so provider proposals and
// private access lifecycles both settle through this exhaustive mapping rather
// than a policy guard beside an independently built catalog entry. Format-free
// custody identities use the Psi-published `boundary_format` while format-bearing
// builtin symbols use their own IEEE format; admission still rejoins the
// complete boundary signature independently.

use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use target_operations::CompilerBuiltinExecution;

/// ```compile_fail
/// let forged = provider_admission::SettledCompilerBuiltin(
///     target_operations::CompilerBuiltinExecution::HostedReadByte,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettledCompilerBuiltin(CompilerBuiltinExecution);

impl SettledCompilerBuiltin {
    pub(crate) fn from_identity(
        identity: CompilerIntrinsicExecutionIdentity,
        boundary: semantic_vocabulary::BoundaryMachineId,
        boundary_format: Option<semantic_vocabulary::IeeeFloatFormat>,
    ) -> Option<Self> {
        compiler_builtin_execution(identity, boundary, boundary_format).map(Self)
    }

    pub const fn execution(self) -> CompilerBuiltinExecution {
        self.0
    }
}

fn compiler_builtin_execution(
    identity: CompilerIntrinsicExecutionIdentity,
    boundary: semantic_vocabulary::BoundaryMachineId,
    boundary_format: Option<semantic_vocabulary::IeeeFloatFormat>,
) -> Option<CompilerBuiltinExecution> {
    match identity {
        CompilerIntrinsicExecutionIdentity::TaskRegisterActivate
        | CompilerIntrinsicExecutionIdentity::TaskRegisterRead
        | CompilerIntrinsicExecutionIdentity::SegmentReload(_)
        | CompilerIntrinsicExecutionIdentity::SegmentRead(_) => None,
        CompilerIntrinsicExecutionIdentity::StaticInterrupt(operation) => {
            Some(CompilerBuiltinExecution::StaticInterrupt {
                boundary,
                operation,
            })
        }
        CompilerIntrinsicExecutionIdentity::AccessLifecycle(operation) => {
            Some(CompilerBuiltinExecution::AccessLifecycle {
                boundary,
                operation,
            })
        }
        CompilerIntrinsicExecutionIdentity::EmptyRunRelease => {
            Some(CompilerBuiltinExecution::EmptyRunRelease)
        }
        CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart => {
            Some(CompilerBuiltinExecution::ProgramImageRetainedSuffixStart)
        }
        CompilerIntrinsicExecutionIdentity::ProgramStorageEntry => {
            Some(CompilerBuiltinExecution::ProgramStorageEntry)
        }
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => {
            Some(CompilerBuiltinExecution::HostedExitProcessI32)
        }
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => {
            Some(CompilerBuiltinExecution::HostedWriteByteI32)
        }
        CompilerIntrinsicExecutionIdentity::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => Some(CompilerBuiltinExecution::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation: match operation {
                native_intrinsics::CompilerAccessorScalarOperation::Read => {
                    terminal_psi::TerminalAccessorKind::Read
                }
                native_intrinsics::CompilerAccessorScalarOperation::Take => {
                    terminal_psi::TerminalAccessorKind::Take
                }
                native_intrinsics::CompilerAccessorScalarOperation::Write => {
                    terminal_psi::TerminalAccessorKind::Write
                }
            },
        }),
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            Some(CompilerBuiltinExecution::HostedReadByte)
        }
        numeric @ (CompilerIntrinsicExecutionIdentity::BuiltinFunction(_)
        | CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { .. }
        | CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison { .. }
        | CompilerIntrinsicExecutionIdentity::NamedFloatNegation(_)
        | CompilerIntrinsicExecutionIdentity::NamedFloatConversion { .. }) => {
            numeric_intrinsic_admission::catalog_realization(numeric, boundary_format).map(
                |realization| CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                    operation: realization.operation,
                    format: realization.format,
                },
            )
        }
    }
}

#[cfg(test)]
mod integer_width_tests {
    use super::*;
    use native_intrinsics::CompilerNumericType;
    use numerics::{arithmetic::ArithmeticDomain, integer_carrier::FixedInteger};

    #[test]
    fn integer_width_native_conversion_requires_an_exact_supported_carrier() {
        let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
        for (width, supported) in [(13, false), (32, true), (64, true), (257, false)] {
            let meaning = CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                source: CompilerNumericType::Integer(FixedInteger::unsigned(width)),
                target: CompilerNumericType::F32,
                domain: ArithmeticDomain::Exact,
            };
            assert_eq!(
                compiler_builtin_execution(meaning, boundary, None).is_some(),
                supported
            );
        }
    }
}
