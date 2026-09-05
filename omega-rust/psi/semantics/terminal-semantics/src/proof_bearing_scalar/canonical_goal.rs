//! Canonical proposition algebra for proof-bearing scalar leaves.
//!
//! This owner defines the exact operation-local goals that certificate
//! producers must prove. It does not inspect predecessor definitions, choose
//! sufficient preimages, or admit proof evidence.

use semantic_vocabulary::{
    IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
    Proposition, ScalarTerm, ScalarType,
};

use crate::{OperationSemanticError, OperationSemanticTag, ScalarLeafGoalShape};

use super::ProofBearingScalarLeafSchema;

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
    pub const fn shape(&self) -> ScalarLeafGoalShape {
        match self {
            Self::ExactCastRepresentable { .. } => ScalarLeafGoalShape::ExactCastRepresentable,
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

    /// Project the canonical proposition for every proof-bearing scalar goal.
    pub fn kernel_proposition(&self) -> Result<Proposition, OperationSemanticError> {
        let proposition = match self {
            Self::ExactCastRepresentable {
                source_type,
                target_type,
                operand,
            } => exact_cast_representability_proposition(*source_type, *target_type, operand)?,
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
                    return Err(OperationSemanticError::ExactArithmeticExpressionShapeMismatch);
                }
            },
        };
        proposition
            .validate()
            .map_err(OperationSemanticError::InvalidProposition)?;
        Ok(proposition)
    }
}

fn exact_multiply_representability_proposition(
    integer_type: IntegerType,
    expression: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
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
    if *scalar_type != integer_type {
        return Err(
            OperationSemanticError::ExactArithmeticExpressionTypeMismatch {
                declared: integer_type,
                actual: ScalarType::Integer(*scalar_type),
            },
        );
    }
    if left.scalar_type() != ScalarType::Integer(integer_type)
        || right.scalar_type() != ScalarType::Integer(integer_type)
    {
        return Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch {
            declared: integer_type,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    let mathematical_left = fixed_integer_math_term(
        integer_type,
        left,
        |_| unreachable!("exact-multiply operand types were checked together"),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    let mathematical_right = fixed_integer_math_term(
        integer_type,
        right,
        |_| unreachable!("exact-multiply operand types were checked together"),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    if let (
        ScalarTerm::Integer {
            value: left_value, ..
        },
        ScalarTerm::Integer {
            value: right_value, ..
        },
    ) = (left.as_ref(), right.as_ref())
    {
        return Ok(
            if integer_type.exact_mul(*left_value, *right_value).is_some() {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            },
        );
    }
    if matches!(mathematical_left, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
        || matches!(mathematical_right, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
    {
        return Ok(Proposition::Truth);
    }
    let one = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    if left.integer_value() == Some((integer_type, one))
        || right.integer_value() == Some((integer_type, one))
    {
        return Ok(Proposition::Truth);
    }

    let product =
        IntegerMathTerm::Multiply(Box::new(mathematical_left), Box::new(mathematical_right));
    let embedded_negative_one = integer_type.sign() == IntegerSign::Signed
        && (left.integer_value() == Some((integer_type, IntegerValue::Signed(-1)))
            || right.integer_value() == Some((integer_type, IntegerValue::Signed(-1))));
    let mut bounds = Vec::with_capacity(2);
    if integer_type.sign() == IntegerSign::Signed && !embedded_negative_one {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(integer_type.minimum_value()),
            product.clone(),
        ));
    }
    bounds.push(Proposition::IntegerMathLessOrEqual(
        product,
        IntegerMathTerm::literal(integer_type.maximum_value()),
    ));
    Ok(canonical_conjunction(bounds))
}

fn exact_subtract_representability_proposition(
    integer_type: IntegerType,
    expression: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    if integer_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactArithmeticRequiresFixedInteger(
            integer_type,
        ));
    }
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = expression
    else {
        unreachable!("exact-subtract projection is selected by expression shape")
    };
    if *scalar_type != integer_type {
        return Err(
            OperationSemanticError::ExactArithmeticExpressionTypeMismatch {
                declared: integer_type,
                actual: ScalarType::Integer(*scalar_type),
            },
        );
    }
    if left.scalar_type() != ScalarType::Integer(integer_type)
        || right.scalar_type() != ScalarType::Integer(integer_type)
    {
        return Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch {
            declared: integer_type,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    let mathematical_left = fixed_integer_math_term(
        integer_type,
        left,
        |_| unreachable!("exact-subtract operand types were checked together"),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    let mathematical_right = fixed_integer_math_term(
        integer_type,
        right,
        |_| unreachable!("exact-subtract operand types were checked together"),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    if let (
        ScalarTerm::Integer {
            value: left_value, ..
        },
        ScalarTerm::Integer {
            value: right_value, ..
        },
    ) = (left.as_ref(), right.as_ref())
    {
        return Ok(
            if integer_type.exact_sub(*left_value, *right_value).is_some() {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            },
        );
    }
    if matches!(mathematical_right, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
        || left == right
    {
        return Ok(Proposition::Truth);
    }

    let difference =
        IntegerMathTerm::Subtract(Box::new(mathematical_left), Box::new(mathematical_right));
    let mut bounds = Vec::with_capacity(2);
    bounds.push(Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(integer_type.minimum_value()),
        difference.clone(),
    ));
    if integer_type.sign() == IntegerSign::Signed {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            difference,
            IntegerMathTerm::literal(integer_type.maximum_value()),
        ));
    }
    Ok(canonical_conjunction(bounds))
}

fn exact_add_representability_proposition(
    integer_type: IntegerType,
    expression: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    if integer_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactArithmeticRequiresFixedInteger(
            integer_type,
        ));
    }
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = expression
    else {
        unreachable!("exact-add projection is selected by expression shape")
    };
    if *scalar_type != integer_type {
        return Err(
            OperationSemanticError::ExactArithmeticExpressionTypeMismatch {
                declared: integer_type,
                actual: ScalarType::Integer(*scalar_type),
            },
        );
    }
    if left.scalar_type() != ScalarType::Integer(integer_type)
        || right.scalar_type() != ScalarType::Integer(integer_type)
    {
        return Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch {
            declared: integer_type,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    let mathematical_left = fixed_integer_math_term(
        integer_type,
        left,
        |_| unreachable!("exact-add operand types were checked together"),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    let mathematical_right = fixed_integer_math_term(
        integer_type,
        right,
        |_| unreachable!("exact-add operand types were checked together"),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    if let (
        ScalarTerm::Integer {
            value: left_value, ..
        },
        ScalarTerm::Integer {
            value: right_value, ..
        },
    ) = (left.as_ref(), right.as_ref())
    {
        return Ok(
            if integer_type.exact_add(*left_value, *right_value).is_some() {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            },
        );
    }
    if matches!(mathematical_left, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
        || matches!(mathematical_right, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0)
    {
        return Ok(Proposition::Truth);
    }

    let sum = IntegerMathTerm::Add(Box::new(mathematical_left), Box::new(mathematical_right));
    let mut bounds = Vec::with_capacity(2);
    if integer_type.sign() == IntegerSign::Signed {
        bounds.push(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(integer_type.minimum_value()),
            sum.clone(),
        ));
    }
    bounds.push(Proposition::IntegerMathLessOrEqual(
        sum,
        IntegerMathTerm::literal(integer_type.maximum_value()),
    ));
    Ok(canonical_conjunction(bounds))
}

fn fixed_integer_math_term(
    declared: IntegerType,
    term: &ScalarTerm,
    mismatch: impl FnOnce(ScalarType) -> OperationSemanticError,
    unsupported: OperationSemanticError,
) -> Result<IntegerMathTerm, OperationSemanticError> {
    if term.scalar_type() != ScalarType::Integer(declared) {
        return Err(mismatch(term.scalar_type()));
    }
    match term {
        ScalarTerm::Value { id, .. } => Ok(IntegerMathTerm::MathValue {
            source_type: declared,
            value: *id,
        }),
        ScalarTerm::Integer { value, .. } => {
            ScalarTerm::integer(declared, *value)
                .map_err(OperationSemanticError::InvalidProposition)?;
            Ok(IntegerMathTerm::literal(*value))
        }
        _ => Err(unsupported),
    }
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

fn exact_shift_left_representability_proposition(
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    let count_proposition = exact_shift_count_proposition(value_type, count_type, count)?;
    let mathematical_value = fixed_integer_math_term(
        value_type,
        value,
        |actual| OperationSemanticError::ExactShiftLeftValueTypeMismatch {
            declared: value_type,
            actual,
        },
        OperationSemanticError::ExactShiftLeftRequiresValueOrLiteralOperand,
    )?;
    let mathematical_count = fixed_integer_math_term(
        count_type,
        count,
        |actual| OperationSemanticError::ExactShiftCountTypeMismatch {
            declared: count_type,
            actual,
        },
        OperationSemanticError::ExactShiftLeftRequiresValueOrLiteralCount,
    )?;
    if count_proposition == Proposition::Falsehood {
        return Ok(Proposition::Falsehood);
    }
    if let (ScalarTerm::Integer { value, .. }, ScalarTerm::Integer { value: count, .. }) =
        (value, count)
    {
        return Ok(
            if value_type
                .exact_shift_left(*value, count_type, *count)
                .is_some()
            {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            },
        );
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
            IntegerMathTerm::literal(value_type.minimum_value()),
            shifted.clone(),
        ));
    }
    conjuncts.push(Proposition::IntegerMathLessOrEqual(
        shifted,
        IntegerMathTerm::literal(value_type.maximum_value()),
    ));
    Ok(canonical_conjunction(conjuncts))
}

fn exact_cast_representability_proposition(
    source_type: IntegerType,
    target_type: IntegerType,
    operand: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    if source_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactCastRequiresFixedSourceInteger(
            source_type,
        ));
    }
    if target_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactCastRequiresFixedTargetInteger(
            target_type,
        ));
    }
    if operand.scalar_type() != ScalarType::Integer(source_type) {
        return Err(OperationSemanticError::ExactCastOperandTypeMismatch {
            declared: source_type,
            actual: operand.scalar_type(),
        });
    }
    if let ScalarTerm::Integer { value, .. } = operand {
        return Ok(
            if source_type
                .exact_cast_value_to(target_type, *value)
                .is_some()
            {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            },
        );
    }
    let ScalarTerm::Value { id, .. } = operand else {
        return Err(OperationSemanticError::ExactCastRequiresValueOrLiteralOperand);
    };
    let value = IntegerMathTerm::MathValue {
        source_type,
        value: *id,
    };
    let source_minimum = IntegerMathLiteral::from_integer_value(source_type.minimum_value());
    let source_maximum = IntegerMathLiteral::from_integer_value(source_type.maximum_value());
    let target_minimum = IntegerMathLiteral::from_integer_value(target_type.minimum_value());
    let target_maximum = IntegerMathLiteral::from_integer_value(target_type.maximum_value());
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

fn compare_math_literals(
    left: IntegerMathLiteral,
    right: IntegerMathLiteral,
) -> std::cmp::Ordering {
    match (left.negative(), right.negative()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => left.magnitude().cmp(&right.magnitude()),
        (true, true) => right.magnitude().cmp(&left.magnitude()),
    }
}

fn exact_shift_count_proposition(
    value_type: IntegerType,
    count_type: IntegerType,
    count: &ScalarTerm,
) -> Result<Proposition, OperationSemanticError> {
    if value_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactShiftCountRequiresFixedValueInteger(value_type));
    }
    if count_type.carrier() != IntegerCarrier::Fixed {
        return Err(OperationSemanticError::ExactShiftCountRequiresFixedCountInteger(count_type));
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
            if count.is_some_and(|count| count < u128::from(value_type.bits())) {
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
    let maximum = u128::from(value_type.bits() - 1);
    let maximum = match count_type.sign() {
        IntegerSign::Signed => i128::try_from(maximum).ok().map(IntegerValue::Signed),
        IntegerSign::Unsigned => Some(IntegerValue::Unsigned(maximum)),
    };
    // Once the bound is admitted it cannot exceed the carrier maximum, so
    // inequality here is exactly the strict-greater test used by reduction.
    if let Some(maximum) = maximum
        && count_type.admits(maximum)
        && count_type.maximum_value() != maximum
    {
        bounds.push(Proposition::LessOrEqual(
            count.clone(),
            ScalarTerm::integer(count_type, maximum)
                .map_err(OperationSemanticError::InvalidProposition)?,
        ));
    }
    // A value-leading upper bound precedes a literal-leading lower bound in
    // canonical proposition order. Keep the semantic producer's conjunction
    // directly encodable rather than relying on a downstream reorder.
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
            if integer_type.bits() == 1 {
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

    let minimum_plus_one = match integer_type.minimum_value() {
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
    if integer_type.bits() == 1 {
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

pub(super) fn canonical_goal(
    tag: OperationSemanticTag,
    schema: ProofBearingScalarLeafSchema,
    result_type: IntegerType,
    auxiliary_type: Option<IntegerType>,
    inputs: &[ScalarTerm],
    denotation: ScalarTerm,
) -> Result<CanonicalScalarGoal, OperationSemanticError> {
    let invalid = || OperationSemanticError::ProofBearingScalarSchemaMismatch(tag);
    match (schema.goal, inputs) {
        (ScalarLeafGoalShape::ExactCastRepresentable, [operand]) => {
            Ok(CanonicalScalarGoal::ExactCastRepresentable {
                source_type: auxiliary_type.ok_or_else(invalid)?,
                target_type: result_type,
                operand: operand.clone(),
            })
        }
        (ScalarLeafGoalShape::ExactShiftCount, [_, count]) => {
            Ok(CanonicalScalarGoal::ExactShiftCount {
                value_type: result_type,
                count_type: auxiliary_type.ok_or_else(invalid)?,
                count: count.clone(),
            })
        }
        (ScalarLeafGoalShape::ExactShiftLeftRepresentable, [value, count]) => {
            Ok(CanonicalScalarGoal::ExactShiftLeftRepresentable {
                value_type: result_type,
                count_type: auxiliary_type.ok_or_else(invalid)?,
                value: value.clone(),
                count: count.clone(),
            })
        }
        (ScalarLeafGoalShape::ExactArithmeticRepresentable, [_, _]) => {
            Ok(CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: result_type,
                expression: denotation,
            })
        }
        (ScalarLeafGoalShape::ExactDivisionDefined, [left, right]) => {
            Ok(CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: result_type,
                left: left.clone(),
                right: right.clone(),
            })
        }
        (ScalarLeafGoalShape::NonzeroDivisor, [_, divisor]) => {
            Ok(CanonicalScalarGoal::NonzeroDivisor {
                integer_type: result_type,
                divisor: divisor.clone(),
            })
        }
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::ValueId;

    use super::*;

    #[test]
    fn nonzero_divisor_projects_the_exact_fixed_carrier_proposition() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_divisor = ScalarTerm::value(
            ValueId::new(20).expect("signed divisor"),
            ScalarType::Integer(signed),
        );
        let negative_one = ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("-1i8");
        let positive_one = ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("1i8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: signed,
                divisor: signed_divisor.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::Disjunction(vec![
                Proposition::LessOrEqual(signed_divisor.clone(), negative_one),
                Proposition::LessOrEqual(positive_one, signed_divisor),
            ])),
        );

        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_divisor = ScalarTerm::value(
            ValueId::new(21).expect("unsigned divisor"),
            ScalarType::Integer(unsigned),
        );
        let one = ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("1u8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: unsigned,
                divisor: unsigned_divisor.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::LessOrEqual(one, unsigned_divisor)),
        );
    }

    #[test]
    fn nonzero_divisor_handles_i1_and_rejects_invalid_carrier_identity() {
        let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let divisor = ScalarTerm::value(
            ValueId::new(22).expect("i1 divisor"),
            ScalarType::Integer(i1),
        );
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: i1,
                divisor: divisor.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::LessOrEqual(
                divisor,
                ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("-1i1"),
            )),
        );

        let address = IntegerType::address(64).expect("address64");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: address,
                divisor: ScalarTerm::value(
                    ValueId::new(23).expect("address divisor"),
                    ScalarType::Integer(address),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::NonzeroDivisorRequiresFixedInteger(
                address,
            )),
        );

        let i8 = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: i8,
                divisor: ScalarTerm::boolean(true),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::NonzeroDivisorTypeMismatch {
                declared: i8,
                actual: ScalarType::Boolean,
            }),
        );

        let u8 = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: i8,
                divisor: ScalarTerm::value(
                    ValueId::new(25).expect("wrong-carrier divisor"),
                    ScalarType::Integer(u8),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::NonzeroDivisorTypeMismatch {
                declared: i8,
                actual: ScalarType::Integer(u8),
            }),
        );
    }

    #[test]
    fn exact_division_projects_complete_signed_and_unsigned_definedness() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(26).expect("signed dividend"),
            ScalarType::Integer(signed),
        );
        let right = ScalarTerm::value(
            ValueId::new(27).expect("signed divisor"),
            ScalarType::Integer(signed),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: signed,
                left: left.clone(),
                right: right.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::Disjunction(vec![
                Proposition::LessOrEqual(
                    right.clone(),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-2)).unwrap(),
                ),
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).unwrap(),
                    right.clone(),
                ),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(
                        right,
                        ScalarTerm::integer(signed, IntegerValue::Signed(-1)).unwrap(),
                    ),
                    Proposition::LessOrEqual(
                        ScalarTerm::integer(signed, IntegerValue::Signed(-127)).unwrap(),
                        left,
                    ),
                ]),
            ])),
        );

        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let left = ScalarTerm::value(
            ValueId::new(28).expect("unsigned dividend"),
            ScalarType::Integer(unsigned),
        );
        let right = ScalarTerm::value(
            ValueId::new(29).expect("unsigned divisor"),
            ScalarType::Integer(unsigned),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: unsigned,
                left,
                right: right.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).unwrap(),
                right,
            )),
        );
    }

    #[test]
    fn exact_division_handles_i1_and_rejects_invalid_carrier_identity() {
        let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let left = ScalarTerm::value(
            ValueId::new(30).expect("i1 dividend"),
            ScalarType::Integer(i1),
        );
        let right = ScalarTerm::value(
            ValueId::new(31).expect("i1 divisor"),
            ScalarType::Integer(i1),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: i1,
                left: left.clone(),
                right: right.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    right,
                    ScalarTerm::integer(i1, IntegerValue::Signed(-1)).unwrap(),
                ),
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i1, IntegerValue::Signed(0)).unwrap(),
                    left,
                ),
            ])),
        );

        let address = IntegerType::address(64).expect("address64");
        let address_value = ScalarTerm::value(
            ValueId::new(32).expect("address"),
            ScalarType::Integer(address),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: address,
                left: address_value.clone(),
                right: address_value,
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactDivisionRequiresFixedInteger(
                address,
            )),
        );

        let i8 = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i8_value = ScalarTerm::value(ValueId::new(33).expect("i8"), ScalarType::Integer(i8));
        assert_eq!(
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: i8,
                left: ScalarTerm::boolean(true),
                right: i8_value,
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactDivisionOperandTypeMismatch {
                declared: i8,
                left: ScalarType::Boolean,
                right: ScalarType::Integer(i8),
            }),
        );
    }

    #[test]
    fn exact_shift_count_projects_only_non_carrier_implied_bounds() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let unsigned_count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_count = ScalarTerm::value(
            ValueId::new(34).expect("unsigned count"),
            ScalarType::Integer(unsigned_count_type),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactShiftCount {
                value_type,
                count_type: unsigned_count_type,
                count: unsigned_count.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::LessOrEqual(
                unsigned_count,
                ScalarTerm::integer(unsigned_count_type, IntegerValue::Unsigned(63)).unwrap(),
            )),
        );

        let signed_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_count = ScalarTerm::value(
            ValueId::new(35).expect("signed count"),
            ScalarType::Integer(signed_count_type),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactShiftCount {
                value_type,
                count_type: signed_count_type,
                count: signed_count.clone(),
            }
            .kernel_proposition(),
            Ok(Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    signed_count.clone(),
                    ScalarTerm::integer(signed_count_type, IntegerValue::Signed(63)).unwrap(),
                ),
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed_count_type, IntegerValue::Signed(0)).unwrap(),
                    signed_count,
                ),
            ])),
        );

        let narrow_count_type = IntegerType::new(IntegerSign::Unsigned, 5).expect("u5");
        assert_eq!(
            CanonicalScalarGoal::ExactShiftCount {
                value_type,
                count_type: narrow_count_type,
                count: ScalarTerm::value(
                    ValueId::new(36).expect("narrow count"),
                    ScalarType::Integer(narrow_count_type),
                ),
            }
            .kernel_proposition(),
            Ok(Proposition::Truth),
        );
    }

    #[test]
    fn exact_shift_count_normalizes_known_literal_boundary() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        for (literal, expected) in [(63, Proposition::Truth), (64, Proposition::Falsehood)] {
            let goal = CanonicalScalarGoal::ExactShiftCount {
                value_type,
                count_type,
                count: ScalarTerm::integer(count_type, IntegerValue::Unsigned(literal))
                    .expect("u8 shift count"),
            };
            assert_eq!(goal.kernel_proposition(), Ok(expected));
        }
    }

    #[test]
    fn exact_shift_count_rejects_invalid_carrier_identity() {
        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let address = IntegerType::address(64).expect("address64");
        assert_eq!(
            CanonicalScalarGoal::ExactShiftCount {
                value_type: address,
                count_type: u8_type,
                count: ScalarTerm::value(
                    ValueId::new(37).expect("count"),
                    ScalarType::Integer(u8_type),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactShiftCountRequiresFixedValueInteger(address),),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactShiftCount {
                value_type: u64_type,
                count_type: address,
                count: ScalarTerm::value(
                    ValueId::new(38).expect("address count"),
                    ScalarType::Integer(address),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactShiftCountRequiresFixedCountInteger(address),),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactShiftCount {
                value_type: u64_type,
                count_type: u8_type,
                count: ScalarTerm::boolean(true),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactShiftCountTypeMismatch {
                declared: u8_type,
                actual: ScalarType::Boolean,
            }),
        );
    }

    #[test]
    fn exact_multiply_projects_bounds_and_normalizes_embedded_negative_one() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left_id = ValueId::new(24).expect("left");
        let right_id = ValueId::new(25).expect("right");
        let left = ScalarTerm::value(left_id, ScalarType::Integer(integer));
        let right = ScalarTerm::value(right_id, ScalarType::Integer(integer));
        let product = IntegerMathTerm::Multiply(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer,
                value: left_id,
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer,
                value: right_id,
            }),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: integer,
                expression: ScalarTerm::exact_integer_multiply(integer, left.clone(), right)
                    .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::Conjunction(vec![
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                    product.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    product,
                    IntegerMathTerm::literal(IntegerValue::Signed(127)),
                ),
            ])),
        );

        let negative_one = ScalarTerm::integer(integer, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: integer,
                expression: ScalarTerm::exact_integer_multiply(
                    integer,
                    left.clone(),
                    negative_one.clone(),
                )
                .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Multiply(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer,
                        value: left_id,
                    }),
                    Box::new(IntegerMathTerm::literal(IntegerValue::Signed(-1))),
                ),
                IntegerMathTerm::literal(IntegerValue::Signed(127)),
            )),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: integer,
                expression: ScalarTerm::exact_integer_multiply(integer, negative_one, left)
                    .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Multiply(
                    Box::new(IntegerMathTerm::literal(IntegerValue::Signed(-1))),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer,
                        value: left_id,
                    }),
                ),
                IntegerMathTerm::literal(IntegerValue::Signed(127)),
            )),
        );

        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_left_id = ValueId::new(26).expect("unsigned left");
        let unsigned_right_id = ValueId::new(27).expect("unsigned right");
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: unsigned,
                expression: ScalarTerm::exact_integer_multiply(
                    unsigned,
                    ScalarTerm::value(unsigned_left_id, ScalarType::Integer(unsigned)),
                    ScalarTerm::value(unsigned_right_id, ScalarType::Integer(unsigned)),
                )
                .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Multiply(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: unsigned,
                        value: unsigned_left_id,
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: unsigned,
                        value: unsigned_right_id,
                    }),
                ),
                IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
            )),
        );
    }

    #[test]
    fn exact_multiply_folds_closed_and_zero_products() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let literal = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
        let value = ScalarTerm::value(ValueId::new(28).unwrap(), ScalarType::Integer(integer));
        for (left, right, expected) in [
            (literal(12), literal(10), Proposition::Truth),
            (literal(64), literal(2), Proposition::Falsehood),
            (value.clone(), literal(0), Proposition::Truth),
            (literal(0), value.clone(), Proposition::Truth),
            (value.clone(), literal(1), Proposition::Truth),
            (literal(1), value, Proposition::Truth),
        ] {
            assert_eq!(
                CanonicalScalarGoal::ExactArithmeticRepresentable {
                    integer_type: integer,
                    expression: ScalarTerm::exact_integer_multiply(integer, left, right).unwrap(),
                }
                .kernel_proposition(),
                Ok(expected),
            );
        }
    }

    #[test]
    fn exact_multiply_rejects_malformed_declared_and_operand_types() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let address = IntegerType::address(64).expect("address");
        let i8_value = ScalarTerm::value(
            ValueId::new(29).expect("i8 value"),
            ScalarType::Integer(i8_type),
        );
        let i16_value = ScalarTerm::value(
            ValueId::new(30).expect("i16 value"),
            ScalarType::Integer(i16_type),
        );
        let malformed = |scalar_type, left, right| ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        };
        assert!(matches!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: address,
                expression: malformed(address, i8_value.clone(), i8_value.clone()),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticRequiresFixedInteger(actual))
                if actual == address
        ));
        assert!(matches!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(i16_type, i8_value.clone(), i8_value.clone()),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticExpressionTypeMismatch { .. })
        ));
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: i8_value.clone(),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticExpressionShapeMismatch),
        );
        assert!(matches!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(i8_type, i8_value.clone(), i16_value),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch { .. })
        ));
        let malformed_zero = ScalarTerm::Integer {
            scalar_type: i8_type,
            value: IntegerValue::Unsigned(0),
        };
        for expression in [
            malformed(i8_type, i8_value.clone(), malformed_zero.clone()),
            malformed(i8_type, malformed_zero, i8_value.clone()),
        ] {
            assert!(matches!(
                CanonicalScalarGoal::ExactArithmeticRepresentable {
                    integer_type: i8_type,
                    expression,
                }
                .kernel_proposition(),
                Err(OperationSemanticError::InvalidProposition(_))
            ));
        }
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(
                    i8_type,
                    i8_value.clone(),
                    ScalarTerm::exact_integer_multiply(
                        i8_type,
                        i8_value,
                        ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap(),
                    )
                    .unwrap(),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand),
        );
    }

    #[test]
    fn exact_subtract_projects_canonical_mathematical_carrier_bounds() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left_id = ValueId::new(31).expect("left");
        let right_id = ValueId::new(32).expect("right");
        let difference = IntegerMathTerm::Subtract(
            Box::new(IntegerMathTerm::MathValue {
                source_type: signed,
                value: left_id,
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: signed,
                value: right_id,
            }),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: signed,
                expression: ScalarTerm::exact_integer_subtract(
                    signed,
                    ScalarTerm::value(left_id, ScalarType::Integer(signed)),
                    ScalarTerm::value(right_id, ScalarType::Integer(signed)),
                )
                .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::Conjunction(vec![
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                    difference.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    difference,
                    IntegerMathTerm::literal(IntegerValue::Signed(127)),
                ),
            ])),
        );

        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let left_id = ValueId::new(33).expect("left");
        let right_id = ValueId::new(34).expect("right");
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: unsigned,
                expression: ScalarTerm::exact_integer_subtract(
                    unsigned,
                    ScalarTerm::value(left_id, ScalarType::Integer(unsigned)),
                    ScalarTerm::value(right_id, ScalarType::Integer(unsigned)),
                )
                .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                IntegerMathTerm::Subtract(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: unsigned,
                        value: left_id,
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: unsigned,
                        value: right_id,
                    }),
                ),
            )),
        );
    }

    #[test]
    fn exact_subtract_folds_closed_and_right_zero_identity_forms() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        for (left, right, expected) in [
            (-120, 7, Proposition::Truth),
            (-121, 8, Proposition::Falsehood),
        ] {
            assert_eq!(
                CanonicalScalarGoal::ExactArithmeticRepresentable {
                    integer_type: integer,
                    expression: ScalarTerm::exact_integer_subtract(
                        integer,
                        ScalarTerm::integer(integer, IntegerValue::Signed(left)).unwrap(),
                        ScalarTerm::integer(integer, IntegerValue::Signed(right)).unwrap(),
                    )
                    .unwrap(),
                }
                .kernel_proposition(),
                Ok(expected),
            );
        }
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: integer,
                expression: ScalarTerm::exact_integer_subtract(
                    integer,
                    ScalarTerm::value(
                        ValueId::new(35).expect("value"),
                        ScalarType::Integer(integer),
                    ),
                    ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap(),
                )
                .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::Truth),
        );
        let value = ScalarTerm::value(
            ValueId::new(38).expect("same value"),
            ScalarType::Integer(integer),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: integer,
                expression: ScalarTerm::exact_integer_subtract(integer, value.clone(), value,)
                    .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::Truth),
        );
    }

    #[test]
    fn exact_add_projects_canonical_mathematical_carrier_bounds() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_left_id = ValueId::new(24).expect("signed left");
        let signed_right_id = ValueId::new(25).expect("signed right");
        let signed_left = ScalarTerm::value(signed_left_id, ScalarType::Integer(signed));
        let signed_right = ScalarTerm::value(signed_right_id, ScalarType::Integer(signed));
        let signed_sum = IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::MathValue {
                source_type: signed,
                value: signed_left_id,
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: signed,
                value: signed_right_id,
            }),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: signed,
                expression: ScalarTerm::exact_integer_add(signed, signed_left, signed_right)
                    .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::Conjunction(vec![
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                    signed_sum.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    signed_sum,
                    IntegerMathTerm::literal(IntegerValue::Signed(127)),
                ),
            ])),
        );

        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_left_id = ValueId::new(26).expect("unsigned left");
        let unsigned_right_id = ValueId::new(27).expect("unsigned right");
        let unsigned_left = ScalarTerm::value(unsigned_left_id, ScalarType::Integer(unsigned));
        let unsigned_right = ScalarTerm::value(unsigned_right_id, ScalarType::Integer(unsigned));
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: unsigned,
                expression: ScalarTerm::exact_integer_add(unsigned, unsigned_left, unsigned_right,)
                    .unwrap(),
            }
            .kernel_proposition(),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: unsigned,
                        value: unsigned_left_id,
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: unsigned,
                        value: unsigned_right_id,
                    }),
                ),
                IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
            )),
        );
    }

    #[test]
    fn exact_add_folds_closed_and_zero_identity_forms() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        for (left, right, expected) in [
            (120, 7, Proposition::Truth),
            (120, 8, Proposition::Falsehood),
        ] {
            assert_eq!(
                CanonicalScalarGoal::ExactArithmeticRepresentable {
                    integer_type: integer,
                    expression: ScalarTerm::exact_integer_add(
                        integer,
                        ScalarTerm::integer(integer, IntegerValue::Signed(left)).unwrap(),
                        ScalarTerm::integer(integer, IntegerValue::Signed(right)).unwrap(),
                    )
                    .unwrap(),
                }
                .kernel_proposition(),
                Ok(expected),
            );
        }
        let value = ScalarTerm::value(
            ValueId::new(28).expect("value"),
            ScalarType::Integer(integer),
        );
        let zero = ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap();
        for expression in [
            ScalarTerm::exact_integer_add(integer, value.clone(), zero.clone()).unwrap(),
            ScalarTerm::exact_integer_add(integer, zero, value).unwrap(),
        ] {
            assert_eq!(
                CanonicalScalarGoal::ExactArithmeticRepresentable {
                    integer_type: integer,
                    expression,
                }
                .kernel_proposition(),
                Ok(Proposition::Truth),
            );
        }
    }

    #[test]
    fn exact_add_rejects_malformed_declared_and_operand_types() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let address = IntegerType::address(64).expect("address");
        let i8_value = ScalarTerm::value(
            ValueId::new(29).expect("i8 value"),
            ScalarType::Integer(i8_type),
        );
        let i16_value = ScalarTerm::value(
            ValueId::new(30).expect("i16 value"),
            ScalarType::Integer(i16_type),
        );
        let malformed = |scalar_type, left, right| ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        };
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: address,
                expression: malformed(address, i8_value.clone(), i8_value.clone()),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticRequiresFixedInteger(
                address,
            )),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(i16_type, i8_value.clone(), i8_value.clone()),
            }
            .kernel_proposition(),
            Err(
                OperationSemanticError::ExactArithmeticExpressionTypeMismatch {
                    declared: i8_type,
                    actual: ScalarType::Integer(i16_type),
                },
            ),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(i8_type, i8_value.clone(), i16_value),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch {
                declared: i8_type,
                left: ScalarType::Integer(i8_type),
                right: ScalarType::Integer(i16_type),
            }),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(
                    i8_type,
                    i8_value,
                    ScalarTerm::exact_integer_add(
                        i8_type,
                        ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap(),
                        ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).unwrap(),
                    )
                    .unwrap(),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand),
        );
    }

    #[test]
    fn exact_subtract_rejects_malformed_declared_and_operand_types() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let address = IntegerType::address(64).expect("address");
        let i8_value = ScalarTerm::value(
            ValueId::new(36).expect("i8 value"),
            ScalarType::Integer(i8_type),
        );
        let i16_value = ScalarTerm::value(
            ValueId::new(37).expect("i16 value"),
            ScalarType::Integer(i16_type),
        );
        let malformed = |scalar_type, left, right| ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        };
        assert!(matches!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: address,
                expression: malformed(address, i8_value.clone(), i8_value.clone()),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticRequiresFixedInteger(actual))
                if actual == address
        ));
        assert!(matches!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(i16_type, i8_value.clone(), i8_value.clone()),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticExpressionTypeMismatch { .. })
        ));
        assert!(matches!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(i8_type, i8_value.clone(), i16_value),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch { .. })
        ));
        assert_eq!(
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: i8_type,
                expression: malformed(
                    i8_type,
                    i8_value.clone(),
                    ScalarTerm::exact_integer_add(
                        i8_type,
                        i8_value,
                        ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap(),
                    )
                    .unwrap(),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand),
        );
    }

    #[test]
    fn exact_shift_left_projects_count_then_mathematical_carrier_bounds() {
        let value_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let value_id = ValueId::new(94).expect("value");
        let count_id = ValueId::new(95).expect("count");
        let value = ScalarTerm::value(value_id, ScalarType::Integer(value_type));
        let count = ScalarTerm::value(count_id, ScalarType::Integer(count_type));
        let shifted = IntegerMathTerm::ShiftLeft {
            value: Box::new(IntegerMathTerm::MathValue {
                source_type: value_type,
                value: value_id,
            }),
            count: Box::new(IntegerMathTerm::MathValue {
                source_type: count_type,
                value: count_id,
            }),
        };
        assert_eq!(
            CanonicalScalarGoal::ExactShiftLeftRepresentable {
                value_type,
                count_type,
                value,
                count,
            }
            .kernel_proposition(),
            Ok(Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::value(count_id, ScalarType::Integer(count_type)),
                    ScalarTerm::integer(count_type, IntegerValue::Signed(7)).unwrap(),
                ),
                Proposition::LessOrEqual(
                    ScalarTerm::integer(count_type, IntegerValue::Signed(0)).unwrap(),
                    ScalarTerm::value(count_id, ScalarType::Integer(count_type)),
                ),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                    shifted.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    shifted,
                    IntegerMathTerm::literal(IntegerValue::Signed(127)),
                ),
            ])),
        );
    }

    #[test]
    fn exact_shift_left_folds_closed_safety_and_rejects_malformed_value() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
        for (value, count, expected) in [
            (31, 3, Proposition::Truth),
            (32, 3, Proposition::Falsehood),
            (1, 8, Proposition::Falsehood),
        ] {
            assert_eq!(
                CanonicalScalarGoal::ExactShiftLeftRepresentable {
                    value_type,
                    count_type,
                    value: ScalarTerm::integer(value_type, IntegerValue::Unsigned(value)).unwrap(),
                    count: ScalarTerm::integer(count_type, IntegerValue::Unsigned(count)).unwrap(),
                }
                .kernel_proposition(),
                Ok(expected),
            );
        }
        assert_eq!(
            CanonicalScalarGoal::ExactShiftLeftRepresentable {
                value_type,
                count_type,
                value: ScalarTerm::boolean(true),
                count: ScalarTerm::integer(count_type, IntegerValue::Unsigned(8)).unwrap(),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::ExactShiftLeftValueTypeMismatch {
                declared: value_type,
                actual: ScalarType::Boolean,
            }),
        );
    }

    #[test]
    fn exact_cast_projects_canonical_mathematical_carrier_bounds() {
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let value_id = ValueId::new(91).expect("value");
        let value = ScalarTerm::value(value_id, ScalarType::Integer(i16_type));
        let mathematical_value = IntegerMathTerm::MathValue {
            source_type: i16_type,
            value: value_id,
        };
        assert_eq!(
            CanonicalScalarGoal::ExactCastRepresentable {
                source_type: i16_type,
                target_type: u8_type,
                operand: value,
            }
            .kernel_proposition(),
            Ok(Proposition::Conjunction(vec![
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                    mathematical_value.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    mathematical_value,
                    IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
                ),
            ]))
        );
    }

    #[test]
    fn exact_cast_normalizes_carrier_inclusion_and_closed_literals() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let value = ScalarTerm::value(
            ValueId::new(92).expect("value"),
            ScalarType::Integer(i8_type),
        );
        assert_eq!(
            CanonicalScalarGoal::ExactCastRepresentable {
                source_type: i8_type,
                target_type: i16_type,
                operand: value,
            }
            .kernel_proposition(),
            Ok(Proposition::Truth)
        );
        let literal = ScalarTerm::integer(i16_type, IntegerValue::Signed(-1)).expect("-1:i16");
        assert_eq!(
            CanonicalScalarGoal::ExactCastRepresentable {
                source_type: i16_type,
                target_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                operand: literal,
            }
            .kernel_proposition(),
            Ok(Proposition::Falsehood)
        );
    }
}
