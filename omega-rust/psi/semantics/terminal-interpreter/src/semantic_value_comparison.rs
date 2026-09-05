use semantic_vocabulary::{ScalarType, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    TerminalTraceScalarSchema, TerminalTraceStructuralSchema, TerminalTraceValueComparison,
};

use crate::{TerminalScalarValue, TerminalStructuralValue};

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
        scalar_type: semantic_vocabulary::IntegerType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTraceStructuralValueSide {
    Expected,
    Actual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTraceStructuralComparisonError {
    UnsupportedProjectedStructuralSchema,
    StructuralTypeMismatch {
        side: TerminalTraceStructuralValueSide,
        schema: StructuralTypeId,
        value: StructuralTypeId,
    },
    StructuralQualificationsNonCanonical {
        side: TerminalTraceStructuralValueSide,
    },
    StructuralQualificationMissing {
        side: TerminalTraceStructuralValueSide,
        domain: StructuralDomainId,
    },
    NestedStructuralValue {
        side: TerminalTraceStructuralValueSide,
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

/// Compare two whole-root opaque structural runtime values under one
/// verifier-derived `TerminalTraceV1` schema.
///
/// This first structural rung validates the exact structural type and every
/// whole-root qualification required by the schema before comparing the
/// complete opaque values. Projected schema qualifications and runtime
/// subpaths remain unsupported until a nested structural-value carrier exists.
pub fn compare_terminal_trace_structural_values(
    schema: &TerminalTraceStructuralSchema,
    expected: &TerminalStructuralValue,
    actual: &TerminalStructuralValue,
) -> Result<bool, TerminalTraceStructuralComparisonError> {
    match schema.comparison {
        TerminalTraceValueComparison::ExactSemanticValue => {}
    }
    if !schema.projected_qualifications.is_empty() {
        return Err(TerminalTraceStructuralComparisonError::UnsupportedProjectedStructuralSchema);
    }
    validate_structural_value(schema, expected, TerminalTraceStructuralValueSide::Expected)?;
    validate_structural_value(schema, actual, TerminalTraceStructuralValueSide::Actual)?;
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

fn validate_structural_value(
    schema: &TerminalTraceStructuralSchema,
    value: &TerminalStructuralValue,
    side: TerminalTraceStructuralValueSide,
) -> Result<(), TerminalTraceStructuralComparisonError> {
    if value.structural_type != schema.structural_type {
        return Err(
            TerminalTraceStructuralComparisonError::StructuralTypeMismatch {
                side,
                schema: schema.structural_type,
                value: value.structural_type,
            },
        );
    }
    if value
        .qualifications
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(
            TerminalTraceStructuralComparisonError::StructuralQualificationsNonCanonical { side },
        );
    }
    if let Some(domain) = schema
        .qualifications
        .iter()
        .find(|domain| !value.qualifications.contains(domain))
    {
        return Err(
            TerminalTraceStructuralComparisonError::StructuralQualificationMissing {
                side,
                domain: *domain,
            },
        );
    }
    if !value.path.is_empty() {
        return Err(TerminalTraceStructuralComparisonError::NestedStructuralValue { side });
    }
    Ok(())
}
