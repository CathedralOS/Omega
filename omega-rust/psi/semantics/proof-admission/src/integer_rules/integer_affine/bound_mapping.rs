//! Mapping bounds through a checked affine form: direct and correlated add,
//! subtract, multiply and shift, with the wrapping evidence each step needs.

use crate::integer_rules::integer_affine::CheckedIntegerAffineForm;
use crate::integer_rules::integer_affine::truth_bounds::integer_literal_as_i128;
use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};

/// Check the monotone or antitone image of one canonical root bound.
///
/// The caller must supply a root-bound proposition whose proof or citation was
/// checked independently. This function accepts no proof authority; it checks
/// only that the already-normalized ordered endpoint transform maps that exact
/// bound to the claimed target relation.
pub fn check_integer_affine_bound_conversion(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
    conclusion: &Proposition,
) -> Result<(), IntegerAffineBoundConversionError> {
    let expected = map_integer_affine_bound(form, root_bound)?;
    if conclusion != &expected {
        return Err(IntegerAffineBoundConversionError::ConclusionMismatch);
    }
    Ok(())
}

/// Compute the unique endpoint relation established by one checked ordered
/// transform and one independently proved root bound.
pub fn map_integer_affine_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    if matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedAddLower]
            | [CheckedIntegerEndpointStep::CorrelatedAddUpper]
    ) {
        return map_correlated_add_bound(form, root_bound);
    }
    if matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedSubtractLower]
            | [CheckedIntegerEndpointStep::CorrelatedSubtractUpper]
            | [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
    ) {
        return map_correlated_subtract_bound(form, root_bound);
    }
    if matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum]
            | [CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum]
    ) {
        return map_correlated_multiply_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty() && matches!(form.target(), ScalarTerm::ExactIntegerAdd { .. })
    {
        return map_direct_add_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty()
        && matches!(form.target(), ScalarTerm::ExactIntegerSubtract { .. })
    {
        return map_direct_subtract_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty()
        && matches!(form.target(), ScalarTerm::ExactIntegerMultiply { .. })
    {
        return map_direct_multiply_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty()
        && let ScalarTerm::ExactIntegerShiftLeft {
            count_type, count, ..
        } = form.target()
    {
        return map_direct_shift_left_bound(form, *count_type, count, root_bound);
    }
    let (strict, bound, root_is_lower_endpoint, evidence_members) =
        destructure_chain_root_bound(form, root_bound)?;
    let Some(bound) = integer_literal_as_i128(bound, form.integer_type()) else {
        return Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral);
    };
    let mut mapped = bound;
    let mut reverses_order = false;
    let mut required_evidence = Vec::new();
    for step in &form.endpoint_steps {
        let current_is_lower = root_is_lower_endpoint ^ reverses_order;
        if strict && !step.preserves_strict_endpoint() {
            return Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation);
        }
        mapped = match step {
            CheckedIntegerEndpointStep::Add(value) => mapped.checked_add(*value),
            CheckedIntegerEndpointStep::Subtract(value) => mapped.checked_sub(*value),
            CheckedIntegerEndpointStep::Multiply(value) => {
                if *value < 0 {
                    reverses_order = !reverses_order;
                }
                mapped.checked_mul(*value)
            }
            CheckedIntegerEndpointStep::Divide(value) => {
                if *value < 0 {
                    reverses_order = !reverses_order;
                }
                mapped.checked_div(*value)
            }
            // Truncating remainder keeps the dividend's sign: a dividend
            // already bounded on one side pins the remainder to that side, so
            // `0 <= x` maps to `0 <= x % d` and `x <= 0` maps to `x % d <= 0`.
            // The opposite side keeps the unconditional `|d| - 1` image.
            CheckedIntegerEndpointStep::Remainder(value) => {
                let magnitude = value
                    .checked_abs()
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                Some(
                    if current_is_lower && form.integer_type().sign() == IntegerSign::Signed {
                        if mapped >= 0 {
                            0
                        } else {
                            1_i128
                                .checked_sub(magnitude)
                                .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                        }
                    } else if current_is_lower
                        || (form.integer_type().sign() == IntegerSign::Signed && mapped <= 0)
                    {
                        0
                    } else {
                        magnitude
                            .checked_sub(1)
                            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    },
                )
            }
            CheckedIntegerEndpointStep::ShiftLeft(count) => mapped.checked_mul(
                1_i128
                    .checked_shl(*count)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?,
            ),
            CheckedIntegerEndpointStep::ShiftRight(count) => Some(mapped >> count),
            // A bitwise-and step discards the incoming endpoint: with a
            // checked non-negative mask the result sets only mask bits, so
            // its image is `[0, mask]` whatever bound the operand carried.
            // Neither endpoint inherits the root's literal.
            CheckedIntegerEndpointStep::BitwiseAndMask(mask) => {
                Some(if current_is_lower { 0 } else { *mask })
            }
            // `v = (x + c) mod 2^w` never exceeds `x + c`, so an upper bound on
            // `x` maps unconditionally. A lower bound survives only when the
            // same definition proves `x <= maximum - c`, so the sum cannot
            // reduce modulo the width.
            CheckedIntegerEndpointStep::WrappingAdd { operand, literal } => {
                if current_is_lower && *literal != 0 {
                    push_wrapping_evidence(
                        &mut required_evidence,
                        form.integer_type(),
                        operand,
                        *literal,
                    )?;
                }
                mapped.checked_add(*literal)
            }
            // `x = v - c (mod 2^w)` is never below `v - c`, so a lower bound
            // on `v` maps unconditionally. An upper bound `v < k` yields
            // `x <= k - c - 1` only when `x + c` cannot wrap.
            CheckedIntegerEndpointStep::WrappingAddBackward { operand, literal } => {
                if !current_is_lower && *literal != 0 {
                    push_wrapping_evidence(
                        &mut required_evidence,
                        form.integer_type(),
                        operand,
                        *literal,
                    )?;
                }
                mapped.checked_sub(*literal)
            }
            CheckedIntegerEndpointStep::CorrelatedAddLower
            | CheckedIntegerEndpointStep::CorrelatedAddUpper
            | CheckedIntegerEndpointStep::CorrelatedSubtractLower
            | CheckedIntegerEndpointStep::CorrelatedSubtractUpper
            | CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum => {
                return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
            }
        }
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
    }
    for member in evidence_members {
        let Some(position) = required_evidence
            .iter()
            .position(|required| required == member)
        else {
            return Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected);
        };
        required_evidence.remove(position);
    }
    if !required_evidence.is_empty() {
        return Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing);
    }
    let mapped_value = match form.integer_type().sign() {
        IntegerSign::Signed => IntegerValue::Signed(mapped),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            u128::try_from(mapped)
                .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?,
        ),
    };
    let mapped = ScalarTerm::integer(form.integer_type(), mapped_value)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;

    // Positive forms preserve order, negative forms reverse it. A constant
    // form can soundly provide either orientation; retaining the root bound's
    // orientation makes that choice deterministic.
    let target_is_left = if reverses_order {
        root_is_lower_endpoint
    } else {
        !root_is_lower_endpoint
    };
    Ok(match (strict, target_is_left) {
        (false, true) => Proposition::LessOrEqual(form.target().clone(), mapped),
        (false, false) => Proposition::LessOrEqual(mapped, form.target().clone()),
        (true, true) => Proposition::LessThan(form.target().clone(), mapped),
        (true, false) => Proposition::LessThan(mapped, form.target().clone()),
    })
}

/// Decompose the admitted root bound of a chain form. A bare relation on the
/// root is the ordinary case; a `Conjunction` designates its first member as
/// the relation and treats every later member as a candidate evidence
/// conjunct, matched exactly against the no-wrap requirements the endpoint
/// steps impose. The designated-slot rule keeps evidence such as
/// `operand <= maximum - c` unambiguous even when the operand is itself the
/// root value.
fn destructure_chain_root_bound<'a>(
    form: &CheckedIntegerAffineForm,
    root_bound: &'a Proposition,
) -> Result<(bool, &'a ScalarTerm, bool, Vec<&'a Proposition>), IntegerAffineBoundConversionError> {
    let (relation, evidence_members) = match root_bound {
        Proposition::Conjunction(members) => {
            let Some((relation, evidence)) = members.split_first() else {
                return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual);
            };
            (relation, evidence.iter().collect::<Vec<_>>())
        }
        _ => (root_bound, Vec::new()),
    };
    let (strict, bound_left, bound_right) = match relation {
        Proposition::LessOrEqual(left, right) => (false, left, right),
        Proposition::LessThan(left, right) => (true, left, right),
        _ => return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual),
    };
    let (bound, root_is_lower_endpoint) = if bound_left == form.root() {
        (bound_right, false)
    } else if bound_right == form.root() {
        (bound_left, true)
    } else {
        return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
    };
    Ok((strict, bound, root_is_lower_endpoint, evidence_members))
}

/// Compute the exact no-wrap conjuncts a checked form needs from its root
/// bound for the supplied relation shape, in step order and deduplicated.
/// This is producer convenience: bound conversion recomputes the same set at
/// admission and rejects any member that is missing or unexpected.
pub fn integer_affine_wrapping_evidence(
    form: &CheckedIntegerAffineForm,
    relation: &Proposition,
) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
    let (strict, bound, root_is_lower_endpoint, evidence_members) =
        destructure_chain_root_bound(form, relation)?;
    if !evidence_members.is_empty() {
        return Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected);
    }
    let _ = integer_literal_as_i128(bound, form.integer_type())
        .ok_or(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral)?;
    let mut reverses_order = false;
    let mut required_evidence = Vec::new();
    for step in &form.endpoint_steps {
        let current_is_lower = root_is_lower_endpoint ^ reverses_order;
        if strict && !step.preserves_strict_endpoint() {
            return Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation);
        }
        match step {
            CheckedIntegerEndpointStep::Multiply(value)
            | CheckedIntegerEndpointStep::Divide(value)
                if *value < 0 =>
            {
                reverses_order = !reverses_order;
            }
            CheckedIntegerEndpointStep::WrappingAdd { operand, literal }
                if current_is_lower && *literal != 0 =>
            {
                push_wrapping_evidence(
                    &mut required_evidence,
                    form.integer_type(),
                    operand,
                    *literal,
                )?;
            }
            CheckedIntegerEndpointStep::WrappingAddBackward { operand, literal }
                if !current_is_lower && *literal != 0 =>
            {
                push_wrapping_evidence(
                    &mut required_evidence,
                    form.integer_type(),
                    operand,
                    *literal,
                )?;
            }
            _ => {}
        }
    }
    Ok(required_evidence)
}

/// The required evidence shape for a wrapping-add step on `operand` with
/// addend `literal`: the operand stays inside the headroom
/// `operand <= maximum - literal`, so `operand + literal` cannot reduce
/// modulo the carrier width. Deduplicated so repeated steps cite one conjunct.
fn push_wrapping_evidence(
    required_evidence: &mut Vec<Proposition>,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    literal: i128,
) -> Result<(), IntegerAffineBoundConversionError> {
    let IntegerValue::Unsigned(maximum) = integer_type.maximum_value() else {
        return Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier);
    };
    let headroom = maximum
        .checked_sub(u128::try_from(literal).unwrap_or(u128::MAX))
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    let bound = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(headroom))
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    let evidence = Proposition::LessOrEqual(operand.clone(), bound);
    if !required_evidence.contains(&evidence) {
        required_evidence.push(evidence);
    }
    Ok(())
}

fn map_correlated_add_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerAdd { left, right, .. } = form.target() else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let lower = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedAddLower]
    );
    let expected = if lower {
        Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
    } else {
        Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
    };
    if evidence != &expected {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    let sum = IntegerMathTerm::Add(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(form.integer_type().minimum_value()),
            sum,
        )
    } else {
        Proposition::IntegerMathLessOrEqual(
            sum,
            IntegerMathTerm::literal(form.integer_type().maximum_value()),
        )
    })
}

fn map_correlated_subtract_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerSubtract { left, right, .. } = form.target() else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    let lower = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedSubtractLower]
            | [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
    );
    let unsigned = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
    );
    let expected = if unsigned {
        Proposition::LessOrEqual(right.as_ref().clone(), left.as_ref().clone())
    } else if lower {
        Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
    } else {
        Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
    };
    if evidence != &expected {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    }
    let difference = IntegerMathTerm::Subtract(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(form.integer_type().minimum_value()),
            difference,
        )
    } else {
        Proposition::IntegerMathLessOrEqual(
            difference,
            IntegerMathTerm::literal(form.integer_type().maximum_value()),
        )
    })
}

fn map_correlated_multiply_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerMultiply { left, right, .. } = form.target() else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let [sign_evidence, bound_evidence] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let one = ScalarTerm::integer(
        form.integer_type(),
        match form.integer_type().sign() {
            IntegerSign::Signed => IntegerValue::Signed(1),
            IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        },
    )
    .map_err(|_| IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?;
    let positive = sign_evidence == &Proposition::LessOrEqual(one, right.as_ref().clone());
    let negative_two = ScalarTerm::integer(form.integer_type(), IntegerValue::Signed(-2)).ok();
    let negative = negative_two.is_some_and(|negative_two| {
        sign_evidence == &Proposition::LessOrEqual(right.as_ref().clone(), negative_two)
    });
    if !positive && !negative {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    }
    let endpoint_minimum = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum]
    );
    let lower = endpoint_minimum;
    let expected_bound = if lower == positive {
        Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
    } else {
        Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
    };
    if bound_evidence != &expected_bound {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    }
    let product = IntegerMathTerm::Multiply(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(form.integer_type().minimum_value()),
            product,
        )
    } else {
        Proposition::IntegerMathLessOrEqual(
            product,
            IntegerMathTerm::literal(form.integer_type().maximum_value()),
        )
    })
}

fn map_direct_add_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let [left_evidence, right_evidence] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    if *scalar_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    let left_endpoint = direct_operand_endpoint(left, left_evidence, form.integer_type())?;
    let right_endpoint = direct_operand_endpoint(right, right_evidence, form.integer_type())?;
    let lower = match (left_endpoint.orientation(), right_endpoint.orientation()) {
        (Some(left), Some(right)) if left == right => left,
        (None, Some(right)) => right,
        (Some(left), None) => left,
        (None, None) => return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
        _ => return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
    };
    let left_bound = left_endpoint.literal(form.integer_type(), lower);
    let right_bound = right_endpoint.literal(form.integer_type(), lower);
    let bound = add_math_literals(left_bound, right_bound)?;
    let sum = IntegerMathTerm::Add(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(bound), sum)
    } else {
        Proposition::IntegerMathLessOrEqual(sum, IntegerMathTerm::IntegerLiteral(bound))
    })
}

fn map_direct_subtract_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    let [left_evidence, right_evidence] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    if *scalar_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    }
    let left_endpoint = direct_operand_endpoint(left, left_evidence, form.integer_type())?;
    let right_endpoint = direct_operand_endpoint(right, right_evidence, form.integer_type())?;
    let lower = match (left_endpoint.orientation(), right_endpoint.orientation()) {
        (Some(left), Some(right)) if left != right => left,
        (None, Some(right)) => !right,
        (Some(left), None) => left,
        (None, None)
            if matches!(
                (&left_endpoint, &right_endpoint),
                (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Carrier)
                    if left.as_integer_value(form.integer_type())
                        == Some(form.integer_type().maximum_value())
            ) =>
        {
            true
        }
        (None, None)
            if matches!(
                (&left_endpoint, &right_endpoint),
                (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Carrier)
                    if left.as_integer_value(form.integer_type())
                        == Some(form.integer_type().minimum_value())
            ) =>
        {
            false
        }
        (None, None) => {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        }
        _ => return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch),
    };
    let left_bound = left_endpoint.literal(form.integer_type(), lower);
    let right_bound = right_endpoint.literal(form.integer_type(), !lower);
    let bound = subtract_math_literals(left_bound, right_bound)?;
    let difference = IntegerMathTerm::Subtract(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(bound), difference)
    } else {
        Proposition::IntegerMathLessOrEqual(difference, IntegerMathTerm::IntegerLiteral(bound))
    })
}

fn map_direct_multiply_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let [left_first, left_second, right_first, right_second] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let ScalarTerm::ExactIntegerMultiply {
        scalar_type,
        left,
        right,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    if *scalar_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    }
    let left_first = direct_operand_endpoint(left, left_first, form.integer_type())?;
    let left_second = direct_operand_endpoint(left, left_second, form.integer_type())?;
    let right_first = direct_operand_endpoint(right, right_first, form.integer_type())?;
    let right_second = direct_operand_endpoint(right, right_second, form.integer_type())?;
    let pair_orientation = |first: &DirectAddEndpoint,
                            second: &DirectAddEndpoint|
     -> Result<Option<bool>, IntegerAffineBoundConversionError> {
        match (first.orientation(), second.orientation()) {
            (Some(first), Some(second)) if first != second => Ok(Some(first)),
            (Some(first), None) if matches!(second, DirectAddEndpoint::Carrier) => Ok(Some(first)),
            (None, Some(second)) if matches!(first, DirectAddEndpoint::Carrier) => {
                Ok(Some(!second))
            }
            (None, None)
                if matches!(
                    (first, second),
                    (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Exact(right)) if left == right
                ) || matches!(
                    (first, second),
                    (DirectAddEndpoint::Carrier, DirectAddEndpoint::Carrier)
                ) =>
            {
                Ok(None)
            }
            _ => Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
        }
    };
    let left_orientation = pair_orientation(&left_first, &left_second)?;
    let right_orientation = pair_orientation(&right_first, &right_second)?;
    let lower = match (left_orientation, right_orientation) {
        (Some(left), Some(right)) if left == right => left,
        (Some(left), None) => left,
        (None, Some(right)) => right,
        _ => return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
    };
    let (left_lower, left_upper, right_lower, right_upper) = if lower {
        (
            left_first.literal(form.integer_type(), true),
            left_second.literal(form.integer_type(), false),
            right_first.literal(form.integer_type(), true),
            right_second.literal(form.integer_type(), false),
        )
    } else {
        (
            left_second.literal(form.integer_type(), true),
            left_first.literal(form.integer_type(), false),
            right_second.literal(form.integer_type(), true),
            right_first.literal(form.integer_type(), false),
        )
    };
    let products = [
        multiply_math_literals(left_lower, right_lower)?,
        multiply_math_literals(left_lower, right_upper)?,
        multiply_math_literals(left_upper, right_lower)?,
        multiply_math_literals(left_upper, right_upper)?,
    ];
    let bound = products
        .into_iter()
        .reduce(|current, candidate| {
            if (lower && math_literal_less(candidate, current))
                || (!lower && math_literal_less(current, candidate))
            {
                candidate
            } else {
                current
            }
        })
        .expect("four product corners");
    let product = IntegerMathTerm::Multiply(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(bound), product)
    } else {
        Proposition::IntegerMathLessOrEqual(product, IntegerMathTerm::IntegerLiteral(bound))
    })
}

enum DirectAddEndpoint {
    Exact(semantic_vocabulary::IntegerMathLiteral),
    Oriented {
        literal: semantic_vocabulary::IntegerMathLiteral,
        lower: bool,
    },
    Carrier,
}

impl DirectAddEndpoint {
    fn orientation(&self) -> Option<bool> {
        match self {
            Self::Oriented { lower, .. } => Some(*lower),
            Self::Exact(_) | Self::Carrier => None,
        }
    }

    fn literal(
        &self,
        integer_type: IntegerType,
        lower: bool,
    ) -> semantic_vocabulary::IntegerMathLiteral {
        match self {
            Self::Exact(literal) | Self::Oriented { literal, .. } => *literal,
            Self::Carrier => {
                semantic_vocabulary::IntegerMathLiteral::from_integer_value(if lower {
                    integer_type.minimum_value()
                } else {
                    integer_type.maximum_value()
                })
            }
        }
    }
}

fn direct_operand_endpoint(
    operand: &ScalarTerm,
    evidence: &Proposition,
    integer_type: IntegerType,
) -> Result<DirectAddEndpoint, IntegerAffineBoundConversionError> {
    if let ScalarTerm::Integer { scalar_type, value } = operand {
        if *scalar_type != integer_type || evidence != &Proposition::Truth {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        return Ok(DirectAddEndpoint::Exact(
            semantic_vocabulary::IntegerMathLiteral::from_integer_value(*value),
        ));
    }
    if evidence == &Proposition::Truth {
        return Ok(DirectAddEndpoint::Carrier);
    }
    if let Proposition::Equal(left, right) = evidence {
        let literal = if left == operand {
            right
        } else if right == operand {
            left
        } else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        let (actual, value) = literal
            .integer_value()
            .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?;
        if actual != integer_type {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        return Ok(DirectAddEndpoint::Exact(
            semantic_vocabulary::IntegerMathLiteral::from_integer_value(value),
        ));
    }
    let Proposition::LessOrEqual(left, right) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    if left == right {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    let (bound, lower) = if right == operand {
        (left, true)
    } else if left == operand {
        (right, false)
    } else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let (actual, value) = bound
        .integer_value()
        .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?;
    if actual != integer_type {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    Ok(DirectAddEndpoint::Oriented {
        literal: semantic_vocabulary::IntegerMathLiteral::from_integer_value(value),
        lower,
    })
}

fn add_math_literals(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let (negative, magnitude) = if left.negative() == right.negative() {
        (
            left.negative(),
            left.magnitude()
                .checked_add(right.magnitude())
                .ok_or(IntegerAffineBoundConversionError::DirectAddBoundOverflow)?,
        )
    } else {
        match left.magnitude().cmp(&right.magnitude()) {
            std::cmp::Ordering::Less => (right.negative(), right.magnitude() - left.magnitude()),
            std::cmp::Ordering::Equal => (false, 0),
            std::cmp::Ordering::Greater => (left.negative(), left.magnitude() - right.magnitude()),
        }
    };
    semantic_vocabulary::IntegerMathLiteral::new(negative, magnitude)
        .map_err(|_| IntegerAffineBoundConversionError::DirectAddBoundOverflow)
}

fn subtract_math_literals(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let (negative, magnitude) = if left.negative() != right.negative() {
        (
            left.negative(),
            left.magnitude()
                .checked_add(right.magnitude())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractBoundOverflow)?,
        )
    } else {
        match left.magnitude().cmp(&right.magnitude()) {
            std::cmp::Ordering::Less => (!left.negative(), right.magnitude() - left.magnitude()),
            std::cmp::Ordering::Equal => (false, 0),
            std::cmp::Ordering::Greater => (left.negative(), left.magnitude() - right.magnitude()),
        }
    };
    semantic_vocabulary::IntegerMathLiteral::new(negative, magnitude)
        .map_err(|_| IntegerAffineBoundConversionError::DirectSubtractBoundOverflow)
}

fn multiply_math_literals(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let magnitude = left
        .magnitude()
        .checked_mul(right.magnitude())
        .ok_or(IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)?;
    semantic_vocabulary::IntegerMathLiteral::new(
        magnitude != 0 && left.negative() != right.negative(),
        magnitude,
    )
    .map_err(|_| IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)
}

fn math_literal_less(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> bool {
    match (left.negative(), right.negative()) {
        (true, false) => true,
        (false, true) => false,
        (true, true) => left.magnitude() > right.magnitude(),
        (false, false) => left.magnitude() < right.magnitude(),
    }
}

pub(crate) fn direct_math_leaf(
    term: &ScalarTerm,
    expected: IntegerType,
) -> Option<IntegerMathTerm> {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(actual),
        } if *actual == expected => Some(IntegerMathTerm::MathValue {
            source_type: expected,
            value: *id,
        }),
        ScalarTerm::Integer { scalar_type, value } if *scalar_type == expected => {
            Some(IntegerMathTerm::literal(*value))
        }
        _ => None,
    }
}

fn map_direct_shift_left_bound(
    form: &CheckedIntegerAffineForm,
    count_type: IntegerType,
    count: &ScalarTerm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let parts = match evidence {
        Proposition::Conjunction(parts) => parts.as_slice(),
        proposition => std::slice::from_ref(proposition),
    };
    let mut root_bound = None;
    let mut exact_count = None;
    let mut count_lower = count_type.sign() == IntegerSign::Unsigned;
    let mut count_upper = None;
    for part in parts {
        match part {
            Proposition::LessOrEqual(left, right)
                if left == form.root() || right == form.root() =>
            {
                if root_bound.replace(part).is_some() {
                    return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
                }
            }
            Proposition::Equal(left, right) if left == count || right == count => {
                let literal = if left == count { right } else { left };
                let (actual, value) = literal
                    .integer_value()
                    .ok_or(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch)?;
                if actual != count_type || exact_count.replace(value).is_some() {
                    return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
                }
            }
            Proposition::LessOrEqual(left, right) if right == count => {
                let (actual, value) = left
                    .integer_value()
                    .ok_or(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch)?;
                if actual == count_type && integer_value_to_u128(value) == Some(0) {
                    count_lower = true;
                }
            }
            Proposition::LessOrEqual(left, right) if left == count => {
                let (actual, value) = right
                    .integer_value()
                    .ok_or(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch)?;
                if actual != count_type || count_upper.replace(value).is_some() {
                    return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
                }
            }
            _ => {
                return Err(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch);
            }
        }
    }
    let root_bound =
        root_bound.ok_or(IntegerAffineBoundConversionError::DirectShiftRootBoundMissing)?;
    let (minimum_count, maximum_count) = if let Some((actual, embedded)) = count.integer_value() {
        if actual != count_type || exact_count.is_some() || count_upper.is_some() {
            return Err(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch);
        }
        (embedded, embedded)
    } else if let Some(exact) = exact_count {
        if count_upper.is_some() {
            return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
        }
        (exact, exact)
    } else if let Some(upper) = count_upper {
        if !count_lower {
            return Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing);
        }
        let zero = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        };
        (zero, upper)
    } else {
        if !count_lower {
            return Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing);
        }
        let zero = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        };
        (zero, count_type.maximum_value())
    };
    let minimum_count = integer_value_to_u128(minimum_count)
        .and_then(|count| u32::try_from(count).ok())
        .filter(|count| *count < u32::from(form.integer_type().bits()))
        .ok_or(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth)?;
    let maximum_count = integer_value_to_u128(maximum_count)
        .and_then(|count| u32::try_from(count).ok())
        .filter(|count| *count < u32::from(form.integer_type().bits()))
        .ok_or(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth)?;
    if minimum_count > maximum_count {
        return Err(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch);
    }

    let Proposition::LessOrEqual(bound_left, bound_right) = root_bound else {
        return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual);
    };
    let (bound, root_is_lower_endpoint) = if bound_left == form.root() {
        (bound_right, false)
    } else if bound_right == form.root() {
        (bound_left, true)
    } else {
        return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
    };
    let (actual_type, bound) = bound
        .integer_value()
        .ok_or(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral)?;
    if actual_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral);
    }
    let negative = matches!(bound, IntegerValue::Signed(value) if value < 0);
    let count = match (root_is_lower_endpoint, negative) {
        (true, true) | (false, false) => maximum_count,
        (true, false) | (false, true) => minimum_count,
    };
    let shifted_bound = shift_math_literal(bound, count)?;
    let shifted = direct_shift_math_term(form)?;
    Ok(if root_is_lower_endpoint {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(shifted_bound), shifted)
    } else {
        Proposition::IntegerMathLessOrEqual(shifted, IntegerMathTerm::IntegerLiteral(shifted_bound))
    })
}

fn integer_value_to_u128(value: IntegerValue) -> Option<u128> {
    match value {
        IntegerValue::Signed(value) => u128::try_from(value).ok(),
        IntegerValue::Unsigned(value) => Some(value),
    }
}

fn shift_math_literal(
    value: IntegerValue,
    count: u32,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let literal = semantic_vocabulary::IntegerMathLiteral::from_integer_value(value);
    let magnitude = literal
        .magnitude()
        .checked_shl(count)
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
    semantic_vocabulary::IntegerMathLiteral::new(literal.negative(), magnitude)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOverflow)
}

fn direct_shift_math_term(
    form: &CheckedIntegerAffineForm,
) -> Result<IntegerMathTerm, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerShiftLeft {
        value_type,
        count_type,
        value,
        count,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch);
    };
    let lift = |term: &ScalarTerm, expected: IntegerType| match term {
        ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(actual),
        } if *actual == expected => Some(IntegerMathTerm::MathValue {
            source_type: expected,
            value: *id,
        }),
        ScalarTerm::Integer { scalar_type, value } if *scalar_type == expected => {
            Some(IntegerMathTerm::literal(*value))
        }
        _ => None,
    };
    Ok(IntegerMathTerm::ShiftLeft {
        value: Box::new(
            lift(value, *value_type)
                .ok_or(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch)?,
        ),
        count: Box::new(
            lift(count, *count_type)
                .ok_or(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch)?,
        ),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerAffineBoundConversionError {
    RootBoundNotLessOrEqual,
    RootBoundMismatch,
    RootBoundNotTypedLiteral,
    MappedBoundOverflow,
    MappedBoundOutsideCarrier,
    TruthRootWithoutTotalImage,
    NonTotalDivisionImage,
    DirectShiftEvidenceMismatch,
    DirectShiftRootBoundMissing,
    DirectShiftCountEvidenceMismatch,
    DirectShiftCountLowerMissing,
    DirectShiftCountOutsideValueWidth,
    AmbiguousDirectShiftEvidence,
    DirectAddEvidenceMismatch,
    DirectAddBoundOverflow,
    DirectSubtractEvidenceMismatch,
    DirectSubtractBoundOverflow,
    DirectMultiplyEvidenceMismatch,
    DirectMultiplyBoundOverflow,
    StrictBoundNotTranslation,
    WrappingEvidenceMissing,
    WrappingEvidenceUnexpected,
    ConclusionMismatch,
}

impl std::fmt::Display for IntegerAffineBoundConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IntegerAffineBoundConversionError {}
