use crate::*;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, MachineId, ScalarTerm, ScalarType, ValueId,
    WidthParameter,
};

fn parameter(sign: IntegerSign) -> IntegerType {
    IntegerType::parameterized(
        sign,
        WidthParameter {
            owner: MachineId::new(1).unwrap(),
            value: ValueId::new(2).unwrap(),
        },
    )
}

#[test]
fn symbolic_width_grants_no_storage_or_recast_geometry() {
    let unsigned = parameter(IntegerSign::Unsigned);
    let signed = parameter(IntegerSign::Signed);
    let scalar = ScalarType::Integer(unsigned);
    assert_eq!(reference_storage_byte_width(scalar), None);
    assert_eq!(
        reference_field_representation_width(
            std::iter::empty(),
            &terminal_psi::StructuralFieldType::Scalar(scalar),
        ),
        None
    );
    assert!(!scalar_reference_view_is_total(
        scalar,
        ScalarType::Integer(signed)
    ));
    assert!(ScalarRepresentationViewPair::classify(ScalarType::Boolean, scalar).is_none());
    assert_eq!(
        scalar_reference_view_value(
            scalar,
            scalar,
            ScalarViewValue::Integer(IntegerValue::Unsigned(0)),
        ),
        None
    );
}

#[test]
fn symbolic_endpoint_and_shift_goals_refuse_concrete_authority() {
    let integer = parameter(IntegerSign::Unsigned);
    let value = ScalarTerm::Value {
        id: ValueId::new(3).unwrap(),
        scalar_type: ScalarType::Integer(integer),
    };
    let closed = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for goal in [
        CanonicalScalarGoal::ExactCastRepresentable {
            source_type: integer,
            target_type: closed,
            operand: value.clone(),
        },
        CanonicalScalarGoal::ExactShiftCount {
            value_type: integer,
            count_type: closed,
            count: ScalarTerm::integer(closed, IntegerValue::Unsigned(0)).unwrap(),
        },
        CanonicalScalarGoal::NonzeroDivisor {
            integer_type: integer,
            divisor: value,
        },
    ] {
        assert_eq!(
            goal.kernel_proposition(),
            Err(OperationSemanticError::UnresolvedIntegerWidth(integer))
        );
    }
}

#[test]
fn closed_geometry_and_one_bit_signed_divisor_keep_their_meaning() {
    let byte = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    assert_eq!(reference_storage_byte_width(byte), Some(1));
    assert!(ScalarRepresentationViewPair::classify(ScalarType::Boolean, byte).is_some());
    let integer = IntegerType::new(IntegerSign::Signed, 1).unwrap();
    let divisor = ScalarTerm::Value {
        id: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Integer(integer),
    };
    let proposition = CanonicalScalarGoal::NonzeroDivisor {
        integer_type: integer,
        divisor,
    }
    .kernel_proposition()
    .unwrap();
    assert!(matches!(
        proposition,
        semantic_vocabulary::Proposition::LessOrEqual(_, _)
    ));
    proposition.validate().unwrap();
}
