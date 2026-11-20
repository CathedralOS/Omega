use semantic_vocabulary::{
    ClosedIntegerType, IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign,
    IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};

use crate::{OperationSemanticError, ScalarLeafGoalShape};

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalScalarGoal {
    ExactCastRepresentable {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    },
    ExactShiftCount {
        value_type: IntegerType,
        count_type: IntegerType,
        count: ScalarTerm,
    },
    ExactShiftLeftRepresentable {
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    },
    ExactArithmeticRepresentable {
        integer_type: IntegerType,
        expression: ScalarTerm,
    },
    ExactDivisionDefined {
        integer_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    },
    NonzeroDivisor {
        integer_type: IntegerType,
        divisor: ScalarTerm,
    },
}

impl CanonicalScalarGoal {
    /// A computed term owes the same safety condition wherever it is
    /// used. Inspect this node after its operands have been formed;
    /// callers must also discharge every nested term's condition.
    /// This describes a question, without assuming or proving it.
    pub fn for_term(term: &ScalarTerm) -> Result<Option<Self>, OperationSemanticError> {
        term.validate_after_operands()
            .map_err(OperationSemanticError::InvalidProposition)?;
        Ok(Some(match term {
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => Self::ExactCastRepresentable {
                source_type: *source_type,
                target_type: *target_type,
                operand: operand.as_ref().clone(),
            },
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                count,
                ..
            } => Self::ExactShiftCount {
                value_type: *value_type,
                count_type: *count_type,
                count: count.as_ref().clone(),
            },
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => Self::ExactShiftLeftRepresentable {
                value_type: *value_type,
                count_type: *count_type,
                value: value.as_ref().clone(),
                count: count.as_ref().clone(),
            },
            ScalarTerm::ExactIntegerAdd { scalar_type, .. }
            | ScalarTerm::ExactIntegerSubtract { scalar_type, .. }
            | ScalarTerm::ExactIntegerMultiply { scalar_type, .. } => {
                Self::ExactArithmeticRepresentable {
                    integer_type: *scalar_type,
                    expression: term.clone(),
                }
            }
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => Self::ExactDivisionDefined {
                integer_type: *scalar_type,
                left: left.as_ref().clone(),
                right: right.as_ref().clone(),
            },
            ScalarTerm::WrappingIntegerDivide {
                scalar_type, right, ..
            }
            | ScalarTerm::WrappingIntegerRemainder {
                scalar_type, right, ..
            }
            | ScalarTerm::SaturatingIntegerDivide {
                scalar_type, right, ..
            }
            | ScalarTerm::SaturatingIntegerRemainder {
                scalar_type, right, ..
            } => Self::NonzeroDivisor {
                integer_type: *scalar_type,
                divisor: right.as_ref().clone(),
            },
            _ => return Ok(None),
        }))
    }

    pub const fn shape(&self) -> ScalarLeafGoalShape {
        match self {
            Self::ExactCastRepresentable { .. } => {
                ScalarLeafGoalShape::ExactCastRepresentable
            }
            Self::ExactShiftCount { .. } => ScalarLeafGoalShape::ExactShiftCount,
            Self::ExactShiftLeftRepresentable { .. } => {
                ScalarLeafGoalShape::ExactShiftLeftRepresentable
            }
            Self::ExactArithmeticRepresentable { .. } => {
                ScalarLeafGoalShape::ExactArithmeticRepresentable
            }
            Self::ExactDivisionDefined { .. } => ScalarLeafGoalShape::ExactDivisionDefined,
            Self::NonzeroDivisor { .. } => ScalarLeafGoalShape::NonzeroDivisor,
        }
    }

    pub fn kernel_proposition(&self) -> Result<Proposition, OperationSemanticError> {
        let proposition = match self {
            Self::ExactCastRepresentable {
                source_type,
                target_type,
                operand,
            } => exact_cast_representability_proposition(
                *source_type,
                *target_type,
                operand,
            )?,
            Self::NonzeroDivisor {
                integer_type,
                divisor,
            } => nonzero_divisor_proposition(*integer_type, divisor)?,
            Self::ExactDivisionDefined {
                integer_type,
                left,
                right,
            } => exact_division_defined_proposition(*integer_type, left, right)?,
            Self::ExactShiftCount {
                value_type,
                count_type,
                count,
            } => exact_shift_count_proposition(*value_type, *count_type, count)?,
            Self::ExactShiftLeftRepresentable {
                value_type,
                count_type,
                value,
                count,
            } => exact_shift_left_representability_proposition(
                *value_type,
                *count_type,
                value,
                count,
            )?,
            Self::ExactArithmeticRepresentable {
                integer_type,
                expression,
            } => match expression {
                ScalarTerm::ExactIntegerAdd { .. } => {
                    exact_add_representability_proposition(*integer_type, expression)?
                }
                ScalarTerm::ExactIntegerSubtract { .. } => {
                    exact_subtract_representability_proposition(*integer_type, expression)?
                }
                ScalarTerm::ExactIntegerMultiply { .. } => {
                    exact_multiply_representability_proposition(*integer_type, expression)?
                }
                _ => {
                    return Err(
                        OperationSemanticError::ExactArithmeticExpressionShapeMismatch,
                    );
                }
            },
        };
        proposition
            .validate()
            .map_err(OperationSemanticError::InvalidProposition)?;
        Ok(proposition)
    }
}

// Concrete endpoints and shifts require a known carrier; a rigid width
// contributes no guessed range even when its binder is positive.
fn concrete_integer(
    integer: IntegerType,
) -> Result<ClosedIntegerType, OperationSemanticError> {
    integer
        .try_into()
        .map_err(|_| OperationSemanticError::UnresolvedIntegerWidth(integer))
}

fn exact_multiply_representability_proposition(
    integer_type: IntegerType,
    expression: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let known_integer_type = concrete_integer(integer_type)?;
    if integer_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactArithmeticRequiresFixedInteger(
            integer_type,
        ));
    }
    let ScalarTerm::ExactIntegerMultiply {
        scalar_type,
        left,
        right,
    } = expression
    else {
        unreachable!("exact-multiply projection is selected by expression shape")
    };
    let [(_, mathematical_left), (_, mathematical_right)] = exact_integer_operands(
        integer_type,
        scalar_type,
        left,
        right,
        |result, left, right| result == left && result == right,
    )?;
    if let Some(proposition) =
        literal_pair_representability(left.as_ref(), right.as_ref(), |left, right| {
            integer_type.exact_mul(left, right)
        })
    {
        return Ok(proposition);
    }
    if matches!(mathematical_left, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
        || matches!(mathematical_right, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
    {
        return Ok(Proposition::Truth);
    }
    let one = integer_type.one();
    if left.integer_value() == Some((integer_type, one))
        || right.integer_value() == Some((integer_type, one))
    {
        return Ok(Proposition::Truth);
    }

    let product = IntegerMathTerm::Multiply(
        Box::new(mathematical_left),
        Box::new(mathematical_right),
    );
    let embedded_negative_one = integer_type.sign() == IntegerSign::Signed
        && (left.integer_value() == Some((integer_type, IntegerValue::Signed(-1)))
            || right.integer_value() == Some((integer_type, IntegerValue::Signed(-1))));
    let mut bounds = Vec::with_capacity(2);
    if integer_type.sign() == IntegerSign::Signed && !embedded_negative_one {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(known_integer_type.minimum_value()),
            product.clone(),
        ));
    }
    bounds.push(Proposition::IntegerMathLessOrEqual(
        product,
        IntegerMathTerm::literal(known_integer_type.maximum_value()),
    ));
    Ok(canonical_conjunction(bounds))
}

// A parameter width's endpoints are terms over its binder
// (`IntegerMathTerm::carrier_minimum`), so exact add and subtract
// owe the same range at every width the body admits.
fn exact_subtract_representability_proposition(
    integer_type: IntegerType,
    expression: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = expression
    else {
        unreachable!("exact-subtract projection is selected by expression shape")
    };
    let [
        (left_type, mathematical_left),
        (right_type, mathematical_right),
    ] = exact_integer_operands(
        integer_type,
        scalar_type,
        left,
        right,
        IntegerType::admits_exact_subtract_operands,
    )?;
    if let Some(proposition) =
        literal_pair_representability(left.as_ref(), right.as_ref(), |left, right| {
            integer_type.exact_subtract_values(left_type, left, right_type, right)
        })
    {
        return Ok(proposition);
    }
    if (left_type == integer_type
        && matches!(mathematical_right, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0))
        || left == right
    {
        return Ok(Proposition::Truth);
    }

    let difference = IntegerMathTerm::Subtract(
        Box::new(mathematical_left),
        Box::new(mathematical_right),
    );
    let mut bounds = Vec::with_capacity(2);
    bounds.push(Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::carrier_minimum(integer_type),
        difference.clone(),
    ));
    if integer_type.sign() == IntegerSign::Signed || left_type != integer_type {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            difference,
            IntegerMathTerm::carrier_maximum(integer_type),
        ));
    }
    Ok(canonical_conjunction(bounds))
}

fn exact_add_representability_proposition(
    integer_type: IntegerType,
    expression: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = expression
    else {
        unreachable!("exact-add projection is selected by expression shape")
    };
    let [
        (left_type, mathematical_left),
        (right_type, mathematical_right),
    ] = exact_integer_operands(
        integer_type,
        scalar_type,
        left,
        right,
        IntegerType::admits_exact_add_operands,
    )?;
    if let Some(proposition) =
        literal_pair_representability(left.as_ref(), right.as_ref(), |left, right| {
            integer_type.exact_add_values(left_type, left, right_type, right)
        })
    {
        return Ok(proposition);
    }
    if (right_type == integer_type
        && matches!(mathematical_left, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0))
        || (left_type == integer_type
            && matches!(mathematical_right, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0))
    {
        return Ok(Proposition::Truth);
    }

    let sum =
        IntegerMathTerm::Add(Box::new(mathematical_left), Box::new(mathematical_right));
    let mut bounds = Vec::with_capacity(2);
    if integer_type.sign() == IntegerSign::Signed {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::carrier_minimum(integer_type),
            sum.clone(),
        ));
    }
    bounds.push(Proposition::IntegerMathLessOrEqual(
        sum,
        IntegerMathTerm::carrier_maximum(integer_type),
    ));
    Ok(canonical_conjunction(bounds))
}

fn exact_integer_operands(
    integer_type: IntegerType,
    scalar_type: &IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    admits: fn(IntegerType, IntegerType, IntegerType) -> bool,
) -> Result<[(IntegerType, IntegerMathTerm); 2], OperationSemanticError> {
    if *scalar_type != integer_type {
        return Err(
            OperationSemanticError::ExactArithmeticExpressionTypeMismatch {
                declared: integer_type,
                actual: ScalarType::Integer(*scalar_type),
            },
        );
    }
    let mismatch = || OperationSemanticError::ExactArithmeticOperandTypeMismatch {
        declared: integer_type,
        left: left.scalar_type(),
        right: right.scalar_type(),
    };
    let (ScalarType::Integer(left_type), ScalarType::Integer(right_type)) =
        (left.scalar_type(), right.scalar_type())
    else {
        return Err(mismatch());
    };
    if !admits(integer_type, left_type, right_type) {
        return Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch {
            declared: integer_type,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    let mathematical_left = integer_math_operand(left_type, left, |_| mismatch())?;
    let mathematical_right = integer_math_operand(right_type, right, |_| mismatch())?;
    Ok([
        (left_type, mathematical_left),
        (right_type, mathematical_right),
    ])
}

fn integer_math_operand(
    declared: IntegerType,
    term: &ScalarTerm,
    mismatch: impl FnOnce(ScalarType) -> OperationSemanticError,
) -> Result<IntegerMathTerm, OperationSemanticError> {
    if term.scalar_type() != ScalarType::Integer(declared) {
        return Err(mismatch(term.scalar_type()));
    }
    // A computed operand keeps its scalar policy in the existing
    // embedding. Validate it before neutral/zero shortcuts can erase
    // it from the final question; formation of every nested partial
    // operation is still a separate obligation of the caller.
    term.validate()
        .map_err(OperationSemanticError::InvalidProposition)?;
    IntegerMathTerm::embed(term.clone()).ok_or_else(|| mismatch(term.scalar_type()))
}

fn append_conjunct(proposition: Proposition, conjuncts: &mut Vec<Proposition>) {
    match proposition {
        Proposition::Truth => {}
        Proposition::Conjunction(parts) => conjuncts.extend(parts),
        proposition => conjuncts.push(proposition),
    }
}

fn canonical_conjunction(mut conjuncts: Vec<Proposition>) -> Proposition {
    match conjuncts.len() {
        0 => Proposition::Truth,
        1 => conjuncts.pop().expect("one canonical conjunct"),
        _ => Proposition::Conjunction(conjuncts),
    }
}

fn representability(representable: Option<IntegerValue>) -> Proposition {
    if representable.is_some() {
        Proposition::Truth
    } else {
        Proposition::Falsehood
    }
}

fn literal_pair_representability(
    left: &ScalarTerm,
    right: &ScalarTerm,
    exact: impl FnOnce(IntegerValue, IntegerValue) -> Option<IntegerValue>,
) -> Option<Proposition> {
    let (
        ScalarTerm::Integer {
            value: left_value, ..
        },
        ScalarTerm::Integer {
            value: right_value, ..
        },
    ) = (left, right)
    else {
        return None;
    };
    Some(representability(exact(*left_value, *right_value)))
}

fn exact_shift_left_representability_proposition(
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let known_value_type = concrete_integer(value_type)?;
    let count_proposition = exact_shift_count_proposition(value_type, count_type, count)?;
    let mathematical_value = integer_math_operand(value_type, value, |actual| {
        OperationSemanticError::ExactShiftLeftValueTypeMismatch {
            declared: value_type,
            actual,
        }
    })?;
    let mathematical_count = integer_math_operand(count_type, count, |actual| {
        OperationSemanticError::ExactShiftCountTypeMismatch {
            declared: count_type,
            actual,
        }
    })?;
    if count_proposition == Proposition::Falsehood {
        return Ok(Proposition::Falsehood);
    }
    if let Some(proposition) =
        literal_pair_representability(value, count, |value, count| {
            value_type.exact_shift_left(value, count_type, count)
        })
    {
        return Ok(proposition);
    }

    let mut conjuncts = Vec::with_capacity(4);
    append_conjunct(count_proposition, &mut conjuncts);
    if matches!(mathematical_count, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
    {
        return Ok(canonical_conjunction(conjuncts));
    }
    if matches!(mathematical_value, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
    {
        return Ok(canonical_conjunction(conjuncts));
    }
    let shifted = IntegerMathTerm::ShiftLeft {
        value: Box::new(mathematical_value),
        count: Box::new(mathematical_count),
    };
    if value_type.sign() == IntegerSign::Signed {
        conjuncts.push(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(known_value_type.minimum_value()),
            shifted.clone(),
        ));
    }
    conjuncts.push(Proposition::IntegerMathLessOrEqual(
        shifted,
        IntegerMathTerm::literal(known_value_type.maximum_value()),
    ));
    Ok(canonical_conjunction(conjuncts))
}

fn exact_cast_representability_proposition(
    source_type: IntegerType,
    target_type: IntegerType,
    operand: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let known_source_type = concrete_integer(source_type)?;
    if target_type.bits().is_none() {
        return exact_cast_to_parameter_width(source_type, target_type, operand);
    }
    let known_target_type = concrete_integer(target_type)?;
    if !source_type.can_exact_cast_to(target_type) {
        return Err(OperationSemanticError::ExactCastUnsupportedCarriers {
            source: source_type,
            target: target_type,
        });
    }
    if operand.scalar_type() != ScalarType::Integer(source_type) {
        return Err(OperationSemanticError::ExactCastOperandTypeMismatch {
            declared: source_type,
            actual: operand.scalar_type(),
        });
    }
    if let ScalarTerm::Integer { value, .. } = operand {
        return Ok(representability(
            source_type.exact_cast_value_to(target_type, *value),
        ));
    }
    let ScalarTerm::Value { id, .. } = operand else {
        return Err(OperationSemanticError::ExactCastRequiresValueOrLiteralOperand);
    };
    let value = IntegerMathTerm::MathValue {
        source_type,
        value: *id,
    };
    let source_minimum =
        IntegerMathLiteral::from_integer_value(known_source_type.minimum_value());
    let source_maximum =
        IntegerMathLiteral::from_integer_value(known_source_type.maximum_value());
    let target_minimum =
        IntegerMathLiteral::from_integer_value(known_target_type.minimum_value());
    let target_maximum =
        IntegerMathLiteral::from_integer_value(known_target_type.maximum_value());
    let mut bounds = Vec::with_capacity(2);
    if compare_math_literals(target_minimum, source_minimum).is_gt() {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::IntegerLiteral(target_minimum),
            value.clone(),
        ));
    }
    if compare_math_literals(target_maximum, source_maximum).is_lt() {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            value,
            IntegerMathTerm::IntegerLiteral(target_maximum),
        ));
    }
    Ok(match bounds.len() {
        0 => Proposition::Truth,
        1 => bounds.pop().expect("one exact-cast bound"),
        _ => Proposition::Conjunction(bounds),
    })
}

/// An exact conversion from a closed carrier into a parameter width
/// owes the target's range at every width the body admits: `value
/// <= 2^N - 1` (or `2^(N-1) - 1`), and the target's least value
/// from a signed source. This is also how a literal
/// lands at a parameter width (`numeric_values.md`, "A symbolic
/// width uses an expected `u<N>`/`i<N>` type or an explicit exact
/// conversion").
fn exact_cast_to_parameter_width(
    source_type: IntegerType,
    target_type: IntegerType,
    operand: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    if !source_type.can_exact_cast_to(target_type) {
        return Err(OperationSemanticError::ExactCastUnsupportedCarriers {
            source: source_type,
            target: target_type,
        });
    }
    if operand.scalar_type() != ScalarType::Integer(source_type) {
        return Err(OperationSemanticError::ExactCastOperandTypeMismatch {
            declared: source_type,
            actual: operand.scalar_type(),
        });
    }
    let ScalarTerm::Value { id, .. } = operand else {
        return Err(OperationSemanticError::ExactCastRequiresValueOrLiteralOperand);
    };
    let value = IntegerMathTerm::MathValue {
        source_type,
        value: *id,
    };
    let mut bounds = Vec::with_capacity(2);
    // An unsigned value is at least zero, which is at least every
    // carrier's least value.
    if source_type.sign() == IntegerSign::Signed {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::carrier_minimum(target_type),
            value.clone(),
        ));
    }
    bounds.push(Proposition::IntegerMathLessOrEqual(
        value,
        IntegerMathTerm::carrier_maximum(target_type),
    ));
    Ok(canonical_conjunction(bounds))
}

fn compare_math_literals(left: IntegerMathLiteral, right: IntegerMathLiteral) -> Ordering {
    match (left.negative(), right.negative()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.magnitude().cmp(&right.magnitude()),
        (true, true) => right.magnitude().cmp(&left.magnitude()),
    }
}

fn exact_shift_count_proposition(
    value_type: IntegerType,
    count_type: IntegerType,
    count: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let known_count_type = concrete_integer(count_type)?;
    let known_value_type = concrete_integer(value_type)?;
    if value_type.carrier() != IntegerCarrier::Fixed {
        return Err(
            OperationSemanticError::ExactShiftCountRequiresFixedValueInteger(value_type),
        );
    }
    if count_type.carrier() != IntegerCarrier::Fixed {
        return Err(
            OperationSemanticError::ExactShiftCountRequiresFixedCountInteger(count_type),
        );
    }
    if count.scalar_type() != ScalarType::Integer(count_type) {
        return Err(OperationSemanticError::ExactShiftCountTypeMismatch {
            declared: count_type,
            actual: count.scalar_type(),
        });
    }
    if let ScalarTerm::Integer { value, .. } = count {
        let count = match value {
            IntegerValue::Signed(count) => u128::try_from(*count).ok(),
            IntegerValue::Unsigned(count) => Some(*count),
        };
        return Ok(
            if count.is_some_and(|count| count < u128::from(known_value_type.bits())) {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            },
        );
    }

    let lower_bound = if count_type.sign() == IntegerSign::Signed {
        Some(Proposition::LessOrEqual(
            ScalarTerm::integer(count_type, IntegerValue::Signed(0))
                .map_err(OperationSemanticError::InvalidProposition)?,
            count.clone(),
        ))
    } else {
        None
    };
    let mut bounds = Vec::with_capacity(2);
    let maximum = u128::from(known_value_type.bits() - 1);
    let maximum = match count_type.sign() {
        IntegerSign::Signed => i128::try_from(maximum).ok().map(IntegerValue::Signed),
        IntegerSign::Unsigned => Some(IntegerValue::Unsigned(maximum)),
    };
    if let Some(maximum) = maximum
        && count_type.admits(maximum)
        && known_count_type.maximum_value() != maximum
    {
        bounds.push(Proposition::LessOrEqual(
            count.clone(),
            ScalarTerm::integer(count_type, maximum)
                .map_err(OperationSemanticError::InvalidProposition)?,
        ));
    }
    if let Some(lower_bound) = lower_bound {
        bounds.push(lower_bound);
    }
    Ok(match bounds.len() {
        0 => Proposition::Truth,
        1 => bounds.pop().expect("one exact-shift count bound"),
        _ => Proposition::Conjunction(bounds),
    })
}

fn nonzero_divisor_proposition(
    integer_type: IntegerType,
    divisor: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let known_integer_type = concrete_integer(integer_type)?;
    if integer_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::NonzeroDivisorRequiresFixedInteger(
            integer_type,
        ));
    }
    if divisor.scalar_type() != ScalarType::Integer(integer_type) {
        return Err(OperationSemanticError::NonzeroDivisorTypeMismatch {
            declared: integer_type,
            actual: divisor.scalar_type(),
        });
    }
    Ok(match integer_type.sign() {
        IntegerSign::Unsigned => Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
                .map_err(OperationSemanticError::InvalidProposition)?,
            divisor.clone(),
        ),
        IntegerSign::Signed => {
            let negative = Proposition::LessOrEqual(
                divisor.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
                    .map_err(OperationSemanticError::InvalidProposition)?,
            );
            if known_integer_type.bits() == 1 {
                negative
            } else {
                Proposition::Disjunction(vec![
                    negative,
                    Proposition::LessOrEqual(
                        ScalarTerm::integer(integer_type, IntegerValue::Signed(1))
                            .map_err(OperationSemanticError::InvalidProposition)?,
                        divisor.clone(),
                    ),
                ])
            }
        }
    })
}

fn exact_division_defined_proposition(
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let known_integer_type = concrete_integer(integer_type)?;
    if integer_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactDivisionRequiresFixedInteger(
            integer_type,
        ));
    }
    let expected = ScalarType::Integer(integer_type);
    if left.scalar_type() != expected || right.scalar_type() != expected {
        return Err(OperationSemanticError::ExactDivisionOperandTypeMismatch {
            declared: integer_type,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    if integer_type.sign() == IntegerSign::Unsigned {
        return Ok(Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
                .map_err(OperationSemanticError::InvalidProposition)?,
            right.clone(),
        ));
    }

    let minimum_plus_one = match known_integer_type.minimum_value() {
        IntegerValue::Signed(minimum) => IntegerValue::Signed(
            minimum
                .checked_add(1)
                .expect("fixed signed minimum has a successor"),
        ),
        IntegerValue::Unsigned(_) => unreachable!("signed carrier has a signed minimum"),
    };
    let minus_one_case = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(
            right.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
                .map_err(OperationSemanticError::InvalidProposition)?,
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, minimum_plus_one)
                .map_err(OperationSemanticError::InvalidProposition)?,
            left.clone(),
        ),
    ]);
    if known_integer_type.bits() == 1 {
        return Ok(minus_one_case);
    }

    Ok(Proposition::Disjunction(vec![
        Proposition::LessOrEqual(
            right.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-2))
                .map_err(OperationSemanticError::InvalidProposition)?,
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Signed(1))
                .map_err(OperationSemanticError::InvalidProposition)?,
            right.clone(),
        ),
        minus_one_case,
    ]))
}
