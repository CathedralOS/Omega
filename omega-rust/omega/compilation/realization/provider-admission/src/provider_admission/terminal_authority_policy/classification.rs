/*
Closed compiler-intrinsic inventory and authority-disposition classification.

`committed_policy_mechanisms` enumerates the exact `CLOSED_POLICY_ROW_COUNT`
non-parameterized compiler-intrinsic coordinates committed into the policy
table, using `BuiltinFunction::COUNT` directly so catalog additions do not
require a duplicate count edit here.

`classify_compiler_intrinsic` assigns each intrinsic's host-authority classes:
static interrupt installation and privileged register/descriptor operations
require `MachineControl`, port I/O builtins require `PortIo`, hosted exit/write/
read builtins require `ProcessTermination`/`ProcessOutput`/`ProcessInput`, and
storage, lifecycle, address-observation, caller-RAM accessor, and arithmetic
intrinsics exercise no host authority class.
*/

use std::sync::OnceLock;

use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation,
    TerminalAuthorityClass, TerminalAuthorityDisposition, primitive_float_operation_tag,
};
use numerics::literals::FloatFormat;
use symbols::BuiltinFunction;

use super::UnclassifiedTerminalMechanism;

pub(super) const CLOSED_POLICY_ROW_COUNT: u32 = 46 + BuiltinFunction::COUNT as u32;
const FLOAT_FORMATS: [FloatFormat; 2] = [FloatFormat::F32, FloatFormat::F64];

pub(super) fn committed_policy_mechanisms() -> &'static [CompilerIntrinsicExecutionIdentity] {
    static MECHANISMS: OnceLock<Vec<CompilerIntrinsicExecutionIdentity>> = OnceLock::new();
    MECHANISMS.get_or_init(closed_policy_mechanisms)
}

pub(super) fn closed_policy_mechanisms() -> Vec<CompilerIntrinsicExecutionIdentity> {
    let mut mechanisms = Vec::with_capacity(CLOSED_POLICY_ROW_COUNT as usize);
    mechanisms.extend(
        language_core::static_interrupts::StaticInterruptOperation::ALL
            .map(CompilerIntrinsicExecutionIdentity::StaticInterrupt),
    );
    mechanisms.push(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::HostedWriteByteI32);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::HostedReadByte);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::ProgramStorageEntry);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::EmptyRunRelease);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::TaskRegisterActivate);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::TaskRegisterRead);
    mechanisms.extend(
        [
            terminal_psi::SegmentReload::Code,
            terminal_psi::SegmentReload::Data,
        ]
        .map(CompilerIntrinsicExecutionIdentity::SegmentReload),
    );
    mechanisms.extend(
        [
            terminal_psi::SegmentRegister::Code,
            terminal_psi::SegmentRegister::Data,
            terminal_psi::SegmentRegister::Extra,
            terminal_psi::SegmentRegister::Stack,
            terminal_psi::SegmentRegister::Fs,
            terminal_psi::SegmentRegister::Gs,
        ]
        .map(CompilerIntrinsicExecutionIdentity::SegmentRead),
    );
    mechanisms.extend(
        [
            terminal_psi::TerminalAccessLifecycleOperation::Initialize,
            terminal_psi::TerminalAccessLifecycleOperation::MoveOut,
            terminal_psi::TerminalAccessLifecycleOperation::View,
            terminal_psi::TerminalAccessLifecycleOperation::Relocate,
        ]
        .map(CompilerIntrinsicExecutionIdentity::AccessLifecycle),
    );
    mechanisms.extend(
        BuiltinFunction::ALL
            .into_iter()
            .map(|(function, _)| CompilerIntrinsicExecutionIdentity::BuiltinFunction(function)),
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
    assert_eq!(mechanisms.len(), CLOSED_POLICY_ROW_COUNT as usize);
    mechanisms
}

pub(super) fn classify_compiler_intrinsic(
    mechanism: CompilerIntrinsicExecutionIdentity,
) -> TerminalAuthorityDisposition {
    match mechanism {
        CompilerIntrinsicExecutionIdentity::StaticInterrupt(operation) => {
            if operation == language_core::static_interrupts::StaticInterruptOperation::Install {
                disposition([TerminalAuthorityClass::MachineControl])
            } else {
                empty_disposition()
            }
        }
        CompilerIntrinsicExecutionIdentity::ProgramStorageEntry
        | CompilerIntrinsicExecutionIdentity::EmptyRunRelease
        | CompilerIntrinsicExecutionIdentity::AccessLifecycle(_)
        | CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart
        | CompilerIntrinsicExecutionIdentity::AccessorScalarAccess { .. } => empty_disposition(),
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => {
            disposition([TerminalAuthorityClass::ProcessTermination])
        }
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => {
            disposition([TerminalAuthorityClass::ProcessOutput])
        }
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            disposition([TerminalAuthorityClass::ProcessInput])
        }
        CompilerIntrinsicExecutionIdentity::TaskRegisterActivate
        | CompilerIntrinsicExecutionIdentity::TaskRegisterRead
        | CompilerIntrinsicExecutionIdentity::SegmentReload(_)
        | CompilerIntrinsicExecutionIdentity::SegmentRead(_) => {
            disposition([TerminalAuthorityClass::MachineControl])
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
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison { .. }
        | CompilerIntrinsicExecutionIdentity::NamedFloatConversion { .. } => empty_disposition(),
    }
}

pub(super) fn classify_from_inventory(
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
        | BuiltinFunction::AsmWriteCr4
        | BuiltinFunction::AsmStoreGdtr
        | BuiltinFunction::AsmLoadGdtr
        | BuiltinFunction::AsmWriteBackInvalidate
        | BuiltinFunction::AsmInvalidate
        | BuiltinFunction::AsmWriteBackNoInvalidate
        | BuiltinFunction::AsmReadSctlrEl1
        | BuiltinFunction::AsmReadTcrEl1
        | BuiltinFunction::AsmReadTtbr0El1
        | BuiltinFunction::AsmReadTtbr1El1
        | BuiltinFunction::AsmReadMairEl1
        | BuiltinFunction::AsmReadVbarEl1
        | BuiltinFunction::AsmReadTpidrEl1
        | BuiltinFunction::AsmReadEsrEl1
        | BuiltinFunction::AsmReadFarEl1
        | BuiltinFunction::AsmWriteSctlrEl1
        | BuiltinFunction::AsmWriteTcrEl1
        | BuiltinFunction::AsmWriteTtbr0El1
        | BuiltinFunction::AsmWriteTtbr1El1
        | BuiltinFunction::AsmWriteMairEl1
        | BuiltinFunction::AsmWriteVbarEl1
        | BuiltinFunction::AsmWriteTpidrEl1 => {
            disposition([TerminalAuthorityClass::MachineControl])
        }
        BuiltinFunction::AsmPortOut | BuiltinFunction::AsmPortIn => {
            disposition([TerminalAuthorityClass::PortIo])
        }
        BuiltinFunction::FloatMaximum
        | BuiltinFunction::FloatMinimum
        | BuiltinFunction::FloatSquareRoot
        | BuiltinFunction::AsmLoadFence
        | BuiltinFunction::AsmStoreFence
        | BuiltinFunction::AsmFullFence
        | BuiltinFunction::AsmAarch64LoadFence
        | BuiltinFunction::AsmAarch64StoreFence
        | BuiltinFunction::AsmAarch64FullFence
        | BuiltinFunction::AsmSerialize
        | BuiltinFunction::AsmInstructionSyncBarrier
        | BuiltinFunction::AsmSpinPause
        | BuiltinFunction::AsmYieldHint
        | BuiltinFunction::AsmNop
        | BuiltinFunction::AsmWaitForEvent
        | BuiltinFunction::AsmWaitForInterrupt
        | BuiltinFunction::AsmSendEvent
        | BuiltinFunction::AsmSendEventLocal
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
        | BuiltinFunction::IntegerEmbed
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

const fn float_format_tag(format: FloatFormat) -> u8 {
    match format {
        FloatFormat::F32 => 0,
        FloatFormat::F64 => 1,
    }
}
