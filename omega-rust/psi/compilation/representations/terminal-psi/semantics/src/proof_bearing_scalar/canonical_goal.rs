//! Canonical proposition algebra for proof-bearing scalar leaves.
//!
//! This owner defines the exact operation-local goals that certificate
//! producers must prove. It does not inspect predecessor definitions, choose
//! sufficient preimages, or admit proof evidence.

use semantic_vocabulary::{
    IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
    Proposition, ScalarTerm, ScalarType,
};

use crate::{OperationSemanticError, ScalarLeafGoalShape};

use super::ProofBearingScalarLeafSchema;
use crate::semantic_rows::OperationSemanticTag;

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
    let (mathematical_left, mathematical_right) =
        exact_integer_operands(integer_type, scalar_type, left, right, "exact-multiply")?;
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
    let one = integer_type.one();
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
    let (ScalarType::Integer(left_type), ScalarType::Integer(right_type)) =
        (left.scalar_type(), right.scalar_type())
    else {
        return Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch {
            declared: integer_type,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    };
    if !integer_type.admits_exact_subtract_operands(left_type, right_type) {
        return Err(OperationSemanticError::ExactArithmeticOperandTypeMismatch {
            declared: integer_type,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    let mathematical_left = integer_leaf_math_term(
        left_type,
        left,
        |_| unreachable!("exact-subtract operand types were checked together"),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    let mathematical_right = integer_leaf_math_term(
        right_type,
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
            if integer_type
                .exact_subtract_values(left_type, *left_value, right_type, *right_value)
                .is_some()
            {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            },
        );
    }
    if (left_type == integer_type
        && matches!(mathematical_right, IntegerMathTerm::IntegerLiteral(literal) if literal.magnitude() == 0))
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
    // A distance between wide addresses must also fit the u64 result. The
    // ordinary unsigned subtraction upper bound follows from its left carrier.
    if integer_type.sign() == IntegerSign::Signed || left_type != integer_type {
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
    let (mathematical_left, mathematical_right) =
        exact_integer_operands(integer_type, scalar_type, left, right, "exact-add")?;
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

/// The shared exact-binary checks for the equal-typed variants: the term's
/// declared scalar type restates the integer type, both operands carry it,
/// and each leaf reads as a math term.
fn exact_integer_operands(
    integer_type: IntegerType,
    scalar_type: &IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    leaf_label: &'static str,
) -> Result<(IntegerMathTerm, IntegerMathTerm), OperationSemanticError> {
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
    let mathematical_left = integer_leaf_math_term(
        integer_type,
        left,
        |_| unreachable!("{} operand types were checked together", leaf_label),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    let mathematical_right = integer_leaf_math_term(
        integer_type,
        right,
        |_| unreachable!("{} operand types were checked together", leaf_label),
        OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
    )?;
    Ok((mathematical_left, mathematical_right))
}

fn integer_leaf_math_term(
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
    let mathematical_value = integer_leaf_math_term(
        value_type,
        value,
        |actual| OperationSemanticError::ExactShiftLeftValueTypeMismatch {
            declared: value_type,
            actual,
        },
        OperationSemanticError::ExactShiftLeftRequiresValueOrLiteralOperand,
    )?;
    let mathematical_count = integer_leaf_math_term(
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
