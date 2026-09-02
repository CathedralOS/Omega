use psi_core::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
    StructuralDomainId, StructuralTypeId,
};
use psi_terminal::{
    StructuralAccess, StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
    TerminalTraceScalarSchema, TerminalTraceStructuralSchema, TerminalTraceValueComparison,
};
use psi_terminal_interpreter::{
    TerminalScalarValue, TerminalStructuralValue, TerminalTraceScalarComparisonError,
    TerminalTraceScalarValueSide, TerminalTraceStructuralComparisonError,
    TerminalTraceStructuralValueSide, compare_terminal_trace_scalar_values,
    compare_terminal_trace_structural_values,
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

fn structural_type(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).expect("structural type ID")
}

fn structural_domain(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).expect("structural domain ID")
}

fn structural_schema(
    structural_type: StructuralTypeId,
    qualifications: Vec<StructuralDomainId>,
) -> TerminalTraceStructuralSchema {
    TerminalTraceStructuralSchema {
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications,
        projected_qualifications: Vec::new(),
        comparison: TerminalTraceValueComparison::ExactSemanticValue,
    }
}

fn structural_value(
    opaque_identity: u64,
    structural_type: StructuralTypeId,
    qualifications: Vec<StructuralDomainId>,
) -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity,
        structural_type,
        qualifications,
        path: Vec::new(),
    }
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

#[test]
fn exact_terminal_trace_structural_values_compare_complete_whole_root_identity() {
    let value_type = structural_type(1);
    let first_domain = structural_domain(1);
    let second_domain = structural_domain(2);
    let schema = structural_schema(value_type, vec![first_domain]);
    let expected = structural_value(41, value_type, vec![first_domain, second_domain]);

    assert_eq!(
        compare_terminal_trace_structural_values(&schema, &expected, &expected),
        Ok(true),
    );
    assert_eq!(
        compare_terminal_trace_structural_values(
            &schema,
            &expected,
            &structural_value(42, value_type, vec![first_domain, second_domain]),
        ),
        Ok(false),
        "different opaque semantic identities remain unequal",
    );
    assert_eq!(
        compare_terminal_trace_structural_values(
            &schema,
            &expected,
            &structural_value(41, value_type, vec![first_domain]),
        ),
        Ok(false),
        "the complete runtime qualification roster participates in exact value equality",
    );
}

#[test]
fn exact_terminal_trace_structural_comparison_rejects_type_and_qualification_drift() {
    let value_type = structural_type(1);
    let other_type = structural_type(2);
    let first_domain = structural_domain(1);
    let second_domain = structural_domain(2);
    let schema = structural_schema(value_type, vec![first_domain]);
    let expected = structural_value(41, value_type, vec![first_domain]);

    assert_eq!(
        compare_terminal_trace_structural_values(
            &schema,
            &expected,
            &structural_value(41, other_type, vec![first_domain]),
        ),
        Err(
            TerminalTraceStructuralComparisonError::StructuralTypeMismatch {
                side: TerminalTraceStructuralValueSide::Actual,
                schema: value_type,
                value: other_type,
            }
        ),
    );
    assert_eq!(
        compare_terminal_trace_structural_values(
            &schema,
            &structural_value(41, value_type, vec![second_domain]),
            &expected,
        ),
        Err(
            TerminalTraceStructuralComparisonError::StructuralQualificationMissing {
                side: TerminalTraceStructuralValueSide::Expected,
                domain: first_domain,
            }
        ),
    );
    assert_eq!(
        compare_terminal_trace_structural_values(
            &schema,
            &structural_value(41, value_type, vec![second_domain, first_domain]),
            &expected,
        ),
        Err(
            TerminalTraceStructuralComparisonError::StructuralQualificationsNonCanonical {
                side: TerminalTraceStructuralValueSide::Expected,
            }
        ),
    );
}

#[test]
fn exact_terminal_trace_structural_comparison_fences_projected_and_nested_values() {
    let value_type = structural_type(1);
    let domain = structural_domain(1);
    let expected = structural_value(41, value_type, vec![domain]);
    let mut projected = structural_schema(value_type, vec![domain]);
    projected
        .projected_qualifications
        .push(StructuralPathQualification {
            path: vec![StructuralPathSegment::Field("item".into())],
            domain,
        });
    assert_eq!(
        compare_terminal_trace_structural_values(&projected, &expected, &expected),
        Err(TerminalTraceStructuralComparisonError::UnsupportedProjectedStructuralSchema),
    );

    let schema = structural_schema(value_type, vec![domain]);
    let mut nested = expected.clone();
    nested.path.push(StructuralPathSegment::FixedIndex(0));
    assert_eq!(
        compare_terminal_trace_structural_values(&schema, &nested, &expected),
        Err(
            TerminalTraceStructuralComparisonError::NestedStructuralValue {
                side: TerminalTraceStructuralValueSide::Expected,
            }
        ),
    );
}
