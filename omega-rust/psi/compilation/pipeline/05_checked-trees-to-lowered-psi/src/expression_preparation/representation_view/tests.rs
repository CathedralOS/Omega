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
        source: PlaceId::new(41).unwrap().into(),
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
fn identity_width_changing_and_bitless_views_reject() {
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
    // Equal byte counts alone admit nothing: a float against the other
    // format, a float against a Boolean and a float against a wider integer
    // have no bit view.
    for (source, view) in [
        (float, ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
        (float, ScalarType::Boolean),
        (float, integer(IntegerSign::Unsigned, 64)),
        (ScalarType::Boolean, integer(IntegerSign::Unsigned, 16)),
    ] {
        assert!(
            error(replay(field_read(source), view)).contains("without a bit view"),
            "{source:?} -> {view:?}"
        );
    }
}

/// The bits a re-view reads, its retained source observation and the
/// Terminal view carrier.
fn re_view(lowered: LoweredDirectExpression) -> (LoweredDirectExpression, ScalarType) {
    let LoweredDirectExpression::RepresentationView {
        scalar_type,
        operand,
    } = lowered
    else {
        panic!("a cross-family view is one Terminal re-view: {lowered:?}");
    };
    (*operand, scalar_type)
}

#[test]
fn float_and_boolean_views_re_view_the_declared_field_read() {
    for (source, view) in [
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            integer(IntegerSign::Unsigned, 32),
        ),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            integer(IntegerSign::Signed, 64),
        ),
        (
            integer(IntegerSign::Signed, 32),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ),
        (ScalarType::Boolean, integer(IntegerSign::Unsigned, 8)),
        (integer(IntegerSign::Unsigned, 8), ScalarType::Boolean),
        (integer(IntegerSign::Signed, 8), ScalarType::Boolean),
    ] {
        let (operand, carrier) = re_view(replay(field_read(source), view).expect("bit view"));
        assert_eq!(carrier, view);
        // The field observation keeps its declared carrier beneath the view.
        assert!(matches!(
            operand,
            LoweredDirectExpression::StructuralField { scalar_type, .. } if scalar_type == source
        ));
    }
}

#[test]
fn a_bool_view_of_a_runtime_selected_byte_is_one_landed_re_view() {
    let offset = LoweredDirectExpression::Local {
        position: 2,
        scalar_type: integer(IntegerSign::Unsigned, 64),
    };
    let (operand, carrier) = re_view(
        replay(byte_element_read(offset), ScalarType::Boolean)
            .expect("an indexed byte re-views as bool"),
    );
    assert_eq!(carrier, ScalarType::Boolean);
    assert!(matches!(
        operand,
        LoweredDirectExpression::IndexedPrimitiveRead { .. }
    ));
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

fn byte_element_read(index: LoweredDirectExpression) -> LoweredDirectExpression {
    LoweredDirectExpression::IndexedPrimitiveRead {
        source: PlaceId::new(41).unwrap(),
        path: Vec::new(),
        index: Box::new(index),
        inner_indexes: Vec::new(),
        element_path: Vec::new(),
        scalar_type: integer(IntegerSign::Unsigned, 8),
    }
}

/// Every byte read of an assembled region, in authored-footprint order.
fn byte_reads(expression: &LoweredDirectExpression, reads: &mut Vec<LoweredDirectExpression>) {
    match expression {
        LoweredDirectExpression::IndexedPrimitiveRead { index, .. } => {
            reads.push(index.as_ref().clone());
        }
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            byte_reads(left, reads);
            byte_reads(right, reads);
        }
        LoweredDirectExpression::IntegerWiden { operand, .. }
        | LoweredDirectExpression::IntegerExactCast { operand, .. }
        | LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => byte_reads(operand, reads),
        _ => {}
    }
}

#[test]
fn a_byte_region_view_reads_each_footprint_byte_from_its_first_element() {
    let offset = LoweredDirectExpression::Parameter {
        position: 0,
        scalar_type: integer(IntegerSign::Unsigned, 64),
    };
    let word = integer(IntegerSign::Unsigned, 32);
    let lowered = replay(byte_element_read(offset.clone()), word).expect("u8 region as u32");
    assert_eq!(lowered.scalar_type(), word);
    let mut reads = Vec::new();
    byte_reads(&lowered, &mut reads);
    // The first byte keeps the authored selector; each later byte offsets it,
    // so each read owes its own `index < extent` obligation.
    assert_eq!(reads.len(), 4);
    assert!(matches!(
        &reads[0],
        LoweredDirectExpression::Parameter { position: 0, .. }
    ));
    for (byte, read) in reads.iter().enumerate().skip(1) {
        let LoweredDirectExpression::IntegerBinary { left, right, .. } = read else {
            panic!("byte {byte} offsets the authored selector: {read:?}");
        };
        assert!(matches!(
            left.as_ref(),
            LoweredDirectExpression::Parameter { position: 0, .. }
        ));
        assert!(matches!(right.as_ref(),
            LoweredDirectExpression::IntegerLiteral { value: IntegerValue::Unsigned(value), .. }
                if *value == byte as u128));
    }

    let signed = integer(IntegerSign::Signed, 16);
    assert_eq!(
        replay(byte_element_read(offset), signed)
            .expect("u8 region as i16")
            .scalar_type(),
        signed
    );
}

#[test]
fn a_byte_region_view_must_start_at_a_runtime_selected_byte() {
    // A whole `u8` field is not the first element of a byte region.
    assert!(
        error(replay(
            field_read(integer(IntegerSign::Unsigned, 8)),
            integer(IntegerSign::Unsigned, 32)
        ))
        .contains("one runtime-selected byte")
    );
}
