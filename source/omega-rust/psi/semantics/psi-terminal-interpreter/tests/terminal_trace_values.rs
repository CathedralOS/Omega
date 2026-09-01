use psi_core::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
};
use psi_terminal::{TerminalTraceScalarSchema, TerminalTraceValueComparison};
use psi_terminal_interpreter::{
    TerminalScalarValue, TerminalTraceScalarComparisonError, TerminalTraceScalarValueSide,
    compare_terminal_trace_scalar_values,
};

fn schema(scalar_type: ScalarType) -> TerminalTraceScalarSchema {
    TerminalTraceScalarSchema {
        scalar_type,
        comparison: TerminalTraceValueComparison::ExactSemanticValue,
    }
}

fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("fixed integer type")
}

fn integer(scalar_type: IntegerType, value: IntegerValue) -> TerminalScalarValue {
    TerminalScalarValue::Integer { scalar_type, value }
}

#[test]
fn exact_terminal_trace_scalar_values_compare_by_typed_semantic_value() {
    assert_eq!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::Boolean),
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(true),
        ),
        Ok(true),
    );
    assert_eq!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::Boolean),
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ),
        Ok(false),
    );

    let u16_type = integer_type(IntegerSign::Unsigned, 16);
    assert_eq!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::Integer(u16_type)),
            integer(u16_type, IntegerValue::Unsigned(513)),
            integer(u16_type, IntegerValue::Unsigned(513)),
        ),
        Ok(true),
    );
    assert_eq!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::Integer(u16_type)),
            integer(u16_type, IntegerValue::Unsigned(513)),
            integer(u16_type, IntegerValue::Unsigned(514)),
        ),
        Ok(false),
    );
}

#[test]
fn exact_terminal_trace_float_values_compare_raw_interchange_bits() {
    let binary32 = schema(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32));
    assert_eq!(
        compare_terminal_trace_scalar_values(
            binary32,
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x0000_0000)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x8000_0000)),
        ),
        Ok(false),
        "positive and negative zero remain distinct semantic payloads",
    );
    assert_eq!(
        compare_terminal_trace_scalar_values(
            binary32,
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0042)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0042)),
        ),
        Ok(true),
    );
    assert_eq!(
        compare_terminal_trace_scalar_values(
            binary32,
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0042)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0043)),
        ),
        Ok(false),
        "distinct NaN payloads remain distinct semantic values",
    );
}

#[test]
fn exact_terminal_trace_scalar_comparison_rejects_type_and_value_drift() {
    let u8_type = integer_type(IntegerSign::Unsigned, 8);
    assert_eq!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::Integer(u8_type)),
            integer(u8_type, IntegerValue::Unsigned(7)),
            TerminalScalarValue::Boolean(true),
        ),
        Err(TerminalTraceScalarComparisonError::ScalarTypeMismatch {
            side: TerminalTraceScalarValueSide::Actual,
            schema: ScalarType::Integer(u8_type),
            value: ScalarType::Boolean,
        }),
    );
    assert_eq!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::Integer(u8_type)),
            integer(u8_type, IntegerValue::Unsigned(256)),
            integer(u8_type, IntegerValue::Unsigned(0)),
        ),
        Err(TerminalTraceScalarComparisonError::InvalidIntegerValue {
            side: TerminalTraceScalarValueSide::Expected,
            scalar_type: u8_type,
        }),
    );
    assert!(matches!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(0)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0)),
        ),
        Err(TerminalTraceScalarComparisonError::ScalarTypeMismatch {
            side: TerminalTraceScalarValueSide::Expected,
            ..
        })
    ));
    let address = IntegerType::address(64).expect("address carrier type");
    assert_eq!(
        compare_terminal_trace_scalar_values(
            schema(ScalarType::Integer(address)),
            integer(address, IntegerValue::Unsigned(0)),
            integer(address, IntegerValue::Unsigned(0)),
        ),
        Err(
            TerminalTraceScalarComparisonError::UnsupportedScalarSchema {
                scalar_type: ScalarType::Integer(address),
            }
        ),
    );
}
