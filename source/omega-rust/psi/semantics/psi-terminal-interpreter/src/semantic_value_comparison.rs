use psi_core::ScalarType;
use psi_terminal::{TerminalTraceScalarSchema, TerminalTraceValueComparison};

use crate::TerminalScalarValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTraceScalarValueSide {
    Expected,
    Actual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTraceScalarComparisonError {
    UnsupportedScalarSchema {
        scalar_type: ScalarType,
    },
    ScalarTypeMismatch {
        side: TerminalTraceScalarValueSide,
        schema: ScalarType,
        value: ScalarType,
    },
    InvalidIntegerValue {
        side: TerminalTraceScalarValueSide,
        scalar_type: psi_core::IntegerType,
    },
}

/// Compare two runtime scalar values under one verifier-derived
/// `TerminalTraceV1` schema.
///
/// Schema agreement is checked before equality. IEEE values compare their
/// retained interchange bits directly, so signed zero and NaN payloads remain
/// semantic data rather than passing through host floating-point equality.
pub fn compare_terminal_trace_scalar_values(
    schema: TerminalTraceScalarSchema,
    expected: TerminalScalarValue,
    actual: TerminalScalarValue,
) -> Result<bool, TerminalTraceScalarComparisonError> {
    match schema.comparison {
        TerminalTraceValueComparison::ExactSemanticValue => {}
    }
    if matches!(schema.scalar_type, ScalarType::Integer(integer) if integer.is_address()) {
        return Err(
            TerminalTraceScalarComparisonError::UnsupportedScalarSchema {
                scalar_type: schema.scalar_type,
            },
        );
    }
    validate_scalar_value(
        schema.scalar_type,
        expected,
        TerminalTraceScalarValueSide::Expected,
    )?;
    validate_scalar_value(
        schema.scalar_type,
        actual,
        TerminalTraceScalarValueSide::Actual,
    )?;
    Ok(expected == actual)
}

fn validate_scalar_value(
    schema: ScalarType,
    value: TerminalScalarValue,
    side: TerminalTraceScalarValueSide,
) -> Result<(), TerminalTraceScalarComparisonError> {
    let value_type = value.scalar_type();
    if value_type != schema {
        return Err(TerminalTraceScalarComparisonError::ScalarTypeMismatch {
            side,
            schema,
            value: value_type,
        });
    }
    if let TerminalScalarValue::Integer { scalar_type, value } = value
        && !scalar_type.admits(value)
    {
        return Err(TerminalTraceScalarComparisonError::InvalidIntegerValue { side, scalar_type });
    }
    Ok(())
}
