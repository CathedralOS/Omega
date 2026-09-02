//! Signed-I64 producer/replay coverage over the shared comparison fixture.

use super::*;

fn fixture() -> Fixture {
    let mut fixture = super::fixture();
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    for parameter in &mut fixture.abstracted.parameters {
        parameter.scalar_type = ScalarType::Integer(i64_type);
    }
    for parameter in &mut fixture.optimized.parameters {
        parameter.scalar_type = ScalarType::Integer(i64_type);
    }
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::IntegerLessThan { scalar_type, .. },
        ..
    } = &mut fixture.target.operation
    else {
        unreachable!("comparison fixture")
    };
    *scalar_type = i64_type;
    fixture
}

#[test]
fn condition_is_produced_and_independently_replayed() {
    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("I64 strict less-than source condition");
    assert_eq!(
        derived.shape,
        ScalarConditionShape::IntegerLessThanI64Parameters
    );
    let LegalizedCondition::I64LessThanParametersV1 {
        operation,
        left,
        right,
        ..
    } = &derived.legalized
    else {
        panic!("I64 strict less-than custody")
    };
    assert_eq!(*operation, fixture.operation);
    assert_eq!(left.source_value, fixture.left);
    assert_eq!(right.source_value, fixture.right);

    let replayed = replay::replay_condition_for_test(
        0,
        Architecture::X86_64,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
        derived.source,
        &derived.legalized,
    )
    .expect("independent I64 strict less-than replay");
    assert_eq!(
        replayed.shape,
        ScalarConditionShape::IntegerLessThanI64Parameters
    );
}

#[test]
fn replay_rejects_operand_order_corruption() {
    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("I64 strict less-than source condition");
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::I64LessThanParametersV1 { left, right, .. } = &mut corrupted else {
        panic!("I64 strict less-than custody")
    };
    std::mem::swap(left, right);
    assert_eq!(
        replay::replay_condition_for_test(
            0,
            Architecture::X86_64,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
            derived.source,
            &corrupted,
        )
        .map(|_| ()),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
}
