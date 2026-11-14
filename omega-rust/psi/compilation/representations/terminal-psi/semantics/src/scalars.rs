/*
Semantic judgments for scalar leaf evaluation, integer/float arithmetic and comparison contracts,
and scalar representation views.
*/

pub(crate) mod proof_bearing_scalar {

    use numerics::{
        arithmetic::ArithmeticDomain,
        integer_policy::{
            IntegerFormationCondition, IntegerPolicyBridge, IntegerPolicyPrimitive,
            integer_policy_bridge,
        },
    };
    use semantic_vocabulary::{
        IntegerType, ObligationId, Proposition, ScalarTerm, ScalarType, ValueId,
    };
    use terminal_psi::{Operation, OperationKind};

    use crate::{
        OperationSemanticError, ScalarLeafCrashPolicy, ScalarLeafDenotation, ScalarLeafFactShape,
        ScalarLeafFrontierPolicy, ScalarLeafGoalShape, ScalarLeafOperandShape,
        ScalarLeafResultShape, ScalarLeafSchema, ScalarLeafWorkPolicy, ValueTypes,
    };

    pub(crate) mod canonical_goal {

        use semantic_vocabulary::{
            IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType,
            IntegerValue, Proposition, ScalarTerm, ScalarType,
        };

        use crate::{OperationSemanticError, ScalarLeafGoalShape};

        use super::ProofBearingScalarLeafSchema;
        use crate::semantic_rows::OperationSemanticTag;
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
                IntegerMathTerm::literal(integer_type.minimum_value()),
                difference.clone(),
            ));
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
            let mathematical_left = integer_leaf_math_term(
                left_type,
                left,
                |_| mismatch(),
                OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
            )?;
            let mathematical_right = integer_leaf_math_term(
                right_type,
                right,
                |_| mismatch(),
                OperationSemanticError::ExactArithmeticRequiresValueOrLiteralOperand,
            )?;
            Ok([
                (left_type, mathematical_left),
                (right_type, mathematical_right),
            ])
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
                IntegerMathLiteral::from_integer_value(source_type.minimum_value());
            let source_maximum =
                IntegerMathLiteral::from_integer_value(source_type.maximum_value());
            let target_minimum =
                IntegerMathLiteral::from_integer_value(target_type.minimum_value());
            let target_maximum =
                IntegerMathLiteral::from_integer_value(target_type.maximum_value());
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

        pub(crate) fn canonical_goal(
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
    }
    use crate::scalars::scalar_leaf_semantics::integer_type;
    use crate::scalars::scalar_leaf_semantics::value_term;
    use crate::semantic_rows::OperationSemanticTag;
    pub use canonical_goal::CanonicalScalarGoal;
    use canonical_goal::canonical_goal;

    pub type ProofBearingScalarLeafSchema = ScalarLeafSchema;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProofBearingIntegerPolicyBinding {
        primitive: IntegerPolicyPrimitive,
        policy: ArithmeticDomain,
    }

    impl ProofBearingIntegerPolicyBinding {
        pub const fn primitive(self) -> IntegerPolicyPrimitive {
            self.primitive
        }

        pub const fn policy(self) -> ArithmeticDomain {
            self.policy
        }

        pub const fn bridge(self) -> IntegerPolicyBridge {
            integer_policy_bridge(self.primitive, self.policy)
        }
    }

    pub const fn proof_bearing_integer_policy_binding(
        tag: OperationSemanticTag,
    ) -> Option<ProofBearingIntegerPolicyBinding> {
        let (primitive, policy) = match tag {
            OperationSemanticTag::ExactIntegerAdd => {
                (IntegerPolicyPrimitive::Add, ArithmeticDomain::Exact)
            }
            OperationSemanticTag::ExactIntegerSubtract => {
                (IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact)
            }
            OperationSemanticTag::ExactIntegerMultiply => {
                (IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Exact)
            }
            OperationSemanticTag::ExactIntegerDivide => {
                (IntegerPolicyPrimitive::Divide, ArithmeticDomain::Exact)
            }
            OperationSemanticTag::WrappingIntegerDivide => {
                (IntegerPolicyPrimitive::Divide, ArithmeticDomain::Wrapping)
            }
            OperationSemanticTag::SaturatingIntegerDivide => {
                (IntegerPolicyPrimitive::Divide, ArithmeticDomain::Saturating)
            }
            OperationSemanticTag::ExactIntegerRemainder => {
                (IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Exact)
            }
            OperationSemanticTag::WrappingIntegerRemainder => (
                IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Wrapping,
            ),
            OperationSemanticTag::SaturatingIntegerRemainder => (
                IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Saturating,
            ),
            OperationSemanticTag::ExactIntegerShiftLeft => {
                (IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Exact)
            }
            OperationSemanticTag::ExactIntegerShiftRight => {
                (IntegerPolicyPrimitive::ShiftRight, ArithmeticDomain::Exact)
            }
            _ => return None,
        };
        Some(ProofBearingIntegerPolicyBinding { primitive, policy })
    }

    fn catalog_goal_shape(
        binding: ProofBearingIntegerPolicyBinding,
    ) -> Option<ScalarLeafGoalShape> {
        let bridge = binding.bridge();
        match (binding.primitive, bridge.formation_conditions) {
            (
                IntegerPolicyPrimitive::Add
                | IntegerPolicyPrimitive::Subtract
                | IntegerPolicyPrimitive::Multiply,
                [IntegerFormationCondition::ResultRepresentable],
            ) => Some(ScalarLeafGoalShape::ExactArithmeticRepresentable),
            (
                IntegerPolicyPrimitive::Divide | IntegerPolicyPrimitive::Remainder,
                [
                    IntegerFormationCondition::NonZeroDivisor,
                    IntegerFormationCondition::ResultRepresentable,
                ],
            ) => Some(ScalarLeafGoalShape::ExactDivisionDefined),
            (
                IntegerPolicyPrimitive::Divide | IntegerPolicyPrimitive::Remainder,
                [IntegerFormationCondition::NonZeroDivisor],
            ) => Some(ScalarLeafGoalShape::NonzeroDivisor),
            (
                IntegerPolicyPrimitive::ShiftLeft,
                [
                    IntegerFormationCondition::ShiftCountWithinWidth,
                    IntegerFormationCondition::ResultRepresentable,
                ],
            ) => Some(ScalarLeafGoalShape::ExactShiftLeftRepresentable),
            (
                IntegerPolicyPrimitive::ShiftRight,
                [IntegerFormationCondition::ShiftCountWithinWidth],
            ) => Some(ScalarLeafGoalShape::ExactShiftCount),
            _ => None,
        }
    }

    const fn proof_bearing_scalar_leaf(
        operands: ScalarLeafOperandShape,
        denotation: ScalarLeafDenotation,
        goal: ScalarLeafGoalShape,
    ) -> ProofBearingScalarLeafSchema {
        ProofBearingScalarLeafSchema {
            result: ScalarLeafResultShape::DeclaredInteger,
            operands,
            denotation,
            goal,
            fact: ScalarLeafFactShape::ResultEquation,
            crash: ScalarLeafCrashPolicy::CrashesUnlessGoal,
            work: ScalarLeafWorkPolicy::ConsumeOne,
            frontier: ScalarLeafFrontierPolicy::PreserveLocal,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProofBearingScalarSemanticRow {
        tag: OperationSemanticTag,
        schema: ProofBearingScalarLeafSchema,
    }

    impl ProofBearingScalarSemanticRow {
        pub const fn tag(self) -> OperationSemanticTag {
            self.tag
        }

        pub const fn schema(self) -> ProofBearingScalarLeafSchema {
            self.schema
        }
    }

    macro_rules! proof_bearing_scalar_rows {
        ($( $tag:ident => ($operands:ident, $denotation:ident, $goal:ident) ),+ $(,)?) => {
            impl ProofBearingScalarSemanticRow {
                pub const ALL: [Self; proof_bearing_scalar_rows!(@count $( $tag )+)] = [
                    $( Self {
                        tag: OperationSemanticTag::$tag,
                        schema: proof_bearing_scalar_leaf(
                            ScalarLeafOperandShape::$operands,
                            ScalarLeafDenotation::$denotation,
                            ScalarLeafGoalShape::$goal,
                        ),
                    } ),+
                ];
            }

            crate::semantic_rows::operation_semantic_cohort! {
                rows: ProofBearingScalarSemanticRow,
                tags: PROOF_BEARING_SCALAR_TAGS,
                is_tag: is_proof_bearing_scalar_tag,
                exact_row: exact_proof_bearing_scalar_semantic_row_in,
                reject_unexpected: reject_unexpected_proof_bearing_scalar_rows,
                missing: MissingProofBearingScalarRow,
                duplicate: DuplicateProofBearingScalarRow,
                unexpected: UnexpectedProofBearingScalarRow,
                $( $tag ),+
            }
        };
        (@count $( $variant:ident )+) => {
            <[()]>::len(&[$(proof_bearing_scalar_rows!(@unit $variant)),+])
        };
        (@unit $variant:ident) => { () };
    }

    proof_bearing_scalar_rows! {
        IntegerExactCast => (ExactCastInteger, IntegerExactCast, ExactCastRepresentable),
        ExactIntegerShiftLeft => (IntegerShift, ExactIntegerShiftLeft, ExactShiftLeftRepresentable),
        ExactIntegerShiftRight => (IntegerShift, ExactIntegerShiftRight, ExactShiftCount),
        ExactIntegerAdd => (BinaryInteger, ExactIntegerAdd, ExactArithmeticRepresentable),
        ExactIntegerSubtract => (BinaryInteger, ExactIntegerSubtract, ExactArithmeticRepresentable),
        ExactIntegerMultiply => (BinaryInteger, ExactIntegerMultiply, ExactArithmeticRepresentable),
        ExactIntegerDivide => (BinaryInteger, ExactIntegerDivide, ExactDivisionDefined),
        ExactIntegerRemainder => (BinaryInteger, ExactIntegerRemainder, ExactDivisionDefined),
        WrappingIntegerDivide => (BinaryInteger, WrappingIntegerDivide, NonzeroDivisor),
        WrappingIntegerRemainder => (BinaryInteger, WrappingIntegerRemainder, NonzeroDivisor),
        SaturatingIntegerDivide => (BinaryInteger, SaturatingIntegerDivide, NonzeroDivisor),
        SaturatingIntegerRemainder => (BinaryInteger, SaturatingIntegerRemainder, NonzeroDivisor),
    }

    fn canonical_schema(tag: OperationSemanticTag) -> Option<ProofBearingScalarLeafSchema> {
        ProofBearingScalarSemanticRow::ALL
            .iter()
            .find(|row| row.tag == tag)
            .map(|row| row.schema)
    }

    pub fn validate_proof_bearing_scalar_semantic_rows(
        rows: &[ProofBearingScalarSemanticRow],
    ) -> Result<(), OperationSemanticError> {
        reject_unexpected_proof_bearing_scalar_rows(rows)?;
        for tag in PROOF_BEARING_SCALAR_TAGS {
            let row = exact_proof_bearing_scalar_semantic_row_in(tag, rows)?
                .expect("the requested tag belongs to the proof-bearing scalar cohort");
            if Some(row.schema) != canonical_schema(tag) {
                return Err(OperationSemanticError::ProofBearingScalarSchemaMismatch(
                    tag,
                ));
            }
        }
        for tag in PROOF_BEARING_SCALAR_TAGS {
            let Some(binding) = proof_bearing_integer_policy_binding(tag) else {
                continue;
            };
            let row = exact_proof_bearing_scalar_semantic_row_in(tag, rows)?
                .expect("the catalog-bound tag belongs to the proof-bearing scalar cohort");
            if catalog_goal_shape(binding) != Some(row.schema.goal) {
                return Err(OperationSemanticError::ProofBearingScalarSchemaMismatch(
                    tag,
                ));
            }
        }
        Ok(())
    }

    pub fn proof_bearing_scalar_semantic_row(
        operation: &OperationKind,
    ) -> Result<Option<&'static ProofBearingScalarSemanticRow>, OperationSemanticError> {
        validate_proof_bearing_scalar_semantic_rows(&ProofBearingScalarSemanticRow::ALL)?;
        exact_proof_bearing_scalar_semantic_row_in(
            OperationSemanticTag::for_operation(operation),
            &ProofBearingScalarSemanticRow::ALL,
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProofBearingScalarLeafSemantics {
        tag: OperationSemanticTag,
        schema: ProofBearingScalarLeafSchema,
        obligation: ObligationId,
        canonical_goal: CanonicalScalarGoal,
        denotation: ScalarTerm,
        result_equation: Proposition,
    }

    impl ProofBearingScalarLeafSemantics {
        pub const fn tag(&self) -> OperationSemanticTag {
            self.tag
        }

        pub const fn schema(&self) -> ProofBearingScalarLeafSchema {
            self.schema
        }

        pub const fn obligation(&self) -> ObligationId {
            self.obligation
        }

        pub const fn canonical_goal(&self) -> &CanonicalScalarGoal {
            &self.canonical_goal
        }

        pub const fn denotation(&self) -> &ScalarTerm {
            &self.denotation
        }

        pub const fn result_equation(&self) -> &Proposition {
            &self.result_equation
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProofBearingInputs {
        Unary(ValueId),
        Binary(ValueId, ValueId),
    }

    fn proof_bearing_inputs(
        operation: &OperationKind,
    ) -> Option<(ProofBearingInputs, ObligationId)> {
        match operation {
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } => Some((ProofBearingInputs::Unary(*operand), *obligation)),
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            }
            | OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } => Some((ProofBearingInputs::Binary(*value, *count), *obligation)),
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            }
            | OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation,
            }
            | OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation,
            }
            | OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation,
            }
            | OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation,
            } => Some((ProofBearingInputs::Binary(*left, *right), *obligation)),
            _ => None,
        }
    }

    fn require_integer_type(
        tag: OperationSemanticTag,
        scalar_type: ScalarType,
    ) -> Result<IntegerType, OperationSemanticError> {
        match scalar_type {
            ScalarType::Integer(integer_type) => Ok(integer_type),
            ScalarType::Boolean | ScalarType::IeeeFloat(_) | ScalarType::Callable(_) => {
                Err(OperationSemanticError::OperandShapeMismatch(tag))
            }
        }
    }

    fn build_denotation(
        tag: OperationSemanticTag,
        schema: ProofBearingScalarLeafSchema,
        inputs: ProofBearingInputs,
        result_type: IntegerType,
        value_types: &dyn ValueTypes,
    ) -> Result<(ScalarTerm, Option<IntegerType>, Vec<ScalarTerm>), OperationSemanticError> {
        let invalid = || OperationSemanticError::DenotationShapeMismatch(tag);
        let built = match (schema.operands, schema.denotation, inputs) {
            (
                ScalarLeafOperandShape::ExactCastInteger,
                ScalarLeafDenotation::IntegerExactCast,
                ProofBearingInputs::Unary(operand),
            ) => {
                let operand = value_term(operand, value_types)?;
                let source_type = integer_type(&operand).ok_or_else(invalid)?;
                let denotation =
                    ScalarTerm::integer_exact_cast(source_type, result_type, operand.clone())
                        .map_err(OperationSemanticError::InvalidProposition)?;
                (denotation, Some(source_type), vec![operand])
            }
            (
                ScalarLeafOperandShape::IntegerShift,
                ScalarLeafDenotation::ExactIntegerShiftLeft,
                ProofBearingInputs::Binary(value, count),
            )
            | (
                ScalarLeafOperandShape::IntegerShift,
                ScalarLeafDenotation::ExactIntegerShiftRight,
                ProofBearingInputs::Binary(value, count),
            ) => {
                let value = value_term(value, value_types)?;
                let count = value_term(count, value_types)?;
                if integer_type(&value) != Some(result_type) {
                    return Err(OperationSemanticError::OperandShapeMismatch(tag));
                }
                let count_type = integer_type(&count).ok_or_else(invalid)?;
                let denotation = match schema.denotation {
                    ScalarLeafDenotation::ExactIntegerShiftLeft => {
                        ScalarTerm::exact_integer_shift_left(
                            result_type,
                            count_type,
                            value.clone(),
                            count.clone(),
                        )
                    }
                    ScalarLeafDenotation::ExactIntegerShiftRight => {
                        ScalarTerm::exact_integer_shift_right(
                            result_type,
                            count_type,
                            value.clone(),
                            count.clone(),
                        )
                    }
                    _ => unreachable!("matched exact-shift denotation"),
                }
                .map_err(OperationSemanticError::InvalidProposition)?;
                (denotation, Some(count_type), vec![value, count])
            }
            (
                ScalarLeafOperandShape::BinaryInteger,
                denotation,
                ProofBearingInputs::Binary(left, right),
            ) => {
                let left = value_term(left, value_types)?;
                let right = value_term(right, value_types)?;
                let operands_match = if denotation == ScalarLeafDenotation::ExactIntegerAdd {
                    matches!((integer_type(&left), integer_type(&right)), (Some(left), Some(right))
                        if result_type.admits_exact_add_operands(left, right))
                } else if denotation == ScalarLeafDenotation::ExactIntegerSubtract {
                    matches!((integer_type(&left), integer_type(&right)), (Some(left), Some(right))
                        if result_type.admits_exact_subtract_operands(left, right))
                } else {
                    integer_type(&left) == Some(result_type)
                        && integer_type(&right) == Some(result_type)
                };
                if !operands_match {
                    return Err(OperationSemanticError::OperandShapeMismatch(tag));
                }
                let constructor = match denotation {
                    ScalarLeafDenotation::ExactIntegerAdd => ScalarTerm::exact_integer_add,
                    ScalarLeafDenotation::ExactIntegerSubtract => {
                        ScalarTerm::exact_integer_subtract
                    }
                    ScalarLeafDenotation::ExactIntegerMultiply => {
                        ScalarTerm::exact_integer_multiply
                    }
                    ScalarLeafDenotation::ExactIntegerDivide => ScalarTerm::exact_integer_divide,
                    ScalarLeafDenotation::ExactIntegerRemainder => {
                        ScalarTerm::exact_integer_remainder
                    }
                    ScalarLeafDenotation::WrappingIntegerDivide => {
                        ScalarTerm::wrapping_integer_divide
                    }
                    ScalarLeafDenotation::WrappingIntegerRemainder => {
                        ScalarTerm::wrapping_integer_remainder
                    }
                    ScalarLeafDenotation::SaturatingIntegerDivide => {
                        ScalarTerm::saturating_integer_divide
                    }
                    ScalarLeafDenotation::SaturatingIntegerRemainder => {
                        ScalarTerm::saturating_integer_remainder
                    }
                    _ => return Err(invalid()),
                };
                let built = constructor(result_type, left.clone(), right.clone())
                    .map_err(OperationSemanticError::InvalidProposition)?;
                (built, None, vec![left, right])
            }
            _ => return Err(OperationSemanticError::OperandShapeMismatch(tag)),
        };
        Ok(built)
    }

    pub fn proof_bearing_scalar_leaf_semantics(
        operation: &Operation,
        value_types: &dyn ValueTypes,
    ) -> Result<Option<ProofBearingScalarLeafSemantics>, OperationSemanticError> {
        let Some(row) = proof_bearing_scalar_semantic_row(&operation.kind)? else {
            return Ok(None);
        };
        let result = operation
            .result
            .scalar_ref()
            .ok_or(OperationSemanticError::MissingScalarResult(row.tag))?;
        let result_type = require_integer_type(row.tag, result.scalar_type)?;
        let (inputs, obligation) = proof_bearing_inputs(&operation.kind)
            .ok_or(OperationSemanticError::OperandShapeMismatch(row.tag))?;
        let (denotation, auxiliary_type, inputs) =
            build_denotation(row.tag, row.schema, inputs, result_type, value_types)?;
        let canonical_goal = canonical_goal(
            row.tag,
            row.schema,
            result_type,
            auxiliary_type,
            &inputs,
            denotation.clone(),
        )?;
        if canonical_goal.shape() != row.schema.goal
            || row.schema.fact != ScalarLeafFactShape::ResultEquation
            || row.schema.crash != ScalarLeafCrashPolicy::CrashesUnlessGoal
            || row.schema.work != ScalarLeafWorkPolicy::ConsumeOne
            || row.schema.frontier != ScalarLeafFrontierPolicy::PreserveLocal
        {
            return Err(OperationSemanticError::ProofBearingScalarSchemaMismatch(
                row.tag,
            ));
        }
        Ok(Some(ProofBearingScalarLeafSemantics {
            tag: row.tag,
            schema: row.schema,
            obligation,
            canonical_goal,
            denotation: denotation.clone(),
            result_equation: Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                denotation,
            ),
        }))
    }
}
pub(crate) mod scalar_array {
    use semantic_vocabulary::{ScalarType, StructuralTypeId};
    use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

    pub fn scalar_array_leaf_path<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        mut ordinal: u64,
    ) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
        let (_, count) = scalar_array_leaf_declaration(types.clone(), root)?;
        if ordinal >= count {
            return None;
        }
        let mut dimensions = Vec::new();
        let mut current = root;
        loop {
            match *crate::structural_shape(types.clone(), current)? {
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    dimensions.push(length);
                    current = element;
                }
                StructuralTypeShape::PrimitiveScalar(_) => break,
                _ => return None,
            }
        }
        let mut path = Vec::with_capacity(dimensions.len());
        for length in dimensions.into_iter().rev() {
            path.push(terminal_psi::StructuralPathSegment::FixedIndex(
                ordinal.checked_rem(length)?,
            ));
            ordinal = ordinal.checked_div(length)?;
        }
        path.reverse();
        Some(path)
    }

    pub fn scalar_array_leaf_shape<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Option<(ScalarType, u64)> {
        scalar_array_leaf_declaration(types, root).map(|(declaration, count)| {
            let StructuralTypeShape::PrimitiveScalar(scalar) = declaration.shape else {
                unreachable!()
            };
            (scalar, count)
        })
    }

    pub fn scalar_array_leaf_declaration<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Option<(&'types StructuralTypeDeclaration, u64)> {
        let mut current = root;
        let mut leaf_count = Some(1_u64);
        let mut empty = false;
        let mut array = false;
        for _ in 0..types.clone().count() {
            let declaration = crate::structural_declaration(types.clone(), current)?;
            match declaration.shape {
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    array = true;
                    empty |= length == 0;
                    leaf_count = leaf_count.and_then(|count| count.checked_mul(length));
                    current = element;
                }
                StructuralTypeShape::PrimitiveScalar(_) if array => {
                    return Some((declaration, if empty { 0 } else { leaf_count? }));
                }
                _ => return None,
            }
        }
        None
    }
}

pub(crate) mod scalar_field_requirements {

    use proof_admission::check_predicate_denotations_with_value_equalities;
    use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};
    use terminal_psi::StructuralFieldType;

    pub fn scalar_field_requires_evidence(field_type: &StructuralFieldType) -> bool {
        matches!(
            field_type,
            StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_)
        )
    }

    pub fn scalar_field_requires_initialization_evidence(field: &StructuralFieldType) -> bool {
        scalar_field_requires_evidence(field)
            || matches!(field, StructuralFieldType::AtomicBoundedInteger(_))
    }

    pub fn scalar_field_initialization_requirement(
        field: &StructuralFieldType,
        value: ValueId,
    ) -> Option<Proposition> {
        scalar_field_requirement(field, value).or_else(|| atomic_resident_requirement(field, value))
    }

    pub fn scalar_field_requirement(
        field_type: &StructuralFieldType,
        value: ValueId,
    ) -> Option<Proposition> {
        if let StructuralFieldType::RefinedScalar(refinement) = field_type {
            return Some(refinement.instantiate(value));
        }
        let StructuralFieldType::BoundedInteger(bounds) = field_type else {
            return None;
        };
        Some(integer_range_requirement(*bounds, value))
    }

    pub fn atomic_resident_requirement(
        field: &StructuralFieldType,
        value: ValueId,
    ) -> Option<Proposition> {
        let StructuralFieldType::AtomicBoundedInteger(bounds) = field else {
            return None;
        };
        Some(integer_range_requirement(*bounds, value))
    }

    fn integer_range_requirement(
        bounds: semantic_vocabulary::BoundedIntegerType,
        value: ValueId,
    ) -> Proposition {
        let integer_type = bounds.integer_type();
        let subject = ScalarTerm::value(value, ScalarType::Integer(integer_type));
        let endpoint = |value| ScalarTerm::Integer {
            scalar_type: integer_type,
            value,
        };
        let mut clauses = vec![
            Proposition::LessOrEqual(endpoint(bounds.minimum()), subject.clone()),
            Proposition::LessOrEqual(subject, endpoint(bounds.maximum())),
        ];
        clauses.sort();
        Proposition::Conjunction(clauses)
    }

    pub fn scalar_field_accepts_literal(
        field_type: &StructuralFieldType,
        literal: &ScalarTerm,
    ) -> bool {
        if !matches!(
            literal,
            ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } | ScalarTerm::IeeeFloatLiteral(_)
        ) || field_type.scalar_type() != Some(literal.scalar_type())
        {
            return false;
        }
        let Ok(context) =
            PropositionContext::from_value_types([(ValueId::FIRST, literal.scalar_type())])
        else {
            return false;
        };
        let equation = Proposition::Equal(
            ScalarTerm::value(ValueId::FIRST, literal.scalar_type()),
            literal.clone(),
        );
        if context.validate(&equation).is_err() {
            return false;
        }
        let Some(requirement) = scalar_field_requirement(field_type, ValueId::FIRST)
            .or_else(|| atomic_resident_requirement(field_type, ValueId::FIRST))
        else {
            return true;
        };
        check_predicate_denotations_with_value_equalities(&context, &requirement, &[], &[equation])
            .is_ok_and(|checked| *checked.goal() == Proposition::Truth)
    }

    pub fn scalar_field_accepts_zero(field_type: &StructuralFieldType) -> bool {
        use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};
        let Some(carrier) = field_type.scalar_type() else {
            return false;
        };
        let zero = match carrier {
            ScalarType::Callable(_) => return false,
            ScalarType::Boolean => ScalarTerm::Boolean(false),
            ScalarType::Integer(integer) => ScalarTerm::Integer {
                scalar_type: integer,
                value: integer.zero(),
            },
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => {
                ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0))
            }
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => {
                ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0))
            }
        };
        scalar_field_accepts_literal(field_type, &zero)
    }

    pub fn scalar_field_entails(
        established: &StructuralFieldType,
        required: &StructuralFieldType,
    ) -> bool {
        if established == required {
            return true;
        }
        let Some(carrier) = established
            .scalar_type()
            .filter(|carrier| Some(*carrier) == required.scalar_type())
        else {
            return false;
        };
        let Some(goal) = scalar_field_requirement(required, ValueId::FIRST) else {
            return true;
        };
        let Ok(context) = PropositionContext::from_value_types([(ValueId::FIRST, carrier)]) else {
            return false;
        };
        let premises = scalar_field_requirement(established, ValueId::FIRST)
            .into_iter()
            .collect::<Vec<_>>();
        let Ok(checked) =
            check_predicate_denotations_with_value_equalities(&context, &goal, &premises, &[])
        else {
            return false;
        };
        crate::descriptors::descriptor_results::prove_payload_predicate(
            &context,
            checked.goal(),
            checked.requirements(),
            0,
        )
        .is_some_and(|proof| checked.check_certificate(&context, &proof).is_ok())
    }
}
pub(crate) mod scalar_leaf_schema {

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafResultShape {
        DeclaredInteger,
        Boolean,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafOperandShape {
        IntegerLiteral,
        BooleanLiteral,
        UnaryBoolean,
        BinaryBoolean,
        UnaryInteger,
        BinaryInteger,
        WideningInteger,
        ExactCastInteger,
        IntegerShift,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafDenotation {
        IntegerConstant,
        BooleanConstant,
        BooleanNot,
        BooleanEqual,
        IntegerEqual,
        IntegerLessThan,
        IntegerLessOrEqual,
        IntegerBitwiseNot,
        IntegerWiden,
        IntegerExactCast,
        IntegerBitwiseAnd,
        IntegerBitwiseOr,
        IntegerBitwiseXor,
        WrappingIntegerShiftLeft,
        WrappingIntegerShiftRight,
        ExactIntegerShiftLeft,
        ExactIntegerShiftRight,
        ExactIntegerAdd,
        ExactIntegerSubtract,
        ExactIntegerMultiply,
        ExactIntegerDivide,
        ExactIntegerRemainder,
        WrappingIntegerDivide,
        WrappingIntegerRemainder,
        SaturatingIntegerDivide,
        SaturatingIntegerRemainder,
        WrappingIntegerAdd,
        SaturatingIntegerAdd,
        WrappingIntegerSubtract,
        SaturatingIntegerSubtract,
        WrappingIntegerMultiply,
        SaturatingIntegerMultiply,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafGoalShape {
        None,
        ExactCastRepresentable,
        ExactShiftCount,
        ExactShiftLeftRepresentable,
        ExactArithmeticRepresentable,
        ExactDivisionDefined,
        NonzeroDivisor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafFactShape {
        ResultEquation,
        BooleanResultEquationAndPolarityImplications,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafCrashPolicy {
        Never,
        CrashesUnlessGoal,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafWorkPolicy {
        ConsumeOne,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarLeafFrontierPolicy {
        PreserveLocal,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ScalarLeafSchema {
        pub(crate) result: ScalarLeafResultShape,
        pub(crate) operands: ScalarLeafOperandShape,
        pub(crate) denotation: ScalarLeafDenotation,
        pub(crate) goal: ScalarLeafGoalShape,
        pub(crate) fact: ScalarLeafFactShape,
        pub(crate) crash: ScalarLeafCrashPolicy,
        pub(crate) work: ScalarLeafWorkPolicy,
        pub(crate) frontier: ScalarLeafFrontierPolicy,
    }

    impl ScalarLeafSchema {
        pub const fn result(self) -> ScalarLeafResultShape {
            self.result
        }

        pub const fn operands(self) -> ScalarLeafOperandShape {
            self.operands
        }

        pub const fn denotation(self) -> ScalarLeafDenotation {
            self.denotation
        }

        pub const fn goal(self) -> ScalarLeafGoalShape {
            self.goal
        }

        pub const fn fact(self) -> ScalarLeafFactShape {
            self.fact
        }

        pub const fn crash(self) -> ScalarLeafCrashPolicy {
            self.crash
        }

        pub const fn work(self) -> ScalarLeafWorkPolicy {
            self.work
        }

        pub const fn frontier(self) -> ScalarLeafFrontierPolicy {
            self.frontier
        }
    }

    pub type GoalFreeScalarLeafSchema = ScalarLeafSchema;

    pub(crate) const fn goal_free_scalar_leaf(
        result: ScalarLeafResultShape,
        operands: ScalarLeafOperandShape,
        denotation: ScalarLeafDenotation,
    ) -> Option<ScalarLeafSchema> {
        Some(ScalarLeafSchema {
            result,
            operands,
            denotation,
            goal: ScalarLeafGoalShape::None,
            fact: match (result, denotation) {
                (_, ScalarLeafDenotation::BooleanConstant) => ScalarLeafFactShape::ResultEquation,
                (ScalarLeafResultShape::Boolean, _) => {
                    ScalarLeafFactShape::BooleanResultEquationAndPolarityImplications
                }
                (ScalarLeafResultShape::DeclaredInteger, _) => ScalarLeafFactShape::ResultEquation,
            },
            crash: ScalarLeafCrashPolicy::Never,
            work: ScalarLeafWorkPolicy::ConsumeOne,
            frontier: ScalarLeafFrontierPolicy::PreserveLocal,
        })
    }
}
pub(crate) mod scalar_leaf_semantics {

    use crate::semantic_rows::OperationSemanticTag;
    use crate::{
        GoalFreeScalarLeafSchema, OperationSemanticError, ScalarLeafDenotation,
        ScalarLeafFactShape, ScalarLeafOperandShape, ScalarLeafResultShape, operation_semantic_row,
    };
    use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};
    use std::cmp::Ordering;
    use std::collections::BTreeMap;
    use terminal_psi::{Operation, OperationKind};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ScalarLeafLiteral {
        Integer(IntegerValue),
        Boolean(bool),
    }

    /// The scalar type of each value the scalar-leaf semantics reads. A
    /// caller that holds value types in a dense table passes the table; it
    /// need not build a map keyed by value identity to ask.
    pub trait ValueTypes {
        fn value_type(&self, value: ValueId) -> Option<ScalarType>;
    }

    impl ValueTypes for BTreeMap<ValueId, ScalarType> {
        fn value_type(&self, value: ValueId) -> Option<ScalarType> {
            self.get(&value).copied()
        }
    }

    impl ValueTypes for semantic_vocabulary::IdTable<ValueId, ScalarType> {
        fn value_type(&self, value: ValueId) -> Option<ScalarType> {
            self.get(value).copied()
        }
    }

    /// Value types read from declarations whose identities need not be
    /// dense, sorted once and searched by binary search.
    pub struct SortedValueTypes(Vec<(ValueId, ScalarType)>);

    impl SortedValueTypes {
        /// `None` when one value is declared twice.
        pub fn new(mut rows: Vec<(ValueId, ScalarType)>) -> Option<Self> {
            rows.sort_unstable_by_key(|(value, _)| *value);
            if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return None;
            }
            Some(Self(rows))
        }
    }

    impl ValueTypes for SortedValueTypes {
        fn value_type(&self, value: ValueId) -> Option<ScalarType> {
            self.0
                .binary_search_by_key(&value, |(row, _)| *row)
                .ok()
                .map(|position| self.0[position].1)
        }
    }

    pub fn constant_goal_free_scalar_leaf(
        operation: &Operation,
        literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
        value_types: &BTreeMap<ValueId, ScalarType>,
    ) -> Option<ScalarLeafLiteral> {
        let row = operation_semantic_row(&operation.kind).ok()?;
        let schema = row.goal_free_scalar_leaf()?;
        let inputs = scalar_leaf_inputs(&operation.kind)?;
        let result = operation.result.scalar_ref()?;
        validate_result_shape(row.tag, schema.result, result.scalar_type).ok()?;
        validate_operand_shape(row.tag, schema, inputs, result.scalar_type, value_types).ok()?;
        let integer_literal = |value: ValueId| match literals.get(&value) {
            Some(ScalarLeafLiteral::Integer(literal)) => Some(*literal),
            _ => None,
        };
        let boolean_literal = |value: ValueId| match literals.get(&value) {
            Some(ScalarLeafLiteral::Boolean(literal)) => Some(*literal),
            _ => None,
        };
        let integer_type_of = |value: ValueId| match value_types.get(&value) {
            Some(ScalarType::Integer(integer_type)) => Some(*integer_type),
            _ => None,
        };
        let result_integer_type = || match result.scalar_type {
            ScalarType::Integer(integer_type) => Some(integer_type),
            _ => None,
        };
        let binary_integer = |left: ValueId, right: ValueId| {
            Some((
                integer_type_of(left)?,
                integer_literal(left)?,
                integer_type_of(right)?,
                integer_literal(right)?,
            ))
        };
        macro_rules! integer_binary {
            ($op:ident, $left:ident, $right:ident) => {
                ScalarLeafLiteral::Integer(
                    result_integer_type()?
                        .$op(integer_literal($left)?, integer_literal($right)?)?,
                )
            };
        }
        Some(match (schema.denotation, inputs) {
            (ScalarLeafDenotation::BooleanNot, ScalarLeafInputs::Unary(operand)) => {
                ScalarLeafLiteral::Boolean(!boolean_literal(operand)?)
            }
            (ScalarLeafDenotation::BooleanEqual, ScalarLeafInputs::Binary(left, right)) => {
                ScalarLeafLiteral::Boolean(boolean_literal(left)? == boolean_literal(right)?)
            }
            (ScalarLeafDenotation::IntegerEqual, ScalarLeafInputs::Binary(left, right)) => {
                let (left_type, left, right_type, right) = binary_integer(left, right)?;
                ScalarLeafLiteral::Boolean(
                    left_type.compare_with(left, right_type, right)? == Ordering::Equal,
                )
            }
            (ScalarLeafDenotation::IntegerLessThan, ScalarLeafInputs::Binary(left, right)) => {
                let (left_type, left, right_type, right) = binary_integer(left, right)?;
                ScalarLeafLiteral::Boolean(
                    left_type.compare_with(left, right_type, right)? == Ordering::Less,
                )
            }
            (ScalarLeafDenotation::IntegerLessOrEqual, ScalarLeafInputs::Binary(left, right)) => {
                let (left_type, left, right_type, right) = binary_integer(left, right)?;
                ScalarLeafLiteral::Boolean(
                    left_type.compare_with(left, right_type, right)? != Ordering::Greater,
                )
            }
            (ScalarLeafDenotation::IntegerBitwiseNot, ScalarLeafInputs::Unary(operand)) => {
                ScalarLeafLiteral::Integer(
                    result_integer_type()?.bitwise_not(integer_literal(operand)?)?,
                )
            }
            (ScalarLeafDenotation::IntegerWiden, ScalarLeafInputs::Unary(operand)) => {
                ScalarLeafLiteral::Integer(
                    integer_type_of(operand)?
                        .widen_value_to(result_integer_type()?, integer_literal(operand)?)?,
                )
            }
            (ScalarLeafDenotation::IntegerBitwiseAnd, ScalarLeafInputs::Binary(left, right)) => {
                integer_binary!(bitwise_and, left, right)
            }
            (ScalarLeafDenotation::IntegerBitwiseOr, ScalarLeafInputs::Binary(left, right)) => {
                integer_binary!(bitwise_or, left, right)
            }
            (ScalarLeafDenotation::IntegerBitwiseXor, ScalarLeafInputs::Binary(left, right)) => {
                integer_binary!(bitwise_xor, left, right)
            }
            (
                ScalarLeafDenotation::WrappingIntegerShiftLeft,
                ScalarLeafInputs::Binary(value, count),
            ) => ScalarLeafLiteral::Integer(result_integer_type()?.wrapping_shift_left(
                integer_literal(value)?,
                integer_type_of(count)?,
                integer_literal(count)?,
            )?),
            (
                ScalarLeafDenotation::WrappingIntegerShiftRight,
                ScalarLeafInputs::Binary(value, count),
            ) => ScalarLeafLiteral::Integer(result_integer_type()?.wrapping_shift_right(
                integer_literal(value)?,
                integer_type_of(count)?,
                integer_literal(count)?,
            )?),
            (ScalarLeafDenotation::WrappingIntegerAdd, ScalarLeafInputs::Binary(left, right)) => {
                integer_binary!(wrapping_add, left, right)
            }
            (ScalarLeafDenotation::SaturatingIntegerAdd, ScalarLeafInputs::Binary(left, right)) => {
                integer_binary!(saturating_add, left, right)
            }
            (
                ScalarLeafDenotation::WrappingIntegerSubtract,
                ScalarLeafInputs::Binary(left, right),
            ) => {
                integer_binary!(wrapping_sub, left, right)
            }
            (
                ScalarLeafDenotation::SaturatingIntegerSubtract,
                ScalarLeafInputs::Binary(left, right),
            ) => integer_binary!(saturating_sub, left, right),
            (
                ScalarLeafDenotation::WrappingIntegerMultiply,
                ScalarLeafInputs::Binary(left, right),
            ) => {
                integer_binary!(wrapping_mul, left, right)
            }
            (
                ScalarLeafDenotation::SaturatingIntegerMultiply,
                ScalarLeafInputs::Binary(left, right),
            ) => integer_binary!(saturating_mul, left, right),
            _ => return None,
        })
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ScalarLeafInputs {
        IntegerLiteral(IntegerValue),
        BooleanLiteral(bool),
        Unary(ValueId),
        Binary(ValueId, ValueId),
    }

    fn scalar_leaf_inputs(operation: &OperationKind) -> Option<ScalarLeafInputs> {
        match operation {
            OperationKind::IntegerConstant { value } => {
                Some(ScalarLeafInputs::IntegerLiteral(*value))
            }
            OperationKind::BooleanConstant { value } => {
                Some(ScalarLeafInputs::BooleanLiteral(*value))
            }
            OperationKind::BooleanNot { operand }
            | OperationKind::IntegerBitwiseNot { operand }
            | OperationKind::IntegerWiden { operand } => Some(ScalarLeafInputs::Unary(*operand)),
            OperationKind::BooleanEqual { left, right }
            | OperationKind::IntegerEqual { left, right }
            | terminal_psi::integer_less_or_equal_pair_pattern!(left, right, ..)
            | OperationKind::IntegerBitwiseAnd { left, right }
            | OperationKind::IntegerBitwiseOr { left, right }
            | OperationKind::IntegerBitwiseXor { left, right }
            | OperationKind::WrappingIntegerShiftLeft {
                value: left,
                count: right,
            }
            | OperationKind::WrappingIntegerShiftRight {
                value: left,
                count: right,
            }
            | OperationKind::WrappingIntegerAdd { left, right }
            | OperationKind::SaturatingIntegerAdd { left, right }
            | OperationKind::WrappingIntegerSubtract { left, right }
            | OperationKind::SaturatingIntegerSubtract { left, right }
            | OperationKind::WrappingIntegerMultiply { left, right }
            | OperationKind::SaturatingIntegerMultiply { left, right } => {
                Some(ScalarLeafInputs::Binary(*left, *right))
            }
            _ => None,
        }
    }

    pub(crate) fn value_term(
        value: ValueId,
        value_types: &dyn ValueTypes,
    ) -> Result<ScalarTerm, OperationSemanticError> {
        value_types
            .value_type(value)
            .map(|scalar_type| ScalarTerm::value(value, scalar_type))
            .ok_or(OperationSemanticError::UnknownValue(value))
    }

    pub(crate) fn integer_type(term: &ScalarTerm) -> Option<semantic_vocabulary::IntegerType> {
        match term.scalar_type() {
            ScalarType::Integer(integer_type) => Some(integer_type),
            ScalarType::Boolean | ScalarType::IeeeFloat(_) | ScalarType::Callable(_) => None,
        }
    }

    fn validate_result_shape(
        tag: OperationSemanticTag,
        shape: ScalarLeafResultShape,
        actual: ScalarType,
    ) -> Result<(), OperationSemanticError> {
        let valid = match shape {
            ScalarLeafResultShape::DeclaredInteger => matches!(actual, ScalarType::Integer(_)),
            ScalarLeafResultShape::Boolean => actual == ScalarType::Boolean,
        };
        valid
            .then_some(())
            .ok_or(OperationSemanticError::ResultShapeMismatch(tag))
    }

    fn validate_operand_shape(
        tag: OperationSemanticTag,
        schema: GoalFreeScalarLeafSchema,
        inputs: ScalarLeafInputs,
        result_type: ScalarType,
        value_types: &dyn ValueTypes,
    ) -> Result<(), OperationSemanticError> {
        let type_of = |value: ValueId| {
            value_types
                .value_type(value)
                .ok_or(OperationSemanticError::UnknownValue(value))
        };
        let valid = match (schema.operands, inputs) {
            (ScalarLeafOperandShape::IntegerLiteral, ScalarLeafInputs::IntegerLiteral(_))
            | (ScalarLeafOperandShape::BooleanLiteral, ScalarLeafInputs::BooleanLiteral(_)) => true,
            (ScalarLeafOperandShape::UnaryBoolean, ScalarLeafInputs::Unary(operand)) => {
                type_of(operand)? == ScalarType::Boolean
            }
            (ScalarLeafOperandShape::BinaryBoolean, ScalarLeafInputs::Binary(left, right)) => {
                type_of(left)? == ScalarType::Boolean && type_of(right)? == ScalarType::Boolean
            }
            (ScalarLeafOperandShape::UnaryInteger, ScalarLeafInputs::Unary(operand)) => {
                matches!(result_type, ScalarType::Integer(_)) && type_of(operand)? == result_type
            }
            (ScalarLeafOperandShape::BinaryInteger, ScalarLeafInputs::Binary(left, right)) => {
                let left_type = type_of(left)?;
                let right_type = type_of(right)?;
                if result_type == ScalarType::Boolean {
                    matches!((left_type, right_type),
                        (ScalarType::Integer(left), ScalarType::Integer(right))
                            if left == right || (!left.is_address() && !right.is_address()))
                } else {
                    matches!(left_type, ScalarType::Integer(_))
                        && left_type == right_type
                        && result_type == left_type
                }
            }
            (ScalarLeafOperandShape::WideningInteger, ScalarLeafInputs::Unary(operand)) => {
                let (ScalarType::Integer(source), ScalarType::Integer(target)) =
                    (type_of(operand)?, result_type)
                else {
                    return Err(OperationSemanticError::OperandShapeMismatch(tag));
                };
                source.can_widen_to(target)
            }
            (ScalarLeafOperandShape::IntegerShift, ScalarLeafInputs::Binary(value, count)) => {
                type_of(value)? == result_type
                    && matches!(result_type, ScalarType::Integer(_))
                    && matches!(type_of(count)?, ScalarType::Integer(_))
            }
            _ => false,
        };
        valid
            .then_some(())
            .ok_or(OperationSemanticError::OperandShapeMismatch(tag))
    }

    fn denotation_term(
        tag: OperationSemanticTag,
        schema: GoalFreeScalarLeafSchema,
        inputs: ScalarLeafInputs,
        result_type: ScalarType,
        value_types: &dyn ValueTypes,
    ) -> Result<ScalarTerm, OperationSemanticError> {
        let term = |value| value_term(value, value_types);
        let invalid = || OperationSemanticError::DenotationShapeMismatch(tag);
        let built = match (schema.denotation, inputs) {
            (ScalarLeafDenotation::IntegerConstant, ScalarLeafInputs::IntegerLiteral(value)) => {
                let Some(integer_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                ScalarTerm::integer(integer_type, value)
            }
            (ScalarLeafDenotation::BooleanConstant, ScalarLeafInputs::BooleanLiteral(value)) => {
                return Ok(ScalarTerm::boolean(value));
            }
            (ScalarLeafDenotation::BooleanNot, ScalarLeafInputs::Unary(operand)) => {
                ScalarTerm::boolean_not(term(operand)?)
            }
            (ScalarLeafDenotation::BooleanEqual, ScalarLeafInputs::Binary(left, right)) => {
                ScalarTerm::boolean_equal(term(left)?, term(right)?)
            }
            (ScalarLeafDenotation::IntegerEqual, ScalarLeafInputs::Binary(left, right)) => {
                let left = term(left)?;
                let integer_type = integer_type(&left).ok_or_else(invalid)?;
                ScalarTerm::integer_equal(integer_type, left, term(right)?)
            }
            (ScalarLeafDenotation::IntegerLessThan, ScalarLeafInputs::Binary(left, right)) => {
                let left = term(left)?;
                let integer_type = integer_type(&left).ok_or_else(invalid)?;
                ScalarTerm::integer_less_than(integer_type, left, term(right)?)
            }
            (ScalarLeafDenotation::IntegerLessOrEqual, ScalarLeafInputs::Binary(left, right)) => {
                let left = term(left)?;
                let integer_type = integer_type(&left).ok_or_else(invalid)?;
                ScalarTerm::integer_less_or_equal(integer_type, left, term(right)?)
            }
            (ScalarLeafDenotation::IntegerBitwiseNot, ScalarLeafInputs::Unary(operand)) => {
                let Some(integer_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                ScalarTerm::integer_bitwise_not(integer_type, term(operand)?)
            }
            (ScalarLeafDenotation::IntegerWiden, ScalarLeafInputs::Unary(operand)) => {
                let operand = term(operand)?;
                let source_type = integer_type(&operand).ok_or_else(invalid)?;
                let Some(target_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                ScalarTerm::integer_widen(source_type, target_type, operand)
            }
            (ScalarLeafDenotation::IntegerBitwiseAnd, ScalarLeafInputs::Binary(left, right)) => {
                let Some(integer_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                ScalarTerm::integer_bitwise_and(integer_type, term(left)?, term(right)?)
            }
            (ScalarLeafDenotation::IntegerBitwiseOr, ScalarLeafInputs::Binary(left, right)) => {
                let Some(integer_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                ScalarTerm::integer_bitwise_or(integer_type, term(left)?, term(right)?)
            }
            (ScalarLeafDenotation::IntegerBitwiseXor, ScalarLeafInputs::Binary(left, right)) => {
                let Some(integer_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                ScalarTerm::integer_bitwise_xor(integer_type, term(left)?, term(right)?)
            }
            (
                ScalarLeafDenotation::WrappingIntegerShiftLeft,
                ScalarLeafInputs::Binary(value, count),
            ) => {
                let Some(value_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                let count = term(count)?;
                let count_type = integer_type(&count).ok_or_else(invalid)?;
                ScalarTerm::wrapping_integer_shift_left(value_type, count_type, term(value)?, count)
            }
            (
                ScalarLeafDenotation::WrappingIntegerShiftRight,
                ScalarLeafInputs::Binary(value, count),
            ) => {
                let Some(value_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                let count = term(count)?;
                let count_type = integer_type(&count).ok_or_else(invalid)?;
                ScalarTerm::wrapping_integer_shift_right(
                    value_type,
                    count_type,
                    term(value)?,
                    count,
                )
            }
            (denotation, ScalarLeafInputs::Binary(left, right)) => {
                let Some(integer_type) = result_type.as_integer() else {
                    return Err(invalid());
                };
                let left = term(left)?;
                let right = term(right)?;
                match denotation {
                    ScalarLeafDenotation::WrappingIntegerAdd => {
                        ScalarTerm::wrapping_integer_add(integer_type, left, right)
                    }
                    ScalarLeafDenotation::SaturatingIntegerAdd => {
                        ScalarTerm::saturating_integer_add(integer_type, left, right)
                    }
                    ScalarLeafDenotation::WrappingIntegerSubtract => {
                        ScalarTerm::wrapping_integer_subtract(integer_type, left, right)
                    }
                    ScalarLeafDenotation::SaturatingIntegerSubtract => {
                        ScalarTerm::saturating_integer_subtract(integer_type, left, right)
                    }
                    ScalarLeafDenotation::WrappingIntegerMultiply => {
                        ScalarTerm::wrapping_integer_multiply(integer_type, left, right)
                    }
                    ScalarLeafDenotation::SaturatingIntegerMultiply => {
                        ScalarTerm::saturating_integer_multiply(integer_type, left, right)
                    }
                    _ => return Err(invalid()),
                }
            }
            _ => return Err(invalid()),
        };
        built.map_err(OperationSemanticError::InvalidProposition)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct GoalFreeScalarLeafSemantics {
        result_equation: Proposition,
        fact_shape: ScalarLeafFactShape,
    }

    impl GoalFreeScalarLeafSemantics {
        pub fn result_equation(&self) -> &Proposition {
            &self.result_equation
        }

        pub fn fact_shape(&self) -> ScalarLeafFactShape {
            self.fact_shape
        }
    }

    pub fn goal_free_scalar_leaf_semantics(
        operation: &Operation,
        value_types: &dyn ValueTypes,
    ) -> Result<Option<GoalFreeScalarLeafSemantics>, OperationSemanticError> {
        let row = operation_semantic_row(&operation.kind)?;
        let Some(schema) = row.goal_free_scalar_leaf else {
            return Ok(None);
        };
        let inputs = scalar_leaf_inputs(&operation.kind)
            .ok_or(OperationSemanticError::OperandShapeMismatch(row.tag))?;
        let result = operation
            .result
            .scalar_ref()
            .ok_or(OperationSemanticError::MissingScalarResult(row.tag))?;
        validate_result_shape(row.tag, schema.result, result.scalar_type)?;
        validate_operand_shape(row.tag, schema, inputs, result.scalar_type, value_types)?;
        let denotation = denotation_term(row.tag, schema, inputs, result.scalar_type, value_types)?;
        Ok(Some(GoalFreeScalarLeafSemantics {
            result_equation: Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                denotation,
            ),
            fact_shape: schema.fact(),
        }))
    }
}
pub(crate) mod scalar_observations {

    #[cfg(test)]
    mod tests {
        use super::*;
        use terminal_psi::TerminalIdentityIndex;

        #[test]
        fn descriptor_extents_require_exact_readable_crossings() {
            let bytes = StructuralTypeId::new(1).unwrap();
            let array = StructuralTypeId::new(2).unwrap();
            let reference = StructuralTypeId::new(3).unwrap();
            let declaration = |id, shape| StructuralTypeDeclaration {
                requires: Vec::new(),
                id,
                identity: TerminalIdentityIndex(id.get() as u32),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: false,
                descriptor_cleanup: true,
                shape,
                scalar_restriction: None,
            };
            let mut types = [
                declaration(
                    bytes,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::MutableBorrow),
                    }),
                ),
                declaration(
                    array,
                    StructuralTypeShape::FixedArray {
                        element: bytes,
                        length: 1.into(),
                    },
                ),
                declaration(
                    reference,
                    StructuralTypeShape::Reference {
                        referent: array,
                        access: StructuralAccess::MutableBorrow,
                    },
                ),
            ];
            let path = vec![
                ScalarReadProjection::Referent,
                ScalarReadProjection::Static(Static::FixedIndex(0)),
                ScalarReadProjection::Extent,
            ];
            let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
            assert_eq!(
                structural_read_scalar_type(types.iter(), reference, &path),
                Some(count)
            );
            assert_eq!(
                structural_read_scalar_type(types.iter(), reference, &path[1..]),
                None
            );
            assert_eq!(
                structural_read_scalar_type(types.iter(), array, &[ScalarReadProjection::Extent]),
                None
            );
            let mut outside = path.clone();
            outside[1] = ScalarReadProjection::Static(Static::FixedIndex(1));
            assert_eq!(
                structural_read_scalar_type(types.iter(), reference, &outside),
                None
            );
            let mut continued = path.clone();
            continued.push(ScalarReadProjection::Extent);
            assert_eq!(
                structural_read_scalar_type(types.iter(), reference, &continued),
                None
            );
            types[2].shape = StructuralTypeShape::Reference {
                referent: array,
                access: StructuralAccess::WriteOnlyBorrow,
            };
            assert_eq!(
                structural_read_scalar_type(types.iter(), reference, &path),
                None
            );
            types[2].shape = StructuralTypeShape::Reference {
                referent: array,
                access: StructuralAccess::MutableBorrow,
            };
            types[0].shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::WriteOnlyBorrow),
            });
            assert_eq!(
                structural_read_scalar_type(types.iter(), reference, &path),
                None
            );
        }
    }

    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment as Static, IntegerSign, IntegerType, ScalarReadProjection,
        ScalarType, StructuralTypeId,
    };
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    #[derive(Clone, Copy)]
    enum Target {
        Declaration(StructuralTypeId),
        Bytes(ByteSequenceCarrier),
        Scalar(ScalarType),
    }

    pub fn structural_read_scalar_type<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[ScalarReadProjection],
    ) -> Option<ScalarType> {
        let declaration = |identity| {
            let mut matches = types.clone().filter(|candidate| candidate.id == identity);
            let found = matches.next()?;
            matches.next().is_none().then_some(found)
        };
        let mut target = Target::Declaration(root);
        let mut case_fields: Option<&[StructuralFieldDeclaration]> = None;
        for projection in path {
            match projection {
                ScalarReadProjection::Static(Static::Referent) => return None,
                ScalarReadProjection::Extent => {
                    if case_fields.is_some() {
                        return None;
                    }
                    let carrier = match target {
                        Target::Declaration(root) => match declaration(root)?.shape {
                            StructuralTypeShape::ElementView { access, .. } => {
                                if access == Some(StructuralAccess::WriteOnlyBorrow) {
                                    return None;
                                }
                                None
                            }
                            StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                            _ => return None,
                        },
                        Target::Bytes(carrier) => Some(carrier),
                        Target::Scalar(_) => return None,
                    };
                    if matches!(
                        carrier,
                        Some(
                            ByteSequenceCarrier::BorrowedView {
                                access: Some(StructuralAccess::WriteOnlyBorrow)
                            } | ByteSequenceCarrier::BoundedView {
                                access: Some(StructuralAccess::WriteOnlyBorrow),
                                ..
                            }
                        )
                    ) {
                        return None;
                    }
                    target = Target::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).ok()?,
                    ));
                }
                ScalarReadProjection::Referent => {
                    if case_fields.is_some() {
                        return None;
                    }
                    let Target::Declaration(root) = target else {
                        return None;
                    };
                    let StructuralTypeShape::Reference { referent, access } =
                        declaration(root)?.shape
                    else {
                        return None;
                    };
                    if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
                        return None;
                    }
                    target = Target::Declaration(referent);
                }
                ScalarReadProjection::Static(Static::Case(identity)) => {
                    if case_fields.is_some() {
                        return None;
                    }
                    let Target::Declaration(root) = target else {
                        return None;
                    };
                    let (StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. }) = &declaration(root)?.shape
                    else {
                        return None;
                    };
                    case_fields =
                        Some(&exactly_one(cases.iter(), |case| case.id == *identity)?.fields);
                }
                ScalarReadProjection::Static(Static::Field(identity)) => {
                    let Target::Declaration(root) = target else {
                        return None;
                    };
                    let fields = match case_fields.take() {
                        Some(fields) => fields,
                        None => match &declaration(root)?.shape {
                            terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                            _ => return None,
                        },
                    };
                    let field = exactly_one(fields.iter(), |field| field.id == *identity)?;
                    if field.relevance.is_erased() {
                        return None;
                    }
                    target = match &field.field_type {
                        StructuralFieldType::Structural(child) => Target::Declaration(*child),
                        StructuralFieldType::ByteSequence(carrier)
                        | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                            Target::Bytes(*carrier)
                        }
                        StructuralFieldType::Atomic(_)
                        | StructuralFieldType::AtomicBoundedInteger(_)
                        | StructuralFieldType::Erased { .. } => {
                            return None;
                        }
                        other => Target::Scalar(other.scalar_type()?),
                    };
                }
                ScalarReadProjection::Static(Static::FixedIndex(_))
                | ScalarReadProjection::Index(_) => {
                    if case_fields.is_some() {
                        return None;
                    }
                    let fixed = match projection {
                        ScalarReadProjection::Static(Static::FixedIndex(index)) => Some(*index),
                        ScalarReadProjection::Index(index) => {
                            index.validate().ok()?;
                            if index.scalar_type().as_integer()?.is_address() {
                                return None;
                            }
                            match index.integer_value() {
                                Some((_, semantic_vocabulary::IntegerValue::Signed(value))) => {
                                    Some(u64::try_from(value).ok()?)
                                }
                                Some((_, semantic_vocabulary::IntegerValue::Unsigned(value))) => {
                                    Some(u64::try_from(value).ok()?)
                                }
                                None => None,
                            }
                        }
                        _ => unreachable!(),
                    };
                    if let Target::Declaration(root) = target {
                        target = match &declaration(root)?.shape {
                            StructuralTypeShape::FixedArray {
                                element,
                                length: terminal_psi::ArrayExtent::Literal(length),
                            } => {
                                if *length == 0 || fixed.is_some_and(|index| index >= *length) {
                                    return None;
                                }
                                Target::Declaration(*element)
                            }
                            StructuralTypeShape::ElementView { element, .. } => {
                                Target::Declaration(*element)
                            }
                            StructuralTypeShape::ByteSequence(carrier) => Target::Bytes(*carrier),
                            _ => return None,
                        };
                        if !matches!(target, Target::Bytes(_)) {
                            continue;
                        }
                    }
                    let Target::Bytes(carrier) = target else {
                        return None;
                    };
                    match carrier {
                        ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::WriteOnlyBorrow),
                        }
                        | ByteSequenceCarrier::BoundedView {
                            access: Some(StructuralAccess::WriteOnlyBorrow),
                            ..
                        } => return None,
                        ByteSequenceCarrier::BoundedView { capacity, .. }
                        | ByteSequenceCarrier::BoundedOwned { capacity }
                            if capacity == 0 || fixed.is_some_and(|index| index >= capacity) =>
                        {
                            return None;
                        }
                        _ => {}
                    }
                    target = Target::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).ok()?,
                    ));
                }
            }
        }
        if case_fields.is_some() {
            return None;
        }
        match target {
            Target::Scalar(scalar) => Some(scalar),
            Target::Declaration(root) => match declaration(root)?.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                _ => None,
            },
            Target::Bytes(_) => None,
        }
    }
}
pub mod scalar_ordering {
    use semantic_vocabulary::{
        BlockId, IntegerSign, IntegerType, PlaceId, Proposition, ScalarType, ValueId,
    };

    pub(crate) mod contracts {

        use super::{Relation, Term};
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
        };

        fn index(term: &ScalarTerm) -> Option<Term> {
            let integer = IntegerType::new(IntegerSign::Unsigned, 64).ok()?;
            match term {
                ScalarTerm::Value { id, scalar_type }
                    if *scalar_type == ScalarType::Integer(integer) =>
                {
                    Some(Term::Value(*id))
                }
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                } if *scalar_type == integer => u64::try_from(*value).ok().map(Term::Unsigned),
                _ => None,
            }
        }

        pub(crate) fn relations(propositions: &[Proposition]) -> Vec<Relation> {
            let mut pending = propositions.iter().collect::<Vec<_>>();
            let mut result = Vec::new();
            while let Some(proposition) = pending.pop() {
                let (left, right, strict, equality) = match proposition {
                    Proposition::Conjunction(children) => {
                        pending.extend(children);
                        continue;
                    }
                    Proposition::Equal(left, right) => (left, right, false, true),
                    Proposition::LessThan(left, right) => (left, right, true, false),
                    Proposition::LessOrEqual(left, right) => (left, right, false, false),
                    _ => continue,
                };
                let (Some(left), Some(right)) = (index(left), index(right)) else {
                    continue;
                };
                result.push(Relation {
                    left,
                    right,
                    strict,
                });
                if equality {
                    result.push(Relation {
                        left: right,
                        right: left,
                        strict: false,
                    });
                }
            }
            result
        }

        pub fn instantiate_contract_ordering(
            proposition: &Proposition,
            bindings: &[(semantic_vocabulary::ValueId, ScalarTerm)],
        ) -> Vec<Proposition> {
            let integer =
                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a valid integer type");
            let instantiate = |term| -> Option<ScalarTerm> {
                Some(match term {
                    Term::Unsigned(value) => ScalarTerm::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(u128::from(value)),
                    },
                    Term::Value(formal) => {
                        let (_, actual) =
                            exactly_one(bindings.iter(), |(value, _)| *value == formal)?;
                        index(actual)?;
                        actual.clone()
                    }
                })
            };
            relations(std::slice::from_ref(proposition))
                .into_iter()
                .filter_map(|relation| {
                    let left = instantiate(relation.left)?;
                    let right = instantiate(relation.right)?;
                    Some(if relation.strict {
                        Proposition::LessThan(left, right)
                    } else {
                        Proposition::LessOrEqual(left, right)
                    })
                })
                .collect()
        }
    }
    pub use contracts::instantiate_contract_ordering;
    use std::collections::VecDeque;

    pub type Position = (BlockId, u32);

    #[derive(Clone, Copy, Debug, Default)]
    pub enum Effect {
        #[default]
        None,
        Compare {
            result: ValueId,
            left: ValueId,
            right: ValueId,
            strict: bool,
        },
        Not {
            result: ValueId,
            operand: ValueId,
        },
        UnsignedLiteral {
            result: ValueId,
            value: u128,
        },
        EstablishStorage {
            place: PlaceId,
            value: ValueId,
        },
        StoreStorage {
            place: PlaceId,
            value: ValueId,
        },
        ReadStorage {
            place: PlaceId,
            result: ValueId,
        },
        ForgetStorage,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Binding {
        pub parameter: ValueId,
        pub argument: ValueId,
        pub scalar_type: ScalarType,
    }

    #[derive(Clone, Debug)]
    pub struct Edge {
        pub target: usize,
        pub enters_block: bool,
        pub parameters: Vec<(ValueId, ScalarType)>,
        pub bindings: Vec<Binding>,
        pub condition: Option<(ValueId, bool)>,
    }

    #[derive(Clone, Debug)]
    pub struct Step {
        pub position: Position,
        pub definitions: Vec<(ValueId, ScalarType)>,
        pub effect: Effect,
        pub successors: Vec<Edge>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum Term {
        Value(ValueId),
        Unsigned(u64),
    }

    impl Term {
        fn mentions(self, values: &[ValueId]) -> bool {
            matches!(self, Self::Value(value) if values.contains(&value))
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct Relation {
        left: Term,
        right: Term,
        strict: bool,
    }

    impl Relation {
        fn negated(self) -> Self {
            Self {
                left: self.right,
                right: self.left,
                strict: !self.strict,
            }
        }
    }

    #[derive(Clone, Default, PartialEq, Eq)]
    struct State {
        relations: Vec<Relation>,
        comparisons: Vec<(ValueId, Relation)>,
        types: Vec<(ValueId, ScalarType)>,
        literals: Vec<(ValueId, u128)>,
        storage: Vec<(PlaceId, Option<u64>)>,
    }

    impl State {
        fn known_unsigned(&self, value: ValueId) -> Option<u64> {
            if !self.is_index(value) {
                return None;
            }
            let mut candidates = self
                .literals
                .iter()
                .filter_map(|(_, literal)| u64::try_from(*literal).ok())
                .chain(
                    self.relations
                        .iter()
                        .flat_map(|relation| [relation.left, relation.right])
                        .filter_map(|term| match term {
                            Term::Unsigned(value) => Some(value),
                            _ => None,
                        }),
                )
                .collect::<Vec<_>>();
            candidates.sort_unstable();
            candidates.dedup();
            let mut matching = candidates.into_iter().filter(|literal| {
                self.proves(Term::Value(value), Term::Unsigned(*literal), false)
                    && self.proves(Term::Unsigned(*literal), Term::Value(value), false)
            });
            let result = matching.next()?;
            matching.next().is_none().then_some(result)
        }

        fn proves(&self, left: Term, right: Term, strict: bool) -> bool {
            let valid = |term| match term {
                Term::Value(value) => self.is_index(value),
                Term::Unsigned(_) => true,
            };
            if !valid(left) || !valid(right) {
                return false;
            }
            let mut relations = self.relations.clone();
            for (value, literal) in &self.literals {
                let Ok(literal) = u64::try_from(*literal) else {
                    continue;
                };
                for (left, right) in [
                    (Term::Value(*value), Term::Unsigned(literal)),
                    (Term::Unsigned(literal), Term::Value(*value)),
                ] {
                    relations.push(Relation {
                        left,
                        right,
                        strict: false,
                    });
                }
            }
            let constants = relations
                .iter()
                .flat_map(|relation| [relation.left, relation.right])
                .chain([left, right])
                .filter_map(|term| match term {
                    Term::Unsigned(value) => Some(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut pending = vec![(left, false)];
            let mut visited = Vec::new();
            while let Some((term, already_strict)) = pending.pop() {
                if term == right && (!strict || already_strict) {
                    return true;
                }
                if visited.contains(&(term, already_strict)) {
                    continue;
                }
                visited.push((term, already_strict));
                if let Term::Unsigned(literal) = term {
                    pending.extend(
                        constants
                            .iter()
                            .filter(|other| literal <= **other)
                            .map(|other| {
                                (Term::Unsigned(*other), already_strict || literal < *other)
                            }),
                    );
                }
                pending.extend(
                    relations
                        .iter()
                        .filter(|relation| relation.left == term)
                        .map(|relation| (relation.right, already_strict || relation.strict)),
                );
            }
            false
        }

        fn kill(&mut self, values: &[ValueId]) {
            self.relations.retain(|relation| {
                !relation.left.mentions(values) && !relation.right.mentions(values)
            });
            self.comparisons.retain(|(value, relation)| {
                !values.contains(value)
                    && !relation.left.mentions(values)
                    && !relation.right.mentions(values)
            });
            self.types.retain(|(value, _)| !values.contains(value));
            self.literals.retain(|(value, _)| !values.contains(value));
        }

        fn intersect(&mut self, other: &Self) {
            self.relations
                .retain(|relation| other.relations.contains(relation));
            self.comparisons
                .retain(|comparison| other.comparisons.contains(comparison));
            self.types.retain(|value| other.types.contains(value));
            self.literals.retain(|value| other.literals.contains(value));
            self.storage.retain_mut(|(place, value)| {
                let Some((_, incoming)) = other.storage.iter().find(|(other, _)| other == place)
                else {
                    return false;
                };
                if value != incoming {
                    *value = None;
                }
                true
            });
        }

        fn is_index(&self, value: ValueId) -> bool {
            self.types.contains(&(
                value,
                ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64)
                        .expect("u64 is a valid integer type"),
                ),
            ))
        }

        fn assume(&mut self, propositions: &[Proposition]) {
            for relation in contracts::relations(propositions) {
                let valid = |term| match term {
                    Term::Value(value) => self.is_index(value),
                    Term::Unsigned(_) => true,
                };
                if valid(relation.left) && valid(relation.right) {
                    self.relations.push(relation);
                }
            }
            self.relations.sort_unstable();
            self.relations.dedup();
        }

        fn after(&self, step: &Step) -> Self {
            let result_is = |result, scalar_type| {
                step.definitions
                    .iter()
                    .filter(|(value, _)| *value == result)
                    .count()
                    == 1
                    && step.definitions.contains(&(result, scalar_type))
            };
            let comparisons = match step.effect {
                Effect::Compare {
                    result,
                    left,
                    right,
                    strict,
                } if self.is_index(left)
                    && self.is_index(right)
                    && result_is(result, ScalarType::Boolean) =>
                {
                    vec![(
                        result,
                        Relation {
                            left: Term::Value(left),
                            right: Term::Value(right),
                            strict,
                        },
                    )]
                }
                Effect::Not { result, operand }
                    if self.types.contains(&(operand, ScalarType::Boolean))
                        && result_is(result, ScalarType::Boolean) =>
                {
                    self.comparisons
                        .iter()
                        .filter(|(value, _)| *value == operand)
                        .map(|(_, relation)| (result, relation.negated()))
                        .collect()
                }
                _ => Vec::new(),
            };
            let mut result = self.clone();
            let defined = step
                .definitions
                .iter()
                .map(|(value, _)| *value)
                .collect::<Vec<_>>();
            result.kill(&defined);
            result.types.extend_from_slice(&step.definitions);
            for (value, relation) in comparisons {
                if !relation.left.mentions(&defined) && !relation.right.mentions(&defined) {
                    result.comparisons.push((value, relation));
                }
            }
            if let Effect::UnsignedLiteral {
                result: value,
                value: literal,
            } = step.effect
                && literal <= u128::from(u64::MAX)
                && result.is_index(value)
                && step
                    .definitions
                    .iter()
                    .filter(|(defined, _)| *defined == value)
                    .count()
                    == 1
            {
                result.literals.push((value, literal));
            }
            match step.effect {
                Effect::EstablishStorage { place, value } => {
                    result.storage.retain(|(prior, _)| *prior != place);
                    result.storage.push((place, self.known_unsigned(value)));
                }
                Effect::StoreStorage { place, value } => {
                    if let Some((_, stored)) =
                        result.storage.iter_mut().find(|(prior, _)| *prior == place)
                    {
                        *stored = self.known_unsigned(value);
                    } else {
                        for (_, stored) in &mut result.storage {
                            *stored = None;
                        }
                    }
                }
                Effect::ReadStorage {
                    place,
                    result: value,
                } if result_is(
                    value,
                    ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
                    ),
                ) =>
                {
                    if let Some((_, Some(literal))) =
                        self.storage.iter().find(|(prior, _)| *prior == place)
                    {
                        result.literals.push((value, u128::from(*literal)));
                    }
                }
                Effect::ForgetStorage => {
                    for (_, stored) in &mut result.storage {
                        *stored = None;
                    }
                }
                _ => {}
            }
            result
        }

        fn across(&self, edge: &Edge) -> Self {
            let mut source = self.clone();
            if let Some((condition, selected)) = edge.condition {
                for (_, relation) in source
                    .comparisons
                    .iter()
                    .filter(|(value, _)| *value == condition)
                {
                    let relation = if selected {
                        *relation
                    } else {
                        relation.negated()
                    };
                    if !source.relations.contains(&relation) {
                        source.relations.push(relation);
                    }
                }
            }
            if !edge.enters_block {
                return source;
            }
            if edge.bindings.len() != edge.parameters.len()
                || edge.parameters.iter().any(|(parameter, scalar_type)| {
                    let bindings = edge
                        .bindings
                        .iter()
                        .filter(|binding| binding.parameter == *parameter)
                        .collect::<Vec<_>>();
                    bindings.len() != 1
                        || bindings[0].scalar_type != *scalar_type
                        || !source.types.contains(&(bindings[0].argument, *scalar_type))
                })
            {
                return Self::default();
            }
            let aliases = |value| {
                (!edge
                    .parameters
                    .iter()
                    .any(|(parameter, _)| *parameter == value))
                .then_some(value)
                .into_iter()
                .chain(edge.bindings.iter().filter_map(move |binding| {
                    (binding.argument == value).then_some(binding.parameter)
                }))
                .collect::<Vec<_>>()
            };
            let mut result = Self::default();
            for (value, scalar_type) in &source.types {
                for alias in aliases(*value) {
                    result.types.push((alias, *scalar_type));
                }
            }
            for (value, literal) in &source.literals {
                for alias in aliases(*value) {
                    result.literals.push((alias, *literal));
                }
            }
            let term_aliases = |term| match term {
                Term::Value(value) => aliases(value).into_iter().map(Term::Value).collect(),
                Term::Unsigned(_) => vec![term],
            };
            for relation in &source.relations {
                for left in term_aliases(relation.left) {
                    for right in term_aliases(relation.right) {
                        result.relations.push(Relation {
                            left,
                            right,
                            strict: relation.strict,
                        });
                    }
                }
            }
            for (value, relation) in &source.comparisons {
                for alias in aliases(*value) {
                    for left in term_aliases(relation.left) {
                        for right in term_aliases(relation.right) {
                            result.comparisons.push((
                                alias,
                                Relation {
                                    left,
                                    right,
                                    strict: relation.strict,
                                },
                            ));
                        }
                    }
                }
            }
            result.relations.sort_unstable();
            result.relations.dedup();
            result.comparisons.sort_unstable();
            result.comparisons.dedup();
            result.types.sort_unstable();
            result.types.dedup();
            result.literals.sort_unstable();
            result.literals.dedup();
            result
        }
    }

    pub struct ScalarOrdering {
        steps: Vec<Step>,
        states: Vec<Option<State>>,
    }

    impl ScalarOrdering {
        pub fn new(
            steps: Vec<Step>,
            entry: Position,
            parameters: Vec<(ValueId, ScalarType)>,
        ) -> Self {
            Self::with_contracts(steps, entry, parameters, &[], Vec::new())
        }

        pub fn with_contracts(
            steps: Vec<Step>,
            entry: Position,
            parameters: Vec<(ValueId, ScalarType)>,
            entry_requirements: &[Proposition],
            normal_returns: Vec<(Position, Vec<Proposition>)>,
        ) -> Self {
            let positions = steps.iter().map(|step| step.position).collect::<Vec<_>>();
            let mut result = Self {
                states: vec![None; steps.len()],
                steps,
            };
            if result.steps.iter().any(|step| {
                step.successors
                    .iter()
                    .any(|edge| edge.target >= result.steps.len())
            }) || positions
                .iter()
                .enumerate()
                .any(|(index, position)| positions[..index].contains(position))
            {
                return result;
            }
            let Some(entry) = positions.iter().position(|position| *position == entry) else {
                return result;
            };
            let mut reachable = vec![false; positions.len()];
            let mut pending = vec![entry];
            while let Some(index) = pending.pop() {
                if std::mem::replace(&mut reachable[index], true) {
                    continue;
                }
                pending.extend(
                    result.steps[index]
                        .successors
                        .iter()
                        .map(|edge| edge.target),
                );
            }
            let mut remaining = vec![0usize; positions.len()];
            for (_, step) in result
                .steps
                .iter()
                .enumerate()
                .filter(|(index, _)| reachable[*index])
            {
                for edge in &step.successors {
                    remaining[edge.target] += 1;
                }
            }
            let mut queue = VecDeque::new();
            let mut arrivals = vec![None::<State>; positions.len()];
            let mut initial = State {
                types: parameters,
                ..State::default()
            };
            initial.assume(entry_requirements);
            arrivals[entry] = Some(initial);
            if remaining[entry] == 0 {
                queue.push_back(entry);
            }
            while let Some(index) = queue.pop_front() {
                let state = arrivals[index].take().unwrap_or_default();
                let mut output = state.after(&result.steps[index]);
                for (_, guarantees) in normal_returns
                    .iter()
                    .filter(|(position, _)| *position == result.steps[index].position)
                {
                    output.assume(guarantees);
                }
                result.states[index] = Some(state);
                for edge in &result.steps[index].successors {
                    let arrival = output.across(edge);
                    if let Some(current) = &mut arrivals[edge.target] {
                        current.intersect(&arrival);
                    } else {
                        arrivals[edge.target] = Some(arrival);
                    }
                    remaining[edge.target] -= 1;
                    if remaining[edge.target] == 0 {
                        queue.push_back(edge.target);
                    }
                }
            }
            result
        }

        pub fn known_unsigned(&self, position: Position, value: ValueId) -> Option<u64> {
            self.steps
                .iter()
                .position(|step| step.position == position)
                .and_then(|index| self.states[index].as_ref())?
                .known_unsigned(value)
        }

        pub(crate) fn proves(
            &self,
            position: Position,
            left: Term,
            right: Term,
            strict: bool,
        ) -> bool {
            self.steps
                .iter()
                .position(|step| step.position == position)
                .and_then(|index| self.states[index].as_ref())
                .is_some_and(|state| state.proves(left, right, strict))
        }

        pub fn values_unchanged_since(
            &self,
            birth: Position,
            current: Position,
            values: &[ValueId],
        ) -> bool {
            let Some(birth) = self.steps.iter().position(|step| step.position == birth) else {
                return false;
            };
            let Some(current) = self.steps.iter().position(|step| step.position == current) else {
                return false;
            };
            if self.states[birth].is_none() || self.states[current].is_none() {
                return false;
            }
            let mut pending = vec![current];
            let mut visited = Vec::new();
            while let Some(index) = pending.pop() {
                if index == birth || visited.contains(&index) {
                    continue;
                }
                visited.push(index);
                let mut found = false;
                for (predecessor, step) in self.steps.iter().enumerate() {
                    if self.states[predecessor].is_none() {
                        continue;
                    }
                    for edge in step.successors.iter().filter(|edge| edge.target == index) {
                        found = true;
                        if step
                            .definitions
                            .iter()
                            .any(|(value, _)| values.contains(value))
                            || edge
                                .parameters
                                .iter()
                                .any(|(value, _)| values.contains(value))
                        {
                            return false;
                        }
                        pending.push(predecessor);
                    }
                }
                if !found {
                    return false;
                }
            }
            true
        }
    }
}
pub(crate) mod scalar_representation_view {

    use semantic_vocabulary::{
        IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ScalarRepresentationViewPair {
        FloatBits {
            format: IeeeFloatFormat,
            integer: IntegerType,
        },
        BitsFloat {
            integer: IntegerType,
            format: IeeeFloatFormat,
        },
        BooleanByte {
            integer: IntegerType,
        },
        ByteBoolean {
            integer: IntegerType,
        },
    }

    impl ScalarRepresentationViewPair {
        pub fn classify(source: ScalarType, target: ScalarType) -> Option<Self> {
            let fixed =
                |integer: IntegerType, bits: u16| !integer.is_address() && integer.bits() == bits;
            match (source, target) {
                (ScalarType::IeeeFloat(format), ScalarType::Integer(integer))
                    if fixed(integer, format_bits(format)) =>
                {
                    Some(Self::FloatBits { format, integer })
                }
                (ScalarType::Integer(integer), ScalarType::IeeeFloat(format))
                    if fixed(integer, format_bits(format)) =>
                {
                    Some(Self::BitsFloat { integer, format })
                }
                (ScalarType::Boolean, ScalarType::Integer(integer)) if fixed(integer, 8) => {
                    Some(Self::BooleanByte { integer })
                }
                (ScalarType::Integer(integer), ScalarType::Boolean) if fixed(integer, 8) => {
                    Some(Self::ByteBoolean { integer })
                }
                _ => None,
            }
        }

        pub const fn requires_landing(self) -> bool {
            matches!(self, Self::ByteBoolean { .. })
        }

        pub fn view(self, value: ScalarViewValue) -> Option<ScalarViewValue> {
            match (self, value) {
                (Self::FloatBits { format, integer }, ScalarViewValue::IeeeFloat(float))
                    if float.format() == format =>
                {
                    let bits = match float {
                        IeeeFloatValue::Binary32(bits) => u64::from(bits),
                        IeeeFloatValue::Binary64(bits) => bits,
                    };
                    Some(ScalarViewValue::Integer(integer_from_bits(integer, bits)))
                }
                (Self::BitsFloat { integer, format }, ScalarViewValue::Integer(value)) => {
                    let bits = integer_bits(integer, value)?;
                    Some(ScalarViewValue::IeeeFloat(match format {
                        IeeeFloatFormat::Binary32 => {
                            IeeeFloatValue::Binary32(u32::try_from(bits).ok()?)
                        }
                        IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(bits),
                    }))
                }
                (Self::BooleanByte { integer }, ScalarViewValue::Boolean(value)) => Some(
                    ScalarViewValue::Integer(integer_from_bits(integer, u64::from(value))),
                ),
                (Self::ByteBoolean { integer }, ScalarViewValue::Integer(value)) => {
                    match integer_bits(integer, value)? {
                        0 => Some(ScalarViewValue::Boolean(false)),
                        1 => Some(ScalarViewValue::Boolean(true)),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ScalarViewValue {
        Boolean(bool),
        Integer(IntegerValue),
        IeeeFloat(IeeeFloatValue),
    }

    const fn format_bits(format: IeeeFloatFormat) -> u16 {
        match format {
            IeeeFloatFormat::Binary32 => 32,
            IeeeFloatFormat::Binary64 => 64,
        }
    }

    pub(crate) fn integer_bits(integer: IntegerType, value: IntegerValue) -> Option<u64> {
        if !integer.admits(value) || integer.bits() > 64 {
            return None;
        }
        let mask = if integer.bits() == 64 {
            u64::MAX
        } else {
            (1u64 << integer.bits()) - 1
        };
        Some(match value {
            IntegerValue::Signed(value) => (value as u64) & mask,
            IntegerValue::Unsigned(value) => u64::try_from(value).ok()? & mask,
        })
    }

    pub(crate) fn integer_from_bits(integer: IntegerType, bits: u64) -> IntegerValue {
        let width = u32::from(integer.bits());
        match integer.sign() {
            IntegerSign::Unsigned => {
                let mask = if width == 64 {
                    u64::MAX
                } else {
                    (1u64 << width) - 1
                };
                IntegerValue::Unsigned(u128::from(bits & mask))
            }
            IntegerSign::Signed => {
                let shift = 64 - width;
                IntegerValue::Signed(i128::from(((bits << shift) as i64) >> shift))
            }
        }
    }
}
pub(crate) mod unchanged_scalar {

    use semantic_vocabulary::ValueId;

    pub enum UnchangedScalarOrigin<Identity> {
        Known(Identity),
        Incoming(Vec<ValueId>),
    }

    pub fn unchanged_scalar_identity<Identity: Clone + Eq>(
        value: ValueId,
        mut origin: impl FnMut(ValueId) -> Option<UnchangedScalarOrigin<Identity>>,
    ) -> Option<Identity> {
        fn resolve<Identity: Clone + Eq>(
            value: ValueId,
            origin: &mut impl FnMut(ValueId) -> Option<UnchangedScalarOrigin<Identity>>,
            active: &mut Vec<ValueId>,
            completed: &mut Vec<(ValueId, Option<Identity>)>,
        ) -> Option<Identity> {
            if let Some((_, result)) = completed.iter().find(|(known, _)| *known == value) {
                return result.clone();
            }
            if active.contains(&value) {
                return None;
            }
            active.push(value);
            let result = (|| match origin(value)? {
                UnchangedScalarOrigin::Known(identity) => Some(identity),
                UnchangedScalarOrigin::Incoming(values) => {
                    let mut values = values.into_iter();
                    let first = resolve(values.next()?, origin, active, completed)?;
                    for value in values {
                        if resolve(value, origin, active, completed)? != first {
                            return None;
                        }
                    }
                    Some(first)
                }
            })();
            active.pop();
            completed.push((value, result.clone()));
            result
        }
        resolve(value, &mut origin, &mut Vec::new(), &mut Vec::new())
    }
}
