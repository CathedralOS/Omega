use super::expressions::project_boolean_expression;
use super::project_crash_cause;
use crate::record::{
    PackageReviewArithmeticDomain, PackageReviewBooleanExpression, PackageReviewCrashCause,
    PackageReviewIntegerComparisonKind, PackageReviewPrimitiveType, PackageReviewScalarExpression,
};

#[test]
fn structural_runtime_requirement_crosses_as_closed_review_evidence() {
    let source = psi_checked_trees::CheckedBooleanExpression::IntegerComparison {
        kind: psi_checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
        left: Box::new(psi_checked_trees::CheckedScalarExpression::IntegerLiteral {
            literal: psi_numerics::literals::IntegerLiteral::from_value(7).with_landing(
                psi_numerics::literals::IntegerLanding {
                    landed_type: psi_numerics::literals::LandedIntegerType::U32,
                    domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                },
            ),
        }),
        right: Box::new(
            psi_checked_trees::CheckedScalarExpression::IntegerExactCast {
                primitive_type: psi_typed_trees::types::PrimitiveType::U32,
                operand: Box::new(psi_checked_trees::CheckedScalarExpression::Parameter {
                    position: 3,
                    primitive_type: psi_typed_trees::types::PrimitiveType::U16,
                }),
                range: psi_checked_trees::CheckedIntegerRange {
                    minimum: psi_numerics::bignum::BigInt::from_u64(0),
                    maximum: psi_numerics::bignum::BigInt::from_u64(u32::MAX as u64),
                },
            },
        ),
    };

    let PackageReviewBooleanExpression::IntegerComparison { kind, left, right } =
        project_boolean_expression(&source)
    else {
        panic!("integer comparison must retain its closed review shape")
    };
    assert_eq!(kind, PackageReviewIntegerComparisonKind::LessOrEqual);
    assert!(matches!(
        left.as_ref(),
        PackageReviewScalarExpression::IntegerLiteral(literal)
            if literal.canonical_text() == "7"
                && literal.landing().is_some_and(|landing|
                    landing.landed_type() == PackageReviewPrimitiveType::U32
                        && landing.arithmetic_domain() == PackageReviewArithmeticDomain::Exact)
    ));
    assert!(matches!(
        right.as_ref(),
        PackageReviewScalarExpression::IntegerExactCast {
            primitive_type: PackageReviewPrimitiveType::U32,
            operand,
            range,
        } if range.minimum() == "0"
            && range.maximum() == u32::MAX.to_string()
            && matches!(
                operand.as_ref(),
                PackageReviewScalarExpression::Parameter {
                    position: 3,
                    primitive_type: PackageReviewPrimitiveType::U16,
                }
            )
    ));
}

#[test]
fn crash_cause_crosses_the_review_boundary_as_closed_evidence() {
    assert_eq!(
        project_crash_cause(psi_checked_trees::CrashCause::Trap),
        PackageReviewCrashCause::Trap,
    );
    assert_eq!(
        project_crash_cause(psi_checked_trees::CrashCause::Abort),
        PackageReviewCrashCause::Abort,
    );
}
