//! The retained re-view replays its carrier relation; a forged or
//! substituted judgment rejects instead of lowering.

use super::replay;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarType, StructuralFieldId,
};

fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(sign, bits).unwrap())
}

fn field_read(scalar_type: ScalarType) -> LoweredDirectExpression {
    LoweredDirectExpression::StructuralField {
        source: PlaceId::new(41).unwrap(),
        path: Vec::new(),
        field: StructuralFieldId::new(7).unwrap(),
        scalar_type,
    }
}

fn error(result: Result<LoweredDirectExpression, crate::lowering_error::LoweringError>) -> String {
    match result {
        Ok(_) => panic!("a malformed re-view must reject"),
        Err(error) => format!("{error:?}"),
    }
}

#[test]
fn equal_width_integer_view_reads_the_declared_field_then_its_modular_image() {
    let signed = integer(IntegerSign::Signed, 32);
    let unsigned = integer(IntegerSign::Unsigned, 32);
    let lowered = replay(field_read(signed), unsigned).expect("i32 bits re-view as u32");
    assert_eq!(lowered.scalar_type(), unsigned);
    // The field observation keeps its declared carrier beneath the image.
    fn reads_declared_field(expression: &LoweredDirectExpression, declared: ScalarType) -> bool {
        match expression {
            LoweredDirectExpression::StructuralField { scalar_type, .. } => {
                *scalar_type == declared
            }
            LoweredDirectExpression::IntegerBinary { left, right, .. } => {
                reads_declared_field(left, declared) || reads_declared_field(right, declared)
            }
            LoweredDirectExpression::IntegerWiden { operand, .. }
            | LoweredDirectExpression::IntegerExactCast { operand, .. }
            | LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
                reads_declared_field(operand, declared)
            }
            _ => false,
        }
    }
    assert!(reads_declared_field(&lowered, signed));

    let narrow_unsigned = integer(IntegerSign::Unsigned, 8);
    let narrow_signed = integer(IntegerSign::Signed, 8);
    assert_eq!(
        replay(field_read(narrow_unsigned), narrow_signed)
            .expect("u8 bits re-view as i8")
            .scalar_type(),
        narrow_signed
    );
}

#[test]
fn identity_width_changing_and_cross_family_views_reject() {
    let signed = integer(IntegerSign::Signed, 32);
    assert!(error(replay(field_read(signed), signed)).contains("must change its carrier"));
    assert!(
        error(replay(
            field_read(signed),
            integer(IntegerSign::Unsigned, 16)
        ))
        .contains("carrier width")
    );
    let float = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    assert!(
        error(replay(
            field_read(float),
            integer(IntegerSign::Unsigned, 32)
        ))
        .contains("no Terminal Psi vocabulary")
    );
    assert!(
        error(replay(
            field_read(integer(IntegerSign::Unsigned, 8)),
            ScalarType::Boolean
        ))
        .contains("no Terminal Psi vocabulary")
    );
}

#[test]
fn a_computed_operand_cannot_stand_in_for_the_viewed_place() {
    let signed = integer(IntegerSign::Signed, 32);
    let substituted = LoweredDirectExpression::IntegerLiteral {
        value: IntegerValue::Signed(-1),
        scalar_type: signed,
    };
    assert!(
        error(replay(substituted, integer(IntegerSign::Unsigned, 32))).contains("one scalar place")
    );
}
