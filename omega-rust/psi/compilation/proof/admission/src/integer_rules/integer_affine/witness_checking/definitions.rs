use super::*;

pub(super) fn apply_definition(
    target: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&LandedInteger>,
    prior_axioms: &[Proposition],
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    if !matches!(target, ScalarTerm::Value { .. })
        || target.scalar_type() != ScalarType::Integer(integer_type)
    {
        return None;
    }
    let transformed = match expression {
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && (left.as_ref() == current || right.as_ref() == current) =>
        {
            let operand = if left.as_ref() == current {
                right
            } else {
                left
            };
            let (literal, used_landing) = signed_literal(operand, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_add(literal),
                used_landing,
                CheckedIntegerEndpointStep::Add(literal),
            )
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_sub(literal),
                used_landing,
                CheckedIntegerEndpointStep::Subtract(literal),
            )
        }
        // A multiply step is the same reading with the chain operand on
        // either side; the sibling operand supplies the literal.
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && (left.as_ref() == current || right.as_ref() == current) =>
        {
            let sibling = if left.as_ref() == current {
                right
            } else {
                left
            };
            let (literal, used_landing) = signed_literal(sibling, integer_type, landed)?;
            (
                coefficient.checked_mul(literal),
                offset.checked_mul(literal),
                used_landing,
                CheckedIntegerEndpointStep::Multiply(literal),
            )
        }
        // A division step is the truncating image under either policy, except
        // signed `MIN / -1`: its wrapped result is not that image, so the
        // wrapping spelling rejects the step rather than pretend.
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            if literal == 0 {
                return Some(Err(IntegerAffineWitnessError::ZeroDivisionLiteral));
            }
            if matches!(expression, ScalarTerm::WrappingIntegerDivide { .. })
                && integer_type.sign() == IntegerSign::Signed
                && literal == -1
            {
                return None;
            }
            (
                Some(coefficient),
                Some(offset),
                used_landing,
                CheckedIntegerEndpointStep::Divide(literal),
            )
        }
        // A remainder step keeps the dividend's sign under either policy, and
        // signed `MIN % -1` still has the ordinary remainder image (zero):
        // only a zero divisor prevents the bounded image.
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            if literal == 0 {
                return Some(Err(IntegerAffineWitnessError::ZeroDivisionLiteral));
            }
            (
                Some(coefficient),
                Some(offset),
                used_landing,
                CheckedIntegerEndpointStep::Remainder(literal),
            )
        }
        // `x & mask` carries no affine slope, but a non-negative mask pins
        // the whole image to `[0, mask]`: the result keeps only mask bits.
        // A negative signed mask leaves the sign bit settable, so the step
        // rejects rather than pretend the image is ordered.
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && (left.as_ref() == current || right.as_ref() == current) =>
        {
            let sibling = if left.as_ref() == current {
                right
            } else {
                left
            };
            if let Some((mask, used_landing)) = signed_literal(sibling, integer_type, landed) {
                if mask < 0 {
                    return Some(Err(IntegerAffineWitnessError::NegativeBitwiseAndMask));
                }
                (
                    Some(coefficient),
                    Some(offset),
                    used_landing,
                    CheckedIntegerEndpointStep::BitwiseAndMask(mask),
                )
            } else {
                let (mask, upper) = bounded_bitwise_sibling(sibling, integer_type, prior_axioms)?;
                (
                    Some(coefficient),
                    Some(offset),
                    false,
                    CheckedIntegerEndpointStep::BitwiseAndBoundedMask {
                        mask: mask.clone(),
                        upper,
                    },
                )
            }
        }
        // `x | s` carries no affine slope either, but once the sibling proves
        // non-negative the result never clears bits: `x | s >= x`, and no
        // bit above the wider operand bound is set. The mapped image is
        // `[lower_x, all_ones(bit_length(max(upper_x, upper_s)))]` — a
        // non-monotone operand reading the AND arm cannot express, since an
        // and-mask's image is `[0, mask]` whatever the operand carried.
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && (left.as_ref() == current || right.as_ref() == current) =>
        {
            let sibling_operand = if left.as_ref() == current {
                right
            } else {
                left
            };
            if let Some((sibling, used_landing)) =
                signed_literal(sibling_operand, integer_type, landed)
            {
                if sibling < 0 {
                    return Some(Err(IntegerAffineWitnessError::NegativeBitwiseOrSibling));
                }
                (
                    Some(coefficient),
                    Some(offset),
                    used_landing,
                    CheckedIntegerEndpointStep::BitwiseOrLiteralImage(sibling),
                )
            } else {
                let (operand, upper) =
                    bounded_bitwise_sibling(sibling_operand, integer_type, prior_axioms)?;
                (
                    Some(coefficient),
                    Some(offset),
                    false,
                    CheckedIntegerEndpointStep::BitwiseOrBoundedImage {
                        operand: operand.clone(),
                        upper,
                    },
                )
            }
        }
        // A wrapping sum is the exact sum except where it crosses the carrier
        // bound the addend's sign faces, so bound mapping demands no-wrap
        // evidence on exactly that side, for either carrier sign.
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && (left.as_ref() == current || right.as_ref() == current) =>
        {
            let (sibling, current_operand) = if left.as_ref() == current {
                (right, left)
            } else {
                (left, right)
            };
            let (literal, used_landing) = signed_literal(sibling, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_add(literal),
                used_landing,
                policy_add_step(expression, current_operand, literal),
            )
        }
        // Subtracting `c` uses the endpoint translation by `-c`,
        // on either carrier sign, so the step is the wrapping sum with the
        // negated literal: the side the difference faces still demands the
        // checked no-wrap evidence (`c <= operand` on an unsigned carrier).
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_sub(literal),
                used_landing,
                policy_add_step(expression, left, literal.checked_neg()?),
            )
        }
        _ => return None,
    };
    Some(match transformed {
        (Some(coefficient), Some(offset), used_landing, endpoint_step) => Ok((
            target.clone(),
            coefficient,
            offset,
            used_landing,
            endpoint_step,
        )),
        _ => Err(IntegerAffineWitnessError::CoefficientOverflow),
    })
}

/// Resolve a bitwise sibling operand that is not a literal. `x & mask`
/// keeps only mask bits and `x | sibling` sets no bit above the wider bound,
/// so both steps need the same two facts: prior axioms proving `0 <= sibling`
/// and `sibling <= upper`. The bounds may be stated on the operand itself or
/// on any term an earlier `Equal` axiom aliases it to; a sibling of unsigned
/// carrier already proves its own non-negativity.
fn bounded_bitwise_sibling<'a>(
    operand: &'a ScalarTerm,
    integer_type: IntegerType,
    prior_axioms: &'a [Proposition],
) -> Option<(&'a ScalarTerm, i128)> {
    let mut masks = vec![operand];
    for proposition in prior_axioms {
        let Proposition::Equal(left, right) = proposition else {
            continue;
        };
        if left == operand {
            masks.push(right);
        } else if right == operand {
            masks.push(left);
        }
    }
    for mask in masks {
        let Some(upper) = prior_axioms.iter().find_map(|proposition| {
            let Proposition::LessOrEqual(left, right) = proposition else {
                return None;
            };
            if left != mask {
                return None;
            }
            let (value_type, value) = right.integer_value()?;
            let upper = match value {
                IntegerValue::Signed(value) => value,
                IntegerValue::Unsigned(value) => i128::try_from(value).ok()?,
            };
            (value_type == integer_type && upper >= 0).then_some(upper)
        }) else {
            continue;
        };
        let nonnegative = matches!(
            mask.scalar_type(),
            ScalarType::Integer(kind) if kind.sign() == IntegerSign::Unsigned
        ) || prior_axioms.iter().any(|proposition| {
            let Proposition::LessOrEqual(left, right) = proposition else {
                return false;
            };
            right == mask
                && left.integer_value().is_some_and(|(value_type, value)| {
                    value_type == integer_type
                        && match value {
                            IntegerValue::Signed(value) => value == 0,
                            IntegerValue::Unsigned(value) => value == 0,
                        }
                })
        });
        if nonnegative {
            return Some((mask, upper));
        }
    }
    None
}

pub(super) fn apply_shift_definition(
    target: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    landed: Option<&LandedInteger>,
    definition_index: usize,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    if !matches!(target, ScalarTerm::Value { .. })
        || target.scalar_type() != ScalarType::Integer(integer_type)
        || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let (count_type, count, left) = match expression {
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } if *value_type == integer_type && value.as_ref() == current => {
            (*count_type, count.as_ref(), true)
        }
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } if *value_type == integer_type && value.as_ref() == current => {
            (*count_type, count.as_ref(), false)
        }
        _ => return None,
    };
    let Some((count, used_landing)) = nonnegative_count(count, count_type, landed) else {
        return Some(Err(IntegerAffineWitnessError::ShiftCountNotLanded(
            definition_index,
        )));
    };
    let Ok(count) = u32::try_from(count) else {
        return Some(Err(IntegerAffineWitnessError::ShiftCountOutsideValueWidth(
            definition_index,
        )));
    };
    if count >= u32::from(integer_type.bits()) {
        return Some(Err(IntegerAffineWitnessError::ShiftCountOutsideValueWidth(
            definition_index,
        )));
    }
    let step = if left {
        CheckedIntegerEndpointStep::ShiftLeft(count)
    } else {
        CheckedIntegerEndpointStep::ShiftRight(count)
    };
    Some(Ok((target.clone(), 1, 0, used_landing, step)))
}

fn nonnegative_count(
    term: &ScalarTerm,
    count_type: IntegerType,
    landed: Option<&LandedInteger>,
) -> Option<(u128, bool)> {
    let (actual_type, value, used_landing) =
        if let Some((actual_type, value)) = term.integer_value() {
            (actual_type, value, false)
        } else {
            let landed = landed.filter(|landed| landed.term == *term)?;
            (landed.integer_type, landed.value, true)
        };
    if actual_type != count_type || actual_type.carrier() != IntegerCarrier::Fixed {
        return None;
    }
    match value {
        IntegerValue::Signed(value) => u128::try_from(value)
            .ok()
            .map(|value| (value, used_landing)),
        IntegerValue::Unsigned(value) => Some((value, used_landing)),
    }
}

/// Traverse an `Equal(defined, <policy>_add(operand, literal))` row toward its
/// operand. Exactly one addend may be the chain operand: if both addends land
/// as literals the backward step is ambiguous and rejects rather than
/// guessing which value produced the sum.
#[allow(clippy::too_many_arguments)]
fn apply_add_inverse(
    defined: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&LandedInteger>,
    definition_index: usize,
    matcher: TermMatcher,
    step: fn(&ScalarTerm, i128) -> CheckedIntegerEndpointStep,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    if !matches!(defined, ScalarTerm::Value { .. }) || defined != current {
        return None;
    }
    let Some((scalar_type, left, right)) = matcher(expression) else {
        return None;
    };
    if *scalar_type != integer_type {
        return None;
    }
    let left_operand = if matches!(left, ScalarTerm::Value { .. }) {
        signed_literal(right, integer_type, landed)
    } else {
        None
    };
    let right_operand = if matches!(right, ScalarTerm::Value { .. }) {
        signed_literal(left, integer_type, landed)
    } else {
        None
    };
    let (operand, literal, used_landing) = match (left_operand, right_operand) {
        (Some((literal, used)), None) => (left, literal, used),
        (None, Some((literal, used))) => (right, literal, used),
        (None, None) => return None,
        (Some(_), Some(_)) => {
            return Some(Err(IntegerAffineWitnessError::AmbiguousDefinition(
                definition_index,
            )));
        }
    };
    // `operand = defined - literal` keeps the coefficient; the mapped offset
    // moves opposite the forward step.
    Some(match offset.checked_sub(literal) {
        Some(offset) => Ok((
            operand.clone(),
            coefficient,
            offset,
            used_landing,
            step(operand, literal),
        )),
        None => Err(IntegerAffineWitnessError::CoefficientOverflow),
    })
}

/// Traverse an `Equal(defined, wrapping_add(operand, literal))` row toward its
/// operand. Only unsigned fixed carriers qualify, and exactly one addend may
/// be the chain operand: if both addends land as literals the backward step is
/// ambiguous and rejects rather than guessing which value produced the sum.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_wrapping_add_inverse(
    defined: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&LandedInteger>,
    definition_index: usize,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    apply_add_inverse(
        defined,
        expression,
        current,
        integer_type,
        coefficient,
        offset,
        landed,
        definition_index,
        wrapping_integer_add,
        |operand, literal| CheckedIntegerEndpointStep::WrappingAddBackward {
            operand: operand.clone(),
            literal,
        },
    )
}

/// Traverse an `Equal(defined, exact_add(operand, literal))` row toward its
/// operand. The exact add already carries a checked no-overflow obligation,
/// so `operand = defined - literal` is a true integer equation in either
/// sign: the plain `Subtract` step maps the bound without wrap evidence.
/// Exactly one addend may be the chain operand — two literal addends make the
/// backward step ambiguous and reject rather than guess which produced it.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_exact_add_inverse(
    defined: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&LandedInteger>,
    definition_index: usize,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    apply_add_inverse(
        defined,
        expression,
        current,
        integer_type,
        coefficient,
        offset,
        landed,
        definition_index,
        exact_integer_add,
        |_, literal| CheckedIntegerEndpointStep::Subtract(literal),
    )
}
