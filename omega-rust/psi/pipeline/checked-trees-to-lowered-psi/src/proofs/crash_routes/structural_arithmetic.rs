//! Safe structural divisors and shift counts.

use crate::proofs::{IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType};

pub(crate) fn safe_exact_structural_divisor(
    integer_type: IntegerType,
    dividend: &ScalarTerm,
    divisor: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    match divisor {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } => return *scalar_type == integer_type && *value != 0,
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } => return *scalar_type == integer_type && *value != 0 && *value != -1,
        _ => {}
    }

    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    if let Ok(one) = ScalarTerm::integer(integer_type, one)
        && requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
    {
        return true;
    }
    if integer_type.sign() != IntegerSign::Signed {
        return false;
    }
    if let Ok(negative_two) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2))
        && requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_two))
    {
        return true;
    }
    let Ok(negative_one) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1)) else {
        return false;
    };
    if !requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_one)) {
        return false;
    }
    let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
        unreachable!("signed fixed integer has a signed minimum")
    };
    ScalarTerm::integer(
        integer_type,
        IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
    )
    .is_ok_and(|minimum_plus_one| {
        requirements.contains(&Proposition::LessOrEqual(
            minimum_plus_one,
            dividend.clone(),
        ))
    })
}

pub(crate) fn safe_policy_structural_divisor(
    integer_type: IntegerType,
    divisor: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    match divisor {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } => return *scalar_type == integer_type && *value != 0,
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } => return *scalar_type == integer_type && *value != 0,
        _ => {}
    }

    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    if ScalarTerm::integer(integer_type, one)
        .is_ok_and(|one| requirements.contains(&Proposition::LessOrEqual(one, divisor.clone())))
    {
        return true;
    }
    if integer_type.sign() != IntegerSign::Signed {
        return false;
    }
    [IntegerValue::Signed(-1), IntegerValue::Signed(-2)]
        .into_iter()
        .filter_map(|bound| ScalarTerm::integer(integer_type, bound).ok())
        .any(|bound| requirements.contains(&Proposition::LessOrEqual(divisor.clone(), bound)))
}

fn nonnegative_shift_count(value: IntegerValue) -> Option<u32> {
    match value {
        IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
        IntegerValue::Signed(value) => u32::try_from(value).ok(),
    }
}

fn exact_structural_shift_maximum_count(
    value_type: IntegerType,
    count_type: IntegerType,
    count: &ScalarTerm,
    requirements: &[Proposition],
) -> Option<u32> {
    if count.scalar_type() != ScalarType::Integer(count_type) {
        return None;
    }
    if let Some((literal_type, literal)) = count.integer_value() {
        let literal = nonnegative_shift_count(literal)?;
        return (literal_type == count_type && literal < u32::from(value_type.bits()))
            .then_some(literal);
    }

    if count_type.sign() == IntegerSign::Signed {
        let zero = ScalarTerm::integer(count_type, IntegerValue::Signed(0)).ok()?;
        if !requirements.contains(&Proposition::LessOrEqual(zero, count.clone())) {
            return None;
        }
    }

    let width = u32::from(value_type.bits());
    let intrinsic_maximum = nonnegative_shift_count(count_type.maximum_value())?;
    if intrinsic_maximum < width {
        return Some(intrinsic_maximum);
    }
    requirements
        .iter()
        .filter_map(|requirement| match requirement {
            Proposition::LessOrEqual(left, right) if left == count => {
                let (right_type, right) = right.integer_value()?;
                let right = nonnegative_shift_count(right)?;
                (right_type == count_type && right < width).then_some(right)
            }
            Proposition::LessThan(left, right) if left == count => {
                let (right_type, right) = right.integer_value()?;
                let right = nonnegative_shift_count(right)?;
                (right_type == count_type && right > 0 && right <= width).then_some(right - 1)
            }
            _ => None,
        })
        .min()
}

pub(crate) fn safe_exact_structural_shift(
    left_shift: bool,
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    if value.scalar_type() != ScalarType::Integer(value_type) {
        return false;
    }
    let Some(maximum_count) =
        exact_structural_shift_maximum_count(value_type, count_type, count, requirements)
    else {
        return false;
    };
    if !left_shift || maximum_count == 0 {
        return true;
    }
    if let Some((literal_type, literal)) = value.integer_value() {
        let maximum_count_value = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum_count)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum_count)),
        };
        return literal_type == value_type
            && value_type
                .exact_shift_left(literal, count_type, maximum_count_value)
                .is_some();
    }
    match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer has an unsigned maximum")
            };
            ScalarTerm::integer(value_type, IntegerValue::Unsigned(maximum >> maximum_count))
                .is_ok_and(|maximum| {
                    requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                })
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer has signed bounds")
            };
            let minimum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(minimum >> maximum_count));
            let maximum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(maximum >> maximum_count));
            minimum.is_ok_and(|minimum| {
                requirements.contains(&Proposition::LessOrEqual(minimum, value.clone()))
            }) && maximum.is_ok_and(|maximum| {
                requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
            })
        }
    }
}
