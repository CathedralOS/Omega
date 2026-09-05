use super::*;
use crate::encoding::{
    PackagePolicyRecoveryLimits,
    encode::{encode_compiler_intrinsic_execution, encoder::Encoder},
};

fn encoded(value: PackageReviewCompilerIntrinsicExecution) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(64);
    encode_compiler_intrinsic_execution(&mut encoder, &value).unwrap();
    encoder.finish().unwrap()
}

fn recovered(bytes: &[u8]) -> Result<PackageReviewCompilerIntrinsicExecution, Error> {
    let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default())?;
    let value = execution(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

#[test]
fn exact_intrinsic_inverse_covers_every_scalar_variant_and_builtin() {
    use CompilerNumericType as Numeric;
    use CompilerPrimitiveFloatBinaryOperation as Operation;
    use PackageReviewCompilerIntrinsicExecution as Execution;
    let mut values = vec![
        Execution::LinuxExitGroupI32,
        Execution::LinuxWriteByteI32,
        Execution::LinuxReadByte,
    ];
    values.extend(
        psi_symbols::BuiltinFunction::ALL
            .into_iter()
            .map(Execution::BuiltinFunction),
    );
    for format in [FloatFormat::F32, FloatFormat::F64] {
        values.push(Execution::NamedFloatNegation(format));
        for operation in [
            Operation::Add,
            Operation::Subtract,
            Operation::Multiply,
            Operation::Divide,
            Operation::Equal,
            Operation::NotEqual,
            Operation::Less,
            Operation::LessOrEqual,
            Operation::Greater,
            Operation::GreaterOrEqual,
        ] {
            values.push(Execution::PrimitiveFloatBinary { operation, format });
        }
    }
    let numeric = [
        Numeric::I8,
        Numeric::I16,
        Numeric::I32,
        Numeric::I64,
        Numeric::U8,
        Numeric::U16,
        Numeric::U32,
        Numeric::U64,
        Numeric::F32,
        Numeric::F64,
    ];
    for source in numeric {
        for target in numeric {
            for domain in [
                ArithmeticDomain::Exact,
                ArithmeticDomain::Wrapping,
                ArithmeticDomain::Saturating,
                ArithmeticDomain::Trapping,
            ] {
                values.push(Execution::NamedFloatConversion {
                    source,
                    target,
                    domain,
                });
            }
        }
    }
    for value in values {
        let bytes = encoded(value);
        assert_eq!(recovered(&bytes).unwrap(), value);
        for end in 0..bytes.len() {
            assert!(recovered(&bytes[..end]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(recovered(&trailing), Err(Error::TrailingBytes));
    }
}

#[test]
fn intrinsic_tags_and_builtin_ordinals_are_closed() {
    for bytes in [
        &[7][..],
        &[255],
        &[0, 255, 255],
        &[1, 2],
        &[2, 10, 0, 0],
        &[2, 0, 10, 0],
        &[2, 0, 0, 4],
        &[3, 10, 0],
        &[3, 0, 2],
    ] {
        assert_eq!(recovered(bytes), Err(Error::InvalidTag), "{bytes:?}");
    }
}
