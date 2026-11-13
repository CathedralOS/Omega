/*
Semantic judgments for scalar leaf evaluation, integer/float arithmetic and comparison contracts,
proof-bearing scalar elision, and scalar representation views.
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
    pub(crate) mod elision {

        use std::collections::BTreeMap;

        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition, ScalarTerm,
            ScalarType, ValueId,
        };
        use terminal_psi::{Operation, OperationKind};

        use super::{
            CanonicalScalarGoal, ProofBearingScalarLeafSemantics,
            proof_bearing_scalar_leaf_semantics,
        };
        use crate::{OperationSemanticError, ScalarLeafLiteral};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ProofBearingScalarLeafElision {
            semantics: ProofBearingScalarLeafSemantics,
            replacement: OperationKind,
            result_literal: Option<IntegerValue>,
        }

        impl ProofBearingScalarLeafElision {
            pub const fn semantics(&self) -> &ProofBearingScalarLeafSemantics {
                &self.semantics
            }

            pub const fn obligation(&self) -> ObligationId {
                self.semantics.obligation()
            }

            pub const fn replacement(&self) -> &OperationKind {
                &self.replacement
            }

            pub const fn result_literal(&self) -> Option<IntegerValue> {
                self.result_literal
            }
        }

        pub fn elidable_proof_bearing_scalar_leaf(
            operation: &Operation,
            literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
            value_types: &BTreeMap<ValueId, ScalarType>,
        ) -> Result<Option<ProofBearingScalarLeafElision>, OperationSemanticError> {
            let Some(semantics) = proof_bearing_scalar_leaf_semantics(operation, value_types)?
            else {
                return Ok(None);
            };
            let Some(result_type) = operation
                .result
                .scalar_ref()
                .expect("proof-bearing semantics requires a scalar result")
                .scalar_type
                .as_integer()
            else {
                unreachable!("proof-bearing scalar results are declared integers")
            };
            if !canonical_goal_is_discharged(semantics.canonical_goal(), literals) {
                return Ok(None);
            }
            if let Some(value) = discharged_literal(operation, &semantics, literals, result_type) {
                return Ok(Some(ProofBearingScalarLeafElision {
                    semantics,
                    replacement: OperationKind::IntegerConstant { value },
                    result_literal: Some(value),
                }));
            }
            let replacement = match &operation.kind {
                OperationKind::ExactIntegerShiftLeft { value, count, .. } => {
                    OperationKind::WrappingIntegerShiftLeft {
                        value: *value,
                        count: *count,
                    }
                }
                OperationKind::ExactIntegerShiftRight { value, count, .. } => {
                    OperationKind::WrappingIntegerShiftRight {
                        value: *value,
                        count: *count,
                    }
                }
                kind @ (OperationKind::ExactIntegerAdd { left, right, .. }
                | OperationKind::ExactIntegerSubtract { left, right, .. }) => {
                    if result_type.is_address()
                        || value_types.get(left) != Some(&ScalarType::Integer(result_type))
                        || value_types.get(right) != Some(&ScalarType::Integer(result_type))
                    {
                        return Ok(None);
                    }
                    if matches!(kind, OperationKind::ExactIntegerAdd { .. }) {
                        OperationKind::WrappingIntegerAdd {
                            left: *left,
                            right: *right,
                        }
                    } else {
                        OperationKind::WrappingIntegerSubtract {
                            left: *left,
                            right: *right,
                        }
                    }
                }
                OperationKind::ExactIntegerMultiply { left, right, .. } => {
                    OperationKind::WrappingIntegerMultiply {
                        left: *left,
                        right: *right,
                    }
                }
                OperationKind::IntegerExactCast { operand, .. } => {
                    let Some(ScalarType::Integer(source_type)) = value_types.get(operand).copied()
                    else {
                        return Ok(None);
                    };
                    if !source_type.can_widen_to(result_type) {
                        return Ok(None);
                    }
                    OperationKind::IntegerWiden { operand: *operand }
                }
                _ => return Ok(None),
            };
            Ok(Some(ProofBearingScalarLeafElision {
                semantics,
                replacement,
                result_literal: None,
            }))
        }

        fn canonical_goal_is_discharged(
            goal: &CanonicalScalarGoal,
            literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
        ) -> bool {
            let goal = literal_substituted_goal(goal, literals);
            match &goal {
                CanonicalScalarGoal::NonzeroDivisor { divisor, .. } => {
                    matches!(divisor.integer_value(), Some((_, value)) if !is_zero(value))
                }
                CanonicalScalarGoal::ExactDivisionDefined {
                    integer_type,
                    left,
                    right,
                } => match right.integer_value() {
                    Some((_, divisor)) if !is_zero(divisor) => {
                        integer_type.sign() == IntegerSign::Unsigned
                            || divisor != IntegerValue::Signed(-1)
                            || matches!(
                                left.integer_value(),
                                Some((_, dividend)) if dividend != integer_type.minimum_value()
                            )
                    }
                    _ => false,
                },
                _ => goal
                    .kernel_proposition()
                    .is_ok_and(|proposition| proposition == Proposition::Truth),
            }
        }

        fn discharged_literal(
            operation: &Operation,
            semantics: &ProofBearingScalarLeafSemantics,
            literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
            result_type: IntegerType,
        ) -> Option<IntegerValue> {
            if let Some((denoted_type, value)) =
                literal_scalar_term(semantics.denotation(), literals).integer_value()
                && denoted_type == result_type
            {
                return Some(value);
            }
            let zero = || result_type.zero();
            let literal_of = |value: &ValueId| match literals.get(value) {
                Some(ScalarLeafLiteral::Integer(literal)) => Some(*literal),
                _ => None,
            };
            match &operation.kind {
                OperationKind::ExactIntegerSubtract { left, right, .. } if left == right => {
                    Some(zero())
                }
                OperationKind::ExactIntegerMultiply { left, right, .. }
                    if is_zero_opt(literal_of(left)) || is_zero_opt(literal_of(right)) =>
                {
                    Some(zero())
                }
                OperationKind::ExactIntegerShiftLeft { value, .. }
                | OperationKind::ExactIntegerShiftRight { value, .. }
                    if is_zero_opt(literal_of(value)) =>
                {
                    Some(zero())
                }
                OperationKind::ExactIntegerRemainder { right, .. } if is_one(literal_of(right)) => {
                    Some(zero())
                }
                OperationKind::WrappingIntegerRemainder { right, .. }
                | OperationKind::SaturatingIntegerRemainder { right, .. }
                    if is_one(literal_of(right))
                        || (result_type.sign() == IntegerSign::Signed
                            && literal_of(right) == Some(IntegerValue::Signed(-1))) =>
                {
                    Some(zero())
                }
                _ => None,
            }
        }

        fn literal_substituted_goal(
            goal: &CanonicalScalarGoal,
            literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
        ) -> CanonicalScalarGoal {
            let substitute = |term: &ScalarTerm| literal_scalar_term(term, literals);
            match goal {
                CanonicalScalarGoal::ExactCastRepresentable {
                    source_type,
                    target_type,
                    operand,
                } => CanonicalScalarGoal::ExactCastRepresentable {
                    source_type: *source_type,
                    target_type: *target_type,
                    operand: substitute(operand),
                },
                CanonicalScalarGoal::ExactShiftCount {
                    value_type,
                    count_type,
                    count,
                } => CanonicalScalarGoal::ExactShiftCount {
                    value_type: *value_type,
                    count_type: *count_type,
                    count: substitute(count),
                },
                CanonicalScalarGoal::ExactShiftLeftRepresentable {
                    value_type,
                    count_type,
                    value,
                    count,
                } => CanonicalScalarGoal::ExactShiftLeftRepresentable {
                    value_type: *value_type,
                    count_type: *count_type,
                    value: substitute(value),
                    count: substitute(count),
                },
                CanonicalScalarGoal::ExactArithmeticRepresentable {
                    integer_type,
                    expression,
                } => CanonicalScalarGoal::ExactArithmeticRepresentable {
                    integer_type: *integer_type,
                    expression: literal_scalar_term(expression, literals),
                },
                CanonicalScalarGoal::ExactDivisionDefined {
                    integer_type,
                    left,
                    right,
                } => CanonicalScalarGoal::ExactDivisionDefined {
                    integer_type: *integer_type,
                    left: substitute(left),
                    right: substitute(right),
                },
                CanonicalScalarGoal::NonzeroDivisor {
                    integer_type,
                    divisor,
                } => CanonicalScalarGoal::NonzeroDivisor {
                    integer_type: *integer_type,
                    divisor: substitute(divisor),
                },
            }
        }

        fn literal_scalar_term(
            term: &ScalarTerm,
            literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
        ) -> ScalarTerm {
            if let ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(integer_type),
            } = term
                && let Some(ScalarLeafLiteral::Integer(value)) = literals.get(id)
                && let Ok(literal) = ScalarTerm::integer(*integer_type, *value)
            {
                return literal;
            }
            let rebuild = |left: &ScalarTerm, right: &ScalarTerm| {
                (
                    literal_scalar_term(left, literals),
                    literal_scalar_term(right, literals),
                )
            };
            macro_rules! rebuilt_binary {
                ($constructor:ident, $scalar_type:ident, $left:ident, $right:ident) => {{
                    let (left, right) = rebuild($left, $right);
                    ScalarTerm::$constructor(*$scalar_type, left, right)
                        .unwrap_or_else(|_| term.clone())
                }};
            }
            match term {
                ScalarTerm::IntegerExactCast {
                    source_type,
                    target_type,
                    operand,
                } => ScalarTerm::integer_exact_cast(
                    *source_type,
                    *target_type,
                    literal_scalar_term(operand, literals),
                )
                .unwrap_or_else(|_| term.clone()),
                ScalarTerm::ExactIntegerShiftLeft {
                    value_type,
                    count_type,
                    value,
                    count,
                } => ScalarTerm::exact_integer_shift_left(
                    *value_type,
                    *count_type,
                    literal_scalar_term(value, literals),
                    literal_scalar_term(count, literals),
                )
                .unwrap_or_else(|_| term.clone()),
                ScalarTerm::ExactIntegerShiftRight {
                    value_type,
                    count_type,
                    value,
                    count,
                } => ScalarTerm::exact_integer_shift_right(
                    *value_type,
                    *count_type,
                    literal_scalar_term(value, literals),
                    literal_scalar_term(count, literals),
                )
                .unwrap_or_else(|_| term.clone()),
                ScalarTerm::ExactIntegerAdd {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(exact_integer_add, scalar_type, left, right),
                ScalarTerm::ExactIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(exact_integer_subtract, scalar_type, left, right),
                ScalarTerm::ExactIntegerMultiply {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(exact_integer_multiply, scalar_type, left, right),
                ScalarTerm::ExactIntegerDivide {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(exact_integer_divide, scalar_type, left, right),
                ScalarTerm::ExactIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(exact_integer_remainder, scalar_type, left, right),
                ScalarTerm::WrappingIntegerDivide {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(wrapping_integer_divide, scalar_type, left, right),
                ScalarTerm::WrappingIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(wrapping_integer_remainder, scalar_type, left, right),
                ScalarTerm::SaturatingIntegerDivide {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(saturating_integer_divide, scalar_type, left, right),
                ScalarTerm::SaturatingIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                } => rebuilt_binary!(saturating_integer_remainder, scalar_type, left, right),
                _ => term.clone(),
            }
        }

        fn is_zero(value: IntegerValue) -> bool {
            matches!(value, IntegerValue::Signed(0) | IntegerValue::Unsigned(0))
        }

        fn is_zero_opt(value: Option<IntegerValue>) -> bool {
            value.is_some_and(is_zero)
        }

        fn is_one(value: Option<IntegerValue>) -> bool {
            matches!(
                value,
                Some(IntegerValue::Signed(1) | IntegerValue::Unsigned(1))
            )
        }
    }
    use crate::scalars::scalar_leaf_semantics::integer_type;
    use crate::scalars::scalar_leaf_semantics::value_term;
    use crate::semantic_rows::OperationSemanticTag;
    pub use canonical_goal::CanonicalScalarGoal;
    use canonical_goal::canonical_goal;
    pub use elision::{ProofBearingScalarLeafElision, elidable_proof_bearing_scalar_leaf};

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
pub mod scalar_demand {

    pub(crate) mod operation_inputs {

        use semantic_vocabulary::ValueId;
        use terminal_psi::OperationKind as O;

        pub(crate) fn inputs(operation: &O, values: &mut Vec<ValueId>) -> bool {
            values.extend(
                operation
                    .runtime_indexes()
                    .into_iter()
                    .map(|(index, _)| index),
            );
            match operation {
                O::ReadScalarApplication { .. }
                | O::EstablishNativeCodeLayout { .. }
                | O::ReleaseActivationClaim { .. }
                | O::MergeActivationClaim { .. } => {}
                O::EstablishActivationClaim { committed, .. }
                | O::SplitActivationClaim {
                    committed_at: committed,
                    ..
                } => values.push(*committed),
                O::ActivateTaskRegister { selector, .. }
                | O::ReloadSegmentDescriptors { selector, .. } => values.push(*selector),
                O::ReadTaskRegister { .. } | O::ReadSegmentSelector { .. } => {}
                O::RegisterAccess { event, .. } => values.extend(event.inputs()),
                O::GlobalDescriptorTable { .. }
                | O::StorageAddress { .. }
                | O::PrimitiveScalarRead { .. }
                | O::MoveStructuralValue { .. }
                | O::TransformContent { .. } => {}
                O::StructuralByteSequenceFieldByteStore {
                    index,
                    value,
                    length,
                    ..
                }
                | O::ByteSequenceWrite {
                    index,
                    value,
                    length,
                    ..
                }
                | O::ElementViewWrite {
                    index,
                    value,
                    length,
                    ..
                } => {
                    values.extend([*index, *value, *length]);
                }
                O::ByteSequenceStore { length, .. }
                | O::StructuralByteSequenceFieldStore { length, .. } => values.push(*length),
                O::EstablishByteSequenceConcatenation { left, right, .. }
                | O::ByteSequenceEqual { left, right } => {
                    values.extend([left.length(), right.length()]);
                }
                O::ByteSequencePredicate { operand, .. } => values.push(operand.length()),
                O::ByteSequenceRead { index, length, .. }
                | O::StructuralByteSequenceFieldRead { index, length, .. } => {
                    values.extend([*index, *length])
                }
                O::ByteSequenceSubslice {
                    start, end, length, ..
                }
                | O::ElementViewSubslice {
                    start, end, length, ..
                } => values.extend([*start, *end, *length]),
                O::ElementViewRead { index, length, .. } => values.extend([*index, *length]),
                O::IntegerConstant { .. }
                | O::SelectImplementation { .. }
                | O::SelectMachine { .. }
                | O::BooleanConstant { .. }
                | O::IeeeFloatConstant { .. }
                | O::ScalarStructuralField { .. }
                | O::ByteSequenceLength { .. }
                | O::StructuralByteSequenceFieldLength { .. }
                | O::StructuralByteViewFieldStore { .. }
                | O::BooleanStructuralField { .. }
                | O::StructuralCaseMembership { .. }
                | terminal_psi::move_or_store_structural_field_pattern!()
                | terminal_psi::structural_leaf_copy_pair_pattern!()
                | O::EstablishByteSequenceLiteral { .. }
                | O::ElementViewLength { .. }
                | O::EstablishTrivialAffineLocal { .. }
                | O::EstablishZeroedStructural { .. }
                | O::EstablishStructuralLocal { .. }
                | O::EstablishStructuralArray { .. }
                | O::ReleaseReference { .. }
                | O::EstablishAccessReference { .. }
                | O::PortWrite { .. }
                | O::AtomicFence { .. } => {}
                O::EstablishReference { storage, .. } => {
                    if let Some(storage) = storage {
                        values.push(storage.byte_offset);
                    }
                }
                terminal_psi::element_view_establish_pair_pattern!(region, ..) => {
                    if let terminal_psi::ElementViewRegion::RepresentationBytes {
                        start, end, ..
                    } = region
                    {
                        values.extend([*start, *end]);
                    } else if let terminal_psi::ElementViewRegion::WholeRepresentation {
                        source_length,
                        ..
                    } = region
                    {
                        values.push(*source_length);
                    }
                }
                O::EstablishScalarCase { fields, .. } => {
                    values.extend(fields.iter().map(|field| field.value));
                }
                O::EstablishRecord { fields } | O::EstablishStructuralCase { fields, .. } => {
                    values.extend(fields.iter().filter_map(|field| match field.value {
                        terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(value),
                        terminal_psi::RecordFieldValue::Structural(_) => None,
                    }));
                }
                O::EstablishScalarArray { elements } => values.extend(elements),
                O::BooleanNot { operand }
                | O::IntegerBitwiseNot { operand }
                | O::IntegerWiden { operand }
                | O::IntegerExactCast { operand, .. }
                | O::ScalarRepresentationView { operand, .. } => values.push(*operand),
                O::BooleanEqual { left, right }
                | O::IeeeFloatCompare { left, right, .. }
                | O::IntegerEqual { left, right }
                | terminal_psi::integer_less_or_equal_pair_pattern!(left, right, ..)
                | O::IntegerBitwiseAnd { left, right }
                | O::IntegerBitwiseOr { left, right }
                | O::IntegerBitwiseXor { left, right }
                | O::WrappingIntegerAdd { left, right }
                | O::SaturatingIntegerAdd { left, right }
                | O::WrappingIntegerSubtract { left, right }
                | O::SaturatingIntegerSubtract { left, right }
                | O::WrappingIntegerMultiply { left, right }
                | O::SaturatingIntegerMultiply { left, right }
                | O::ExactIntegerAdd { left, right, .. }
                | O::ExactIntegerSubtract { left, right, .. }
                | O::ExactIntegerMultiply { left, right, .. }
                | O::ExactIntegerDivide { left, right, .. }
                | O::ExactIntegerRemainder { left, right, .. }
                | O::WrappingIntegerDivide { left, right, .. }
                | O::WrappingIntegerRemainder { left, right, .. }
                | O::SaturatingIntegerDivide { left, right, .. }
                | O::SaturatingIntegerRemainder { left, right, .. } => {
                    values.extend([*left, *right])
                }
                O::WrappingIntegerShiftLeft { value, count }
                | O::WrappingIntegerShiftRight { value, count }
                | O::ExactIntegerShiftLeft { value, count, .. }
                | O::ExactIntegerShiftRight { value, count, .. } => values.extend([*value, *count]),
                O::NearestIeeeFloatFusedMultiplyAdd {
                    left,
                    right,
                    addend,
                } => values.extend([*left, *right, *addend]),
                O::EstablishPrimitiveLocal { value, .. }
                | O::WriteOnlyPrimitiveStore { value, .. }
                | O::StructuralScalarFieldStore { value, .. } => values.push(*value),
                O::BoundaryCall { arguments, .. } => values.extend(arguments),
                O::TrappingInteger { operation } => values.extend(operation.operands()),
                O::FloatResultPolicy { policy } => values.extend(policy.operands()),
                O::AtomicAccess { event, .. } => values.extend(event.operands()),
                O::Call {
                    arguments,
                    crash_continuations,
                    ..
                }
                | O::CallUnit {
                    arguments,
                    crash_continuations,
                    ..
                }
                | O::CallStructuralScalar {
                    arguments,
                    crash_continuations,
                    ..
                }
                | O::CallStructuralWithScalarArguments {
                    arguments,
                    crash_continuations,
                    ..
                } => {
                    if !crash_continuations.is_empty() {
                        return false;
                    }
                    values.extend(arguments);
                }
                O::CallMachine {
                    callee,
                    arguments,
                    crash_continuations,
                    ..
                } => {
                    if !crash_continuations.is_empty() {
                        return false;
                    }
                    values.push(*callee);
                    values.extend(arguments);
                }
                O::CallDynamic {
                    selector,
                    arguments,
                    family_actuals,
                    crash_continuations,
                    ..
                } => {
                    if !crash_continuations.is_empty() {
                        return false;
                    }
                    values.push(*selector);
                    values.extend(arguments);
                    values.extend(family_actuals);
                }
                O::CallStructural { .. } => return false,
            }
            true
        }
    }
    pub(crate) mod ranked_coordinates {
        use semantic_vocabulary::{BlockId, ValueId};
        use terminal_psi::{TerminalMachine, TerminalRankedScc};
        #[derive(Default)]
        pub(crate) struct RankedCoordinates {
            pub blocks: Vec<BlockId>,
            pub values: Vec<ValueId>,
        }
        pub(crate) fn ranked_coordinates(machine: &TerminalMachine) -> Option<RankedCoordinates> {
            let mut coverage = RankedCoordinates::default();
            let Some(ranking) = &machine.ranked_scc else {
                return Some(coverage);
            };
            let TerminalRankedScc::Natural(components) = ranking;
            if components.is_empty() {
                return None;
            }
            for component in components {
                if component.ranks.is_empty() {
                    return None;
                }
                for rank in &component.ranks {
                    coverage.blocks.push(rank.block);
                    coverage.values.extend(
                        component
                            .rank_values(rank.values)?
                            .iter()
                            .map(|value| value.value),
                    );
                }
                for edge in &component.edges {
                    coverage.blocks.extend([edge.source, edge.target]);
                    coverage.values.extend(
                        component
                            .rank_values(edge.successor_ranks)?
                            .iter()
                            .map(|value| value.value),
                    );
                }
            }
            Some(coverage)
        }
    }
    pub(crate) mod retained_rows {

        use semantic_vocabulary::{BlockId, OperationId, ValueId};
        use terminal_psi::{CrashRouteGuard, TerminalSuspensionLoanCapture};

        #[derive(Default)]
        pub struct RecordNamedRows {
            pub blocks: Vec<BlockId>,
            pub operations: Vec<OperationId>,
            pub values: Vec<ValueId>,
        }

        pub fn visit_invocation_evidence_values(
            inputs: terminal_psi::ModuleInvocationInputs<'_>,
            mut retain: impl FnMut(ValueId),
        ) {
            visit_erased_value_arguments(
                inputs.erased_arguments,
                inputs.erased_proof_arguments,
                &mut retain,
            );
            for bucket in inputs.crash_continuations {
                for alternative in &bucket.alternatives {
                    if let CrashRouteGuard::Predicate(term) = alternative {
                        term.proposition().visit_value_ids(&mut retain);
                    }
                }
            }
        }

        pub(crate) fn visit_erased_value_arguments(
            scalars: &[semantic_vocabulary::ScalarTerm],
            proofs: &[semantic_vocabulary::ProofTerm],
            mut retain: impl FnMut(ValueId),
        ) {
            for term in scalars {
                term.visit_value_ids(|value| {
                    retain(value);
                    true
                });
            }
            for term in proofs {
                term.visit_value_ids(|value| {
                    retain(value);
                    true
                });
            }
        }

        pub fn record_named_rows(module: &terminal_psi::TerminalModule) -> RecordNamedRows {
            let mut rows = RecordNamedRows::default();
            for observation in module.scalar_applications.conditions.observations() {
                if let terminal_psi::ScalarRangeObservation::EntryResult { result, .. } =
                    observation
                {
                    rows.values.push(*result);
                }
            }
            for machine in &module.machines {
                for invocation in machine.module_invocations() {
                    visit_invocation_evidence_values(invocation.inputs, |value| {
                        rows.values.push(value)
                    });
                }
            }
            rows.operations
                .extend(module.quotients.operations.iter().map(|row| row.call));
            rows.operations
                .extend(module.quotients.adaptations.iter().map(|row| row.copy));
            rows.values.extend(
                module
                    .operations()
                    .filter(|operation| {
                        module
                            .quotients
                            .operations
                            .iter()
                            .any(|row| row.call == operation.id)
                    })
                    .filter_map(|operation| operation.result.scalar().map(|value| value.id)),
            );
            rows.operations.extend(
                module
                    .boundary_application_demands
                    .iter()
                    .map(|demand| demand.terminal_operation()),
            );
            for site in &module.suspension_call_sites {
                rows.operations.push(site.operation);
            }
            for plan in &module.suspension_call_plans {
                rows.operations.push(plan.operation);
                plan.visit_value_ids(|value| {
                    rows.values.push(value);
                });
                for (_, loan) in plan.loans.occurrences.iter() {
                    match loan.capture {
                        TerminalSuspensionLoanCapture::Reference { formation, .. } => {
                            rows.operations.push(formation);
                        }
                        TerminalSuspensionLoanCapture::CompletedOutput { call, .. }
                        | TerminalSuspensionLoanCapture::CallInput { call, .. } => {
                            rows.operations.push(call);
                        }
                        _ => {}
                    }
                }
            }
            for projection in &module.float_meaning_projections {
                match &projection.source {
                    terminal_psi::FloatMeaningSource::DirectMachineParameter(row) => {
                        rows.values.push(row.parameter);
                    }
                    terminal_psi::FloatMeaningSource::DirectMachineResult(row) => {
                        rows.values.push(row.result);
                    }
                    terminal_psi::FloatMeaningSource::DirectBlockParameter(row) => {
                        rows.blocks.push(row.block);
                        rows.values.push(row.parameter);
                    }
                    terminal_psi::FloatMeaningSource::DirectOperationResult(row) => {
                        rows.operations.push(row.producer);
                        rows.values.push(row.result);
                    }
                    terminal_psi::FloatMeaningSource::DirectCallResult(row) => {
                        rows.operations.push(row.producer);
                        rows.values.push(row.result);
                    }
                    terminal_psi::FloatMeaningSource::TransitionalInput(_)
                    | terminal_psi::FloatMeaningSource::DirectStructuralLeaf(_)
                    | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
                    | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_)
                    | terminal_psi::FloatMeaningSource::SemanticApplication(_) => {}
                }
            }
            rows
        }
    }

    pub use retained_rows::{record_named_rows, visit_invocation_evidence_values};
    use semantic_vocabulary::{BlockId, ValueId};
    use terminal_psi::{TerminalMachine, Terminator};

    #[derive(Clone, Copy)]
    pub enum ScalarDemandMode {
        KeepAllOperations,
        DropUnusedTotalOperations,
    }

    #[derive(Clone, Copy)]
    enum Definition {
        MachineParameter,
        BlockParameter(usize, usize),
        Operation(usize, BlockId),
    }

    struct ScalarDemand {
        live: Vec<ValueId>,
        removed: Vec<(BlockId, Vec<usize>)>,
    }

    pub fn ranked_coordinates(machine: &TerminalMachine) -> Option<(Vec<BlockId>, Vec<ValueId>)> {
        let coordinates = ranked_coordinates::ranked_coordinates(machine)?;
        Some((coordinates.blocks, coordinates.values))
    }

    /// Blocks whose parameter lists a scalar qualification row depends on.
    ///
    /// A coercion row and a completion return row each bind one argument of a
    /// jump to the parameter that receives it. The completion row names that
    /// argument by position, so removing an earlier parameter of the receiving
    /// block would leave the row pointing at a different argument, or at none.
    /// Callers that drop or merge block parameters keep these blocks whole.
    pub fn qualification_parameter_blocks(
        module: &terminal_psi::TerminalModule,
    ) -> Option<Vec<(semantic_vocabulary::MachineId, BlockId)>> {
        let completions =
            || {
                module.scalar_qualifications.returns.iter().filter(|row| {
                    matches!(row.site, terminal_psi::ScalarReturnSite::Completion { .. })
                })
            };
        if module.scalar_qualifications.coercions.is_empty() && completions().next().is_none() {
            return Some(Vec::new());
        }
        let mut targets = Vec::new();
        for machine in &module.machines {
            for block in &machine.blocks {
                match &block.terminator {
                    Terminator::Jump { edge, target, .. } => {
                        targets.push(((machine.id, *edge), *target))
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for edge in [when_true, when_false] {
                            targets.push(((machine.id, edge.edge), edge.target));
                        }
                    }
                    _ => {}
                }
            }
        }
        targets.sort_unstable_by_key(|(key, _)| *key);
        if targets.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return None;
        }
        let mut protected = Vec::new();
        for row in &module.scalar_qualifications.coercions {
            let position = targets
                .binary_search_by_key(&(row.machine, row.edge), |(key, _)| *key)
                .ok()?;
            protected.push((row.machine, targets[position].1));
        }
        for row in completions() {
            let position = targets
                .binary_search_by_key(&(row.machine, row.edge), |(key, _)| *key)
                .ok()?;
            protected.push((row.machine, targets[position].1));
        }
        protected.sort_unstable();
        protected.dedup();
        Some(protected)
    }

    pub fn retained_values_by_machine(
        module: &terminal_psi::TerminalModule,
        values: impl IntoIterator<Item = ValueId>,
    ) -> Option<Vec<(semantic_vocabulary::MachineId, Vec<ValueId>)>> {
        let mut namespace = Vec::new();
        for machine in &module.machines {
            namespace.extend(
                machine
                    .parameters
                    .iter()
                    .map(|value| (value.id, machine.id)),
            );
            for block in &machine.blocks {
                namespace.extend(block.parameters.iter().map(|value| (value.id, machine.id)));
                namespace.extend(block.operations.iter().filter_map(|operation| {
                    operation
                        .result
                        .scalar()
                        .map(|value| (value.id, machine.id))
                }));
            }
        }
        namespace.sort_unstable_by_key(|(value, _)| *value);
        if namespace.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return None;
        }
        let mut roots = values
            .into_iter()
            .filter_map(|value| {
                namespace
                    .binary_search_by_key(&value, |(value, _)| *value)
                    .ok()
                    .map(|position| (namespace[position].1, value))
            })
            .collect::<Vec<_>>();
        roots.sort_unstable();
        roots.dedup();
        let mut result: Vec<(semantic_vocabulary::MachineId, Vec<ValueId>)> = Vec::new();
        for (machine, value) in roots {
            if let Some((previous, values)) = result.last_mut()
                && *previous == machine
            {
                values.push(value);
            } else {
                result.push((machine, vec![value]));
            }
        }
        Some(result)
    }

    pub fn complete_scalar_demand(
        machine: &mut TerminalMachine,
        retained_values: impl IntoIterator<Item = ValueId>,
        protected_blocks: impl IntoIterator<Item = BlockId>,
        mode: ScalarDemandMode,
    ) -> bool {
        let Some(demand) = ScalarDemand::prepare(machine, retained_values, protected_blocks, mode)
        else {
            return false;
        };
        if matches!(mode, ScalarDemandMode::DropUnusedTotalOperations) {
            for block in &mut machine.blocks {
                block.operations.retain(|operation| {
                    !crate::is_unconditionally_total_scalar(&operation.kind)
                        || operation
                            .result
                            .scalar()
                            .is_none_or(|result| demand.is_live(result.id))
                });
            }
        }
        demand.remove_unused_block_parameters(machine);
        true
    }

    impl ScalarDemand {
        fn prepare<'a>(
            machine: &'a TerminalMachine,
            retained_values: impl IntoIterator<Item = ValueId>,
            protected_blocks: impl IntoIterator<Item = BlockId>,
            mode: ScalarDemandMode,
        ) -> Option<Self> {
            let operations = machine.operations().collect::<Vec<_>>();
            let mut blocks = machine
                .blocks
                .iter()
                .enumerate()
                .map(|(position, block)| (block.id, position))
                .collect::<Vec<_>>();
            blocks.sort_unstable_by_key(|(id, _)| *id);
            if blocks.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return None;
            }
            let block_position = |id| {
                blocks
                    .binary_search_by_key(&id, |(id, _)| *id)
                    .ok()
                    .map(|position| blocks[position].1)
            };
            let mut successors = Vec::with_capacity(machine.blocks.len());
            for block in &machine.blocks {
                successors.push(
                    block
                        .terminator
                        .successor_targets()
                        .into_iter()
                        .map(&block_position)
                        .collect::<Option<Vec<_>>>()?,
                );
            }
            let mut visited = vec![false; machine.blocks.len()];
            let mut pending_blocks = vec![block_position(machine.entry)?];
            while let Some(block) = pending_blocks.pop() {
                if std::mem::replace(&mut visited[block], true) {
                    continue;
                }
                pending_blocks.extend(&successors[block]);
            }
            if visited.iter().any(|visited| !visited) {
                return None;
            }
            let dominators = crate::control_graph::dominators(machine);
            let operation_blocks = machine
                .blocks
                .iter()
                .flat_map(|block| std::iter::repeat_n(block.id, block.operations.len()))
                .collect::<Vec<_>>();
            let mut definitions = machine
                .parameters
                .iter()
                .map(|parameter| {
                    (
                        parameter.id,
                        Definition::MachineParameter,
                        parameter.scalar_type,
                    )
                })
                .collect::<Vec<_>>();
            for (block, body) in machine.blocks.iter().enumerate() {
                definitions.extend(body.parameters.iter().enumerate().map(
                    |(position, parameter)| {
                        (
                            parameter.id,
                            Definition::BlockParameter(block, position),
                            parameter.scalar_type,
                        )
                    },
                ));
            }
            definitions.extend(operations.iter().enumerate().filter_map(
                |(position, operation)| {
                    operation.result.scalar().map(|value| {
                        (
                            value.id,
                            Definition::Operation(position, operation_blocks[position]),
                            value.scalar_type,
                        )
                    })
                },
            ));
            definitions.sort_unstable_by_key(|(id, _, _)| *id);
            if definitions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return None;
            }

            let (ranked_blocks, rank_values) = ranked_coordinates(machine)?;
            let mut protected = vec![false; machine.blocks.len()];
            for block in protected_blocks.into_iter().chain(ranked_blocks) {
                protected[block_position(block)?] = true;
            }
            let mut incoming = vec![Vec::<&[ValueId]>::new(); machine.blocks.len()];
            let mut pending = retained_values
                .into_iter()
                .map(|value| (value, false))
                .collect::<Vec<_>>();
            pending.extend(rank_values.into_iter().map(|value| (value, true)));
            let mut inputs = Vec::new();
            for operation in &operations {
                if (matches!(mode, ScalarDemandMode::KeepAllOperations)
                    || !crate::is_unconditionally_total_scalar(&operation.kind))
                    && !operation_inputs::inputs(&operation.kind, &mut inputs)
                {
                    return None;
                }
            }
            pending.extend(inputs.drain(..).map(|value| (value, true)));
            for block in &machine.blocks {
                let mut edge = |target, arguments: &'a [ValueId]| -> Option<()> {
                    let position = block_position(target)?;
                    if arguments.len() != machine.blocks[position].parameters.len() {
                        return None;
                    }
                    for (actual, formal) in
                        arguments.iter().zip(&machine.blocks[position].parameters)
                    {
                        let source = definitions
                            .binary_search_by_key(actual, |(id, _, _)| *id)
                            .ok()?;
                        if definitions[source].2 != formal.scalar_type {
                            return None;
                        }
                        let available = match definitions[source].1 {
                            Definition::MachineParameter => true,
                            Definition::BlockParameter(owner, _) => {
                                dominators.dominates(machine.blocks[owner].id, block.id)
                            }
                            Definition::Operation(_, owner) => {
                                dominators.dominates(owner, block.id)
                            }
                        };
                        if !available {
                            return None;
                        }
                    }
                    incoming[position].push(arguments);
                    Some(())
                };
                match &block.terminator {
                    Terminator::Jump {
                        target,
                        arguments,
                        erased_arguments,
                        erased_proof_arguments,
                        ..
                    } => {
                        edge(*target, arguments)?;
                        retained_rows::visit_erased_value_arguments(
                            erased_arguments,
                            erased_proof_arguments,
                            |value| pending.push((value, true)),
                        );
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for incoming in [when_true, when_false] {
                            edge(incoming.target, &incoming.arguments)?;
                            retained_rows::visit_erased_value_arguments(
                                &incoming.erased_arguments,
                                &incoming.erased_proof_arguments,
                                |value| pending.push((value, true)),
                            );
                        }
                    }
                    Terminator::StructuralCase { cases, .. } => {
                        for case in cases {
                            protected[block_position(case.target)?] = true;
                        }
                    }
                    Terminator::Crash { .. } => return None,
                    Terminator::Return { .. }
                    | Terminator::ReturnUnit { .. }
                    | Terminator::ReturnStructural { .. }
                    | Terminator::ExternalCompletion { .. } => {}
                    Terminator::InvokeNoNormal {
                        erased_arguments,
                        erased_proof_arguments,
                        ..
                    } => {
                        retained_rows::visit_erased_value_arguments(
                            erased_arguments,
                            erased_proof_arguments,
                            |value| pending.push((value, true)),
                        );
                    }
                }
                let mut terminator = block.terminator.clone();
                match &mut terminator {
                    Terminator::Jump { arguments, .. } => arguments.clear(),
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        when_true.arguments.clear();
                        when_false.arguments.clear();
                    }
                    _ => {}
                }
                terminator.map_scalar_uses(&mut |value| {
                    pending.push((value, true));
                    value
                });
                let mut path = |path: &[terminal_psi::StructuralPathSegment]| {
                    for segment in path {
                        if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } =
                            segment
                        {
                            pending.push((*index, true));
                        }
                    }
                };
                for projection in block.terminator.storage_uses() {
                    path(projection.path);
                }
                for arguments in block.terminator.successor_block_arguments() {
                    for argument in arguments {
                        path(&argument.path);
                    }
                }
            }
            for (position, block) in machine.blocks.iter().enumerate() {
                if protected[position] || incoming[position].is_empty() {
                    pending.extend(
                        block
                            .parameters
                            .iter()
                            .map(|parameter| (parameter.id, true)),
                    );
                }
            }
            let mut live = vec![false; definitions.len()];
            while let Some((value, required)) = pending.pop() {
                let Ok(position) = definitions.binary_search_by_key(&value, |(id, _, _)| *id)
                else {
                    if required {
                        return None;
                    }
                    continue;
                };
                if std::mem::replace(&mut live[position], true) {
                    continue;
                }
                match definitions[position].1 {
                    Definition::MachineParameter => {}
                    Definition::Operation(operation, _) => {
                        if !operation_inputs::inputs(&operations[operation].kind, &mut inputs) {
                            return None;
                        }
                        pending.extend(inputs.drain(..).map(|value| (value, true)));
                    }
                    Definition::BlockParameter(block, position) => {
                        for arguments in &incoming[block] {
                            pending.push((arguments[position], true));
                        }
                    }
                }
            }
            let live = definitions
                .iter()
                .zip(live)
                .filter_map(|((id, _, _), live)| live.then_some(*id))
                .collect::<Vec<_>>();
            let mut removed = Vec::new();
            for (position, block) in machine.blocks.iter().enumerate() {
                if protected[position] || incoming[position].is_empty() {
                    continue;
                }
                let positions = block
                    .parameters
                    .iter()
                    .enumerate()
                    .filter_map(|(position, parameter)| {
                        live.binary_search(&parameter.id)
                            .is_err()
                            .then_some(position)
                    })
                    .collect::<Vec<_>>();
                if !positions.is_empty() {
                    removed.push((block.id, positions));
                }
            }
            removed.sort_unstable_by_key(|(id, _)| *id);
            Some(Self { live, removed })
        }

        fn is_live(&self, value: ValueId) -> bool {
            self.live.binary_search(&value).is_ok()
        }

        fn remove_unused_block_parameters(&self, machine: &mut TerminalMachine) {
            let positions = |block| {
                self.removed
                    .binary_search_by_key(&block, |(id, _)| *id)
                    .ok()
                    .map(|position| self.removed[position].1.as_slice())
            };
            let remove = |arguments: &mut Vec<ValueId>, removed: Option<&[usize]>| {
                if let Some(removed) = removed {
                    let mut position = 0;
                    arguments.retain(|_| {
                        let keep = removed.binary_search(&position).is_err();
                        position += 1;
                        keep
                    });
                }
            };
            for block in &mut machine.blocks {
                if let Some(removed) = positions(block.id) {
                    let mut position = 0;
                    block.parameters.retain(|_| {
                        let keep = removed.binary_search(&position).is_err();
                        position += 1;
                        keep
                    });
                }
                match &mut block.terminator {
                    Terminator::Jump {
                        target, arguments, ..
                    } => remove(arguments, positions(*target)),
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for edge in [when_true, when_false] {
                            remove(&mut edge.arguments, positions(edge.target));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            BoundaryMachineId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue,
            MachineId, OperationId, ScalarType,
        };
        use terminal_psi::{
            Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
            TerminalMachineResult, ValueDeclaration,
        };

        fn integer(id: u64) -> ValueDeclaration {
            ValueDeclaration {
                id: ValueId::new(id).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                qualifications: Default::default(),
            }
        }

        fn graph(base: u64) -> TerminalMachine {
            let value = |id| integer(base + id);
            let condition = ValueDeclaration {
                scalar_type: ScalarType::Boolean,
                ..value(8)
            };
            let constant = |operation, id| {
                Operation::unsuspended(
                    OperationId::new(operation).unwrap(),
                    OperationResult::Scalar(value(id)),
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(17),
                    },
                )
            };
            let edge = |id, raw| SuccessorEdge {
                edge: EdgeId::new(id).unwrap(),
                target: BlockId::new(2).unwrap(),
                arguments: vec![value(raw).id, value(3).id],
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            };
            let entry = Block {
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                operations: vec![
                    constant(1, 1),
                    constant(2, 4),
                    Operation::unsuspended(
                        OperationId::new(3).unwrap(),
                        OperationResult::Scalar(value(3)),
                        OperationKind::BoundaryCall {
                            boundary: BoundaryMachineId::new(1).unwrap(),
                            arguments: vec![value(1).id],
                            structural_arguments: Vec::new(),
                            completion_receipts: Vec::new(),
                            requirement_obligations: Vec::new(),
                        },
                    ),
                ],
                terminator: Terminator::Conditional {
                    condition: condition.id,
                    when_true: edge(1, 1),
                    when_false: edge(2, 4),
                },
            };
            let target = Block {
                id: BlockId::new(2).unwrap(),
                parameters: vec![value(10), value(11)],
                structural_parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                operations: vec![Operation::unsuspended(
                    OperationId::new(4).unwrap(),
                    OperationResult::Scalar(value(12)),
                    OperationKind::IntegerBitwiseNot {
                        operand: value(11).id,
                    },
                )],
                terminator: Terminator::Return {
                    edge: EdgeId::new(3).unwrap(),
                    value: value(12).id,
                    cleanup_actions: Vec::new(),
                },
            };
            TerminalMachine {
                assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
                id: MachineId::new(1).unwrap(),
                declaration: None,
                attachment: None,
                nominal_cleanup_subject: None,
                parameters: vec![condition],
                parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(value(19)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                block_entry_claims: Vec::new(),
                loan_completions: Vec::new(),
                declared_service_reach: Vec::new(),
                closed_reach_application: None,
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: entry.id,
                blocks: vec![entry, target],
                contract: MachineContract::empty(ContractId::new(1).unwrap()),
            }
        }

        #[test]
        fn generated_scalar_transport_removes_only_unused_slots_on_every_incoming_edge() {
            for base in [0, u64::MAX - 32] {
                let mut machine = graph(base);
                let original = machine.clone();
                assert!(complete_scalar_demand(
                    &mut machine,
                    [],
                    [],
                    ScalarDemandMode::KeepAllOperations
                ));
                assert_eq!(machine.parameters, original.parameters);
                assert_eq!(machine.result, original.result);
                assert_eq!(machine.blocks[1].parameters, [integer(base + 11)]);
                for (before, after) in original.blocks.iter().zip(&machine.blocks) {
                    assert_eq!(
                        before.operations, after.operations,
                        "source operations are never erased"
                    );
                }
                let Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &machine.blocks[0].terminator
                else {
                    panic!("original control retained")
                };
                assert_eq!(when_true.arguments, [ValueId::new(base + 3).unwrap()]);
                assert_eq!(when_false.arguments, when_true.arguments);
            }
        }

        #[test]
        fn generated_scalar_transport_keeps_real_reads_and_evidence_and_refuses_partial_inputs() {
            let mut read = graph(0);
            read.blocks[1].operations.push(Operation::unsuspended(
                OperationId::new(5).unwrap(),
                OperationResult::Scalar(integer(13)),
                OperationKind::IntegerBitwiseNot {
                    operand: integer(10).id,
                },
            ));
            let before = read.clone();
            assert!(complete_scalar_demand(
                &mut read,
                [],
                [],
                ScalarDemandMode::KeepAllOperations
            ));
            assert_eq!(read, before);
            let mut optimized = before.clone();
            assert!(complete_scalar_demand(
                &mut optimized,
                [],
                [],
                ScalarDemandMode::DropUnusedTotalOperations
            ));
            assert!(
                !optimized
                    .operations()
                    .any(|operation| operation.id == OperationId::new(5).unwrap())
            );
            assert!(
                optimized
                    .operations()
                    .any(|operation| operation.id == OperationId::new(3).unwrap())
            );
            assert_eq!(optimized.blocks[1].parameters, [integer(11)]);

            for (retained, protected) in [
                (vec![integer(10).id], Vec::new()),
                (Vec::new(), vec![BlockId::new(2).unwrap()]),
            ] {
                let mut pinned = graph(0);
                let before = pinned.clone();
                assert!(complete_scalar_demand(
                    &mut pinned,
                    retained,
                    protected,
                    ScalarDemandMode::KeepAllOperations
                ));
                assert_eq!(pinned, before);
            }
            for proof in [false, true] {
                let mut evidence = graph(0);
                let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
                let type_identity = identities.intern("u64");
                let term = semantic_vocabulary::ScalarTerm::Value {
                    id: integer(10).id,
                    scalar_type: integer(10).scalar_type,
                };
                evidence.blocks[1].terminator = Terminator::Jump {
                    edge: EdgeId::new(3).unwrap(),
                    target: BlockId::new(3).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: if proof {
                        Vec::new()
                    } else {
                        vec![term.clone()]
                    },
                    erased_proof_arguments: if proof {
                        vec![semantic_vocabulary::ProofTerm::Scalar(term)]
                    } else {
                        Vec::new()
                    },
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                evidence.blocks.push(Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    erased_scalar_formals: if proof { Vec::new() } else { vec![integer(20)] },
                    erased_proof_formals: if proof {
                        vec![terminal_psi::ErasedProofFormal {
                            source_position: 0,
                            type_identity,
                        }]
                    } else {
                        Vec::new()
                    },
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(4).unwrap(),
                        value: integer(12).id,
                        cleanup_actions: Vec::new(),
                    },
                });
                let before = evidence.clone();
                assert!(complete_scalar_demand(
                    &mut evidence,
                    [],
                    [],
                    ScalarDemandMode::KeepAllOperations
                ));
                assert_eq!(evidence, before);
                let Terminator::Jump {
                    erased_arguments,
                    erased_proof_arguments,
                    ..
                } = &mut evidence.blocks[1].terminator
                else {
                    unreachable!()
                };
                let missing = semantic_vocabulary::ScalarTerm::Value {
                    id: integer(30).id,
                    scalar_type: integer(30).scalar_type,
                };
                if proof {
                    erased_proof_arguments[0] = semantic_vocabulary::ProofTerm::Scalar(missing);
                } else {
                    erased_arguments[0] = missing;
                }
                let before = evidence.clone();
                assert!(!complete_scalar_demand(
                    &mut evidence,
                    [],
                    [],
                    ScalarDemandMode::KeepAllOperations
                ));
                assert_eq!(evidence, before);
            }
            let mut cleanup = graph(0);
            let Terminator::Return {
                cleanup_actions, ..
            } = &mut cleanup.blocks[1].terminator
            else {
                unreachable!()
            };
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                terminal_psi::StructuralAffineDiscard {
                    place: semantic_vocabulary::PlaceId::new(1).unwrap(),
                    structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    path: vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: integer(10).id,
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    }],
                },
            ));
            let before = cleanup.clone();
            assert!(complete_scalar_demand(
                &mut cleanup,
                [],
                [],
                ScalarDemandMode::KeepAllOperations
            ));
            assert_eq!(cleanup, before);

            for actual in [integer(30).id, ValueId::new(8).unwrap(), integer(12).id] {
                let mut unused = graph(0);
                let Terminator::Conditional { when_false, .. } = &mut unused.blocks[0].terminator
                else {
                    unreachable!()
                };
                when_false.arguments[0] = actual;
                let before = unused.clone();
                assert!(!complete_scalar_demand(
                    &mut unused,
                    [],
                    [],
                    ScalarDemandMode::KeepAllOperations
                ));
                assert_eq!(
                    unused, before,
                    "unused undefined, wrongly typed or unavailable inputs cannot disappear"
                );
            }
            let mut incomplete = graph(0);
            let Terminator::Conditional { when_false, .. } = &mut incomplete.blocks[0].terminator
            else {
                unreachable!()
            };
            when_false.arguments.pop();
            let before = incomplete.clone();
            assert!(!complete_scalar_demand(
                &mut incomplete,
                [],
                [],
                ScalarDemandMode::KeepAllOperations
            ));
            assert_eq!(incomplete, before);
            let mut unresolved = graph(0);
            unresolved.blocks[1].operations[0].kind = OperationKind::IntegerBitwiseNot {
                operand: integer(30).id,
            };
            let before = unresolved.clone();
            assert!(!complete_scalar_demand(
                &mut unresolved,
                [],
                [],
                ScalarDemandMode::KeepAllOperations
            ));
            assert_eq!(unresolved, before);
        }
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
