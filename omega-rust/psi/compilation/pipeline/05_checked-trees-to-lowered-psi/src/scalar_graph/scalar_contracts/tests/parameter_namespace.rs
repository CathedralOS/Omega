use super::super::super::{CheckedIntegerComparisonKind, PrimitiveType};
use super::super::{
    CheckedBooleanExpression, CheckedScalarExpression, ClosedScalarContractValue,
    ContractViewNamespace, LoweringError, ValueDeclaration, clauses, proposition,
};
use super::{IntegerSign, IntegerType, ScalarType, ValueId};
use numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLanding, IntegerLiteral, LandedIntegerType},
};

fn namespace() -> Vec<ValueDeclaration> {
    [
        (101, ScalarType::Boolean),
        (
            203,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
        ),
        (
            307,
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        ),
        (
            409,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
        ),
    ]
    .into_iter()
    .map(|(identity, scalar_type)| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(identity).unwrap(),
        scalar_type,
    })
    .collect()
}

fn parameter(position: usize, primitive_type: PrimitiveType) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    }
}

fn literal(value: i64, landed_type: LandedIntegerType) -> CheckedScalarExpression {
    CheckedScalarExpression::IntegerLiteral {
        literal: IntegerLiteral::from_value(value).with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    }
}

fn compare(
    kind: CheckedIntegerComparisonKind,
    left: CheckedScalarExpression,
    right: CheckedScalarExpression,
) -> CheckedBooleanExpression {
    CheckedBooleanExpression::IntegerComparison {
        kind,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn clause(predicate: CheckedBooleanExpression) -> Option<ClosedScalarContractValue> {
    Some(ClosedScalarContractValue::Predicate(predicate))
}
