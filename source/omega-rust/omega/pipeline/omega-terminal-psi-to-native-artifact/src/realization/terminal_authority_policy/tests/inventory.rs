//! Optimizer module role: test leaf. Compiler-intrinsic inventory closure and identity.

use omega_effects::{CompilerIntrinsicExecutionIdentity, TerminalAuthorityClass};
use psi_symbols::BuiltinFunction;

use super::*;
use crate::realization::terminal_authority_policy::{
    classification::classify_from_inventory,
    commitment::complete_policy_commitment,
    inventory::{closed_policy_mechanisms, CLOSED_POLICY_ROW_COUNT},
};

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
            | BuiltinFunction::ContentEmbed
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
fn linux_console_and_numeric_families_have_exact_dispositions() {
    let policy = current_compiler_intrinsic_terminal_authority_policy();
    assert_eq!(
        policy
            .classify(CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32)
            .expect("Linux exit must have a committed policy row")
            .classes(),
        &[TerminalAuthorityClass::ProcessTermination]
    );
    assert_eq!(
        policy
            .classify(CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32)
            .expect("Linux write-byte must have a committed policy row")
            .classes(),
        &[TerminalAuthorityClass::ProcessOutput]
    );
    for mechanism in closed_policy_mechanisms()
        .into_iter()
        .skip(2 + BuiltinFunction::COUNT)
    {
        assert!(policy
            .classify(mechanism)
            .expect("every current numeric coordinate must have a committed policy row")
            .is_authority_class_empty());
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
            149, 61, 29, 44, 53, 29, 2, 19, 69, 240, 179, 190, 133, 203, 164, 253, 95, 141, 24,
            147, 156, 221, 98, 63, 189, 77, 120, 57, 153, 245, 182, 255,
        ]
    );
}
