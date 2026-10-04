use super::*;
use checked_trees::CheckedIntegerRange;
use facts::ScalarValue;
use semantic_vocabulary::IntegerSign;

fn write(primitive: PrimitiveType) -> PerformedIntegerWrite {
    PerformedIntegerWrite {
        primitive_type: Some(primitive),
        entry_value: Some(Scalar::Parameter {
            position: 0,
            primitive_type: primitive,
        }),
        ..Default::default()
    }
}

fn stored(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("physical integer")
}

fn evaluate(predicate: Boolean, value: BigInt, guard: bool) -> Option<bool> {
    let value = crate::values::evaluate_checked_scalar(
        &Scalar::Boolean(Box::new(predicate)),
        &mut |position| match position {
            0 => Some(ScalarValue::Integer(value.clone())),
            1 => Some(ScalarValue::Boolean(guard)),
            _ => None,
        },
    )?;
    match value {
        ScalarValue::Boolean(value) => Some(value),
        _ => None,
    }
}

#[test]
fn physical_write_predicates_keep_signed_and_unsigned_endpoints() {
    for (primitive, sign, cases) in [
        (
            PrimitiveType::I64,
            IntegerSign::Signed,
            [(-129, false), (-128, true), (127, true), (128, false)],
        ),
        (
            PrimitiveType::I64,
            IntegerSign::Unsigned,
            [(-1, false), (0, true), (255, true), (256, false)],
        ),
        (
            PrimitiveType::U64,
            IntegerSign::Signed,
            [(0, true), (127, true), (128, false), (255, false)],
        ),
    ] {
        let predicate = integer_write_obligation(&write(primitive), stored(sign, 8)).unwrap();
        for (value, expected) in cases {
            assert_eq!(
                evaluate(predicate.clone(), BigInt::from_i128(value), true),
                Some(expected)
            );
        }
    }
}

#[test]
fn physical_write_guard_selects_the_occurrence_without_granting_fit() {
    let mut write = write(PrimitiveType::I64);
    write.entry_condition = Some(Boolean::Parameter { position: 1 });
    let predicate = integer_write_obligation(&write, stored(IntegerSign::Signed, 8)).unwrap();
    assert_eq!(
        evaluate(predicate.clone(), BigInt::from_i128(128), false),
        Some(true)
    );
    assert_eq!(
        evaluate(predicate.clone(), BigInt::from_i128(128), true),
        Some(false)
    );
    assert_eq!(
        evaluate(predicate, BigInt::from_i128(127), true),
        Some(true)
    );
}

#[test]
fn occurrence_range_can_discharge_an_unknown_value_but_missing_evidence_cannot() {
    let mut write = write(PrimitiveType::I64);
    write.entry_value = None;
    let width = stored(IntegerSign::Signed, 8);
    assert_eq!(integer_write_obligation(&write, width), None);
    write.entry_condition = Some(Boolean::Constant(false));
    assert_eq!(integer_write_obligation(&write, width), None);
    write.integer_range = Some(CheckedIntegerRange {
        minimum: BigInt::from_i128(-128),
        maximum: BigInt::from_i128(127),
    });
    assert_eq!(
        integer_write_obligation(&write, width),
        Some(Boolean::Constant(true))
    );
    write.integer_range.as_mut().unwrap().maximum = BigInt::from_i128(128);
    assert_eq!(integer_write_obligation(&write, width), None);
    write.integer_range.as_mut().unwrap().minimum = BigInt::from_i128(129);
    assert_eq!(
        integer_write_obligation(&write, width),
        None,
        "invalid range is not an unreachable write"
    );
}

#[test]
fn semantic_carrier_bounds_do_not_truncate_full_width_unsigned_values() {
    let mut unsigned = write(PrimitiveType::U64);
    unsigned.entry_value = None;
    assert_eq!(
        integer_write_obligation(&unsigned, stored(IntegerSign::Unsigned, 64)),
        Some(Boolean::Constant(true))
    );
    unsigned.integer_range = Some(CheckedIntegerRange {
        minimum: BigInt::from_u128(u64::MAX as u128),
        maximum: BigInt::from_u128(u64::MAX as u128),
    });
    assert_eq!(
        integer_write_obligation(&unsigned, stored(IntegerSign::Signed, 64)),
        None
    );
    unsigned.entry_value = write(PrimitiveType::U64).entry_value;
    let predicate = integer_write_obligation(&unsigned, stored(IntegerSign::Signed, 64)).unwrap();
    assert_eq!(
        evaluate(predicate, BigInt::from_u128(u64::MAX as u128), true),
        Some(false)
    );
    // An unsigned physical carrier does not make negative semantic values fit.
    let signed = integer_write_obligation(
        &write(PrimitiveType::I64),
        stored(IntegerSign::Unsigned, 64),
    )
    .unwrap();
    assert_eq!(evaluate(signed, BigInt::from_i128(-1), true), Some(false));
}

#[test]
fn physical_write_predicates_refuse_unsupported_geometry_and_mismatched_values() {
    assert_eq!(
        integer_write_obligation(&write(PrimitiveType::I64), stored(IntegerSign::Signed, 7)),
        None
    );
    assert_eq!(
        integer_write_obligation(&write(PrimitiveType::I64), stored(IntegerSign::Signed, 128)),
        None
    );
    let mut mismatched = write(PrimitiveType::I64);
    mismatched.entry_value = write(PrimitiveType::U64).entry_value;
    assert_eq!(
        integer_write_obligation(&mismatched, stored(IntegerSign::Signed, 8)),
        None
    );
}
