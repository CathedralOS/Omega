use super::{Error, Reader, execution};
use crate::encoding::{
    PackagePolicyRecoveryLimits,
    encode::{encode_compiler_intrinsic_execution, encoder::Encoder},
};
use crate::record::PackageReviewCompilerIntrinsicExecution as Execution;
use effects::CompilerAccessorScalarOperation;
use language_core::static_interrupts::StaticInterruptOperation;
use terminal_psi::TerminalAccessLifecycleOperation;

fn encode(value: &Execution) -> Vec<u8> {
    let mut encoder = Encoder::bounded(256);
    encode_compiler_intrinsic_execution(&mut encoder, value).expect("encode intrinsic");
    encoder.finish().expect("finish intrinsic bytes")
}

fn recover(bytes: &[u8]) -> Result<Execution, Error> {
    let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default())?;
    let value = execution(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

fn affected_executions() -> Vec<(Execution, Vec<u8>)> {
    let mut cases = vec![(Execution::EmptyRunRelease, vec![16])];
    for operation in StaticInterruptOperation::ALL {
        cases.push((
            Execution::StaticInterrupt(operation),
            vec![17, operation.code()],
        ));
    }
    for (code, operation) in [
        TerminalAccessLifecycleOperation::Initialize,
        TerminalAccessLifecycleOperation::MoveOut,
        TerminalAccessLifecycleOperation::View,
        TerminalAccessLifecycleOperation::Relocate,
    ]
    .into_iter()
    .enumerate()
    {
        cases.push((Execution::AccessLifecycle(operation), vec![11, code as u8]));
    }
    for (code, operation) in [
        CompilerAccessorScalarOperation::Read,
        CompilerAccessorScalarOperation::Take,
        CompilerAccessorScalarOperation::Write,
    ]
    .into_iter()
    .enumerate()
    {
        let contract = std::array::from_fn(|position| position as u8);
        let placement_commitment = std::array::from_fn(|position| 255 - position as u8);
        let access_slot = 0x1234_abcd;
        let value = Execution::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        };
        let mut bytes = vec![8];
        bytes.extend(contract);
        bytes.extend(placement_commitment);
        bytes.extend(access_slot.to_le_bytes());
        bytes.push(code as u8);
        cases.push((value, bytes));
    }
    cases
}

#[test]
fn intrinsic_payloads_round_trip_without_changing_existing_bytes() {
    for (value, expected_bytes) in affected_executions() {
        assert_eq!(encode(&value), expected_bytes, "{value:?}");
        let recovered = recover(&expected_bytes).expect("recover emitted intrinsic");
        assert_eq!(recovered, value);
        assert_eq!(encode(&recovered), expected_bytes);
    }
}

#[test]
fn existing_intrinsic_variants_still_round_trip() {
    use numerics::{arithmetic::ArithmeticDomain, literals::FloatFormat};
    use provider_planning::{
        CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
        CompilerPrimitiveIntegerComparisonOperation,
    };
    let cases = [
        Execution::BuiltinFunction(
            symbols::BuiltinFunction::from_ordinal(0).expect("first builtin"),
        ),
        Execution::NamedFloatNegation(FloatFormat::F64),
        Execution::NamedFloatConversion {
            source: CompilerNumericType::I32,
            target: CompilerNumericType::F32,
            domain: ArithmeticDomain::Trapping,
        },
        Execution::PrimitiveFloatBinary {
            operation: CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual,
            format: FloatFormat::F32,
        },
        Execution::HostedExitProcessI32,
        Execution::HostedWriteByteI32,
        Execution::HostedReadByte,
        Execution::PrimitiveIntegerComparison {
            operation: CompilerPrimitiveIntegerComparisonOperation::LessOrEqual,
            integer_type: CompilerNumericType::U64,
        },
        Execution::ProgramImageRetainedSuffixStart,
        Execution::ProgramStorageEntry,
        Execution::TaskRegisterActivate,
        Execution::TaskRegisterRead,
        Execution::SegmentReload(terminal_psi::SegmentReload::Data),
        Execution::SegmentRead(terminal_psi::SegmentRegister::Gs),
    ];
    for (value, tag) in cases
        .into_iter()
        .zip([0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 12, 13, 14, 15])
    {
        let bytes = encode(&value);
        assert_eq!(bytes[0], tag);
        assert_eq!(recover(&bytes), Ok(value));
    }
    // A valid numeric tag is still insufficient for an integer-only operation.
    assert_eq!(recover(&[7, 0, 8]), Err(Error::InvalidTag));
    assert_eq!(recover(&[0, 255, 255]), Err(Error::InvalidTag));
}

#[test]
fn intrinsic_recovery_rejects_truncation_trailing_bytes_and_unknown_operations() {
    for (value, bytes) in affected_executions() {
        for length in 0..bytes.len() {
            assert_eq!(
                recover(&bytes[..length]),
                Err(Error::UnexpectedEnd),
                "{value:?} at {length}"
            );
        }
        let mut malformed = bytes.clone();
        malformed.push(0);
        assert_eq!(recover(&malformed), Err(Error::TrailingBytes));
        if bytes.len() > 1 {
            let mut malformed = bytes;
            *malformed.last_mut().expect("operation byte") = 255;
            assert_eq!(recover(&malformed), Err(Error::InvalidTag));
        }
    }
    for tag in 18..=u8::MAX {
        assert_eq!(recover(&[tag]), Err(Error::InvalidTag));
    }
}

#[test]
fn accessor_identity_changes_remain_distinct_after_recovery() {
    let original = affected_executions().pop().expect("accessor case").1;
    let recovered = recover(&original).expect("recover accessor");
    // Commitments and slots are inert identity, not self-authenticating receipts.
    // Every changed coordinate must survive recovery for the later exact join.
    for offset in [1, 33, 65] {
        let mut changed = original.clone();
        changed[offset] ^= 1;
        let changed_value = recover(&changed).expect("changed identity remains inert data");
        assert_ne!(changed_value, recovered);
        assert_eq!(encode(&changed_value), changed);
    }
}
