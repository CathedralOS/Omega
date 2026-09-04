//! Signed-I64 inclusive-comparison producer and independent replay boundaries.

use super::*;

fn fixture() -> Fixture {
    let mut fixture = super::less_or_equal_fixture();
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    for parameter in &mut fixture.abstracted.parameters {
        parameter.scalar_type = ScalarType::Integer(i64_type);
    }
    for parameter in &mut fixture.optimized.parameters {
        parameter.scalar_type = ScalarType::Integer(i64_type);
    }
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::IntegerLessOrEqual { scalar_type, .. },
        ..
    } = &mut fixture.target.operation
    else {
        unreachable!("inclusive comparison fixture")
    };
    *scalar_type = i64_type;
    fixture
}

fn derived(fixture: &Fixture) -> source::conditions::DerivedCondition<'_> {
    source::derive_condition_for_test(0, &fixture.target, &fixture.abstracted, &fixture.optimized)
        .expect("I64 inclusive source condition")
}

fn assert_replay_rejects(fixture: &Fixture, proposed: &LegalizedCondition) {
    assert!(
        replay::replay_condition_for_test(
            0,
            Architecture::X86_64,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
            fixture.condition,
            proposed,
        )
        .is_err(),
        "independent replay accepted corrupted signed-inclusive custody"
    );
}

#[test]
fn condition_is_produced_and_independently_replayed() {
    let fixture = fixture();
    let derived = derived(&fixture);
    assert_eq!(derived.source, fixture.condition);
    assert_eq!(
        derived.shape,
        ScalarConditionShape::IntegerLessOrEqualI64Parameters
    );
    let LegalizedCondition::I64LessOrEqualParametersV1 {
        operation,
        left,
        right,
        fuel,
        ..
    } = &derived.legalized
    else {
        panic!("I64 inclusive custody")
    };
    assert_eq!(*operation, fixture.operation);
    assert_eq!((left.source_value, left.parameter_index), (fixture.left, 0));
    assert_eq!(
        (right.source_value, right.parameter_index),
        (fixture.right, 1)
    );
    assert_eq!(fuel.len(), 1);

    let replayed = replay::replay_condition_for_test(
        0,
        Architecture::X86_64,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
        derived.source,
        &derived.legalized,
    )
    .expect("independent I64 inclusive replay");
    assert_eq!(replayed.source, fixture.condition);
    assert_eq!(
        replayed.shape,
        ScalarConditionShape::IntegerLessOrEqualI64Parameters
    );
}

#[test]
fn signedness_and_predicate_boundaries_are_distinct() {
    let unsigned = super::less_or_equal_fixture();
    let unsigned_derived = derived(&unsigned);
    assert_eq!(
        unsigned_derived.shape,
        ScalarConditionShape::IntegerLessOrEqualU64Parameters
    );
    assert!(matches!(
        unsigned_derived.legalized,
        LegalizedCondition::IntegerLessOrEqualParametersV1 { .. }
    ));

    let signed = fixture();
    let signed_derived = derived(&signed);
    let LegalizedCondition::I64LessOrEqualParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left,
        right,
    } = signed_derived.legalized
    else {
        panic!("I64 inclusive custody")
    };
    assert_replay_rejects(
        &signed,
        &LegalizedCondition::IntegerLessOrEqualParametersV1 {
            operation,
            result_definition_site,
            fuel: fuel.clone(),
            left: left.clone(),
            right: right.clone(),
        },
    );
    assert_replay_rejects(
        &signed,
        &LegalizedCondition::I64LessThanParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        },
    );
}

#[test]
fn reversed_signed_less_is_exact_at_equality_and_sign_boundaries() {
    let inclusive_via_reversed_less = |left: i64, right: i64| right >= left;
    for (left, right) in [
        (i64::MIN, i64::MIN),
        (i64::MAX, i64::MAX),
        (-1, -1),
        (0, 0),
        (i64::MIN, i64::MAX),
        (-1, 0),
        (0, -1),
        (i64::MAX, i64::MIN),
    ] {
        assert_eq!(inclusive_via_reversed_less(left, right), left <= right);
    }
}

#[test]
fn replay_rejects_every_retained_condition_field_corruption() {
    let fixture = fixture();
    let canonical = derived(&fixture).legalized;
    let mut corruptions = Vec::new();

    let mut corrupted = canonical.clone();
    let LegalizedCondition::I64LessOrEqualParametersV1 { operation, .. } = &mut corrupted else {
        unreachable!()
    };
    *operation = id(99);
    corruptions.push(corrupted);

    let mut corrupted = canonical.clone();
    let LegalizedCondition::I64LessOrEqualParametersV1 {
        result_definition_site,
        ..
    } = &mut corrupted
    else {
        unreachable!()
    };
    *result_definition_site = ValueDefinitionSite::FunctionParameter(0);
    corruptions.push(corrupted);

    let mut corrupted = canonical.clone();
    let LegalizedCondition::I64LessOrEqualParametersV1 { fuel, .. } = &mut corrupted else {
        unreachable!()
    };
    fuel[0].units += 1;
    corruptions.push(corrupted);

    for select_left in [true, false] {
        for field in 0..4 {
            let mut corrupted = canonical.clone();
            let LegalizedCondition::I64LessOrEqualParametersV1 { left, right, .. } = &mut corrupted
            else {
                unreachable!()
            };
            let parameter = if select_left { left } else { right };
            match field {
                0 => parameter.source_value = id(99),
                1 => parameter.parameter_index ^= 1,
                2 => parameter.register = MachineRegister::X86Rdx,
                3 => parameter.definition_site = ValueDefinitionSite::FunctionParameter(9),
                _ => unreachable!(),
            }
            corruptions.push(corrupted);
        }
    }

    let mut swapped = canonical;
    let LegalizedCondition::I64LessOrEqualParametersV1 { left, right, .. } = &mut swapped else {
        unreachable!()
    };
    std::mem::swap(left, right);
    corruptions.push(swapped);

    for corrupted in &corruptions {
        assert_replay_rejects(&fixture, corrupted);
    }
}
