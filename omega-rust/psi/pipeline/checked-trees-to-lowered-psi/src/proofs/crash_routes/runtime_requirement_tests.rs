//! Runtime requirement lowering tests.

use super::{CheckedBooleanExpression, ValueDeclaration, value_id};
use crate::proofs::{
    CheckedIntegerComparisonKind, CheckedScalarExpression, PrimitiveType, Proposition, ScalarTerm,
    integer_scalar_type,
};
use crate::unit::runtime_requirements::lower_structural_runtime_requirement;

#[test]
fn runtime_requirement_equality_orders_literal_and_formal_without_reversing_inequalities() {
    let scalar_type = integer_scalar_type(PrimitiveType::U64).expect("integer type");
    let formal = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(7),
        scalar_type,
    };
    let literal = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(1).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U64,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    for kind in [
        CheckedIntegerComparisonKind::Equal,
        CheckedIntegerComparisonKind::LessThan,
        CheckedIntegerComparisonKind::LessOrEqual,
    ] {
        let expression = CheckedBooleanExpression::IntegerComparison {
            kind,
            left: Box::new(literal.clone()),
            right: Box::new(parameter.clone()),
        };
        let proposition = lower_structural_runtime_requirement(
            &expression,
            std::slice::from_ref(&formal),
            &[],
            &[],
            &[],
        )
        .expect("bounded runtime requirement");
        match proposition {
            Proposition::Equal(left, right) => {
                assert!(matches!(left, ScalarTerm::Value { id, .. } if id == formal.id));
                assert!(matches!(right, ScalarTerm::Integer { .. }));
            }
            Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                assert!(matches!(left, ScalarTerm::Integer { .. }));
                assert!(matches!(right, ScalarTerm::Value { id, .. } if id == formal.id));
            }
            _ => panic!("integer comparison"),
        }
    }
}
