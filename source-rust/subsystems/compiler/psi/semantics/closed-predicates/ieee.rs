//! Evaluate closed IEEE comparisons and equations between mathematical float meanings.

use numerics::float_semantics::{
    FloatFormat, FloatMeaning, FloatSemanticOperand, FloatSemanticOperation, FloatSemanticResult,
    FloatSemantics,
};
use semantic_vocabulary::{
    ClauseIdentity, FloatMeaningClauseOperand, FloatMeaningClauseValue, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatValue, IntegerMathLiteral, Proposition, ScalarTerm,
};

pub fn closed_ieee_relation(proposition: &Proposition) -> Option<bool> {
    ieee_relation(proposition, &|_| None)
}

pub(crate) fn ieee_relation(
    proposition: &Proposition,
    observe: &impl Fn(&ScalarTerm) -> Option<FloatMeaning>,
) -> Option<bool> {
    let Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    } = proposition
    else {
        return None;
    };
    if left.scalar_type().as_ieee_float()? != *format
        || right.scalar_type().as_ieee_float()? != *format
    {
        return None;
    }
    let (left, right) = (
        observed_meaning(left, observe)?,
        observed_meaning(right, observe)?,
    );
    Some(match kind {
        IeeeFloatComparisonKind::Equal => FloatSemantics::equal(&left, &right),
        IeeeFloatComparisonKind::NotEqual => FloatSemantics::not_equal(&left, &right),
        IeeeFloatComparisonKind::LessThan => FloatSemantics::less(&left, &right),
        IeeeFloatComparisonKind::LessOrEqual => FloatSemantics::less_or_equal(&left, &right),
        IeeeFloatComparisonKind::NotLessThan => !FloatSemantics::less(&left, &right),
        IeeeFloatComparisonKind::NotLessOrEqual => !FloatSemantics::less_or_equal(&left, &right),
    })
}

fn observed_meaning(
    source: &ScalarTerm,
    observe: &impl Fn(&ScalarTerm) -> Option<FloatMeaning>,
) -> Option<FloatMeaning> {
    match source {
        ScalarTerm::IeeeFloatLiteral(bits) => Some(meaning(*bits)),
        _ => observe(source),
    }
}

fn meaning(value: IeeeFloatValue) -> FloatMeaning {
    match value {
        IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
    }
}

/// Evaluate the checked graph in dependency order after actual arguments and
/// the returned scalar have been substituted. Structural equality of meanings
/// retains zero signs and discards NaN payloads.
pub(crate) fn closed_float_meaning_equality(
    clause: &ClauseIdentity,
    observe: &impl Fn(&ScalarTerm) -> Option<FloatMeaning>,
) -> Option<bool> {
    clause.validate().ok()?;
    let mut values: Vec<FloatMeaning> = Vec::with_capacity(clause.values.len());
    for row in clause.float_meaning_rows() {
        let value = match row {
            FloatMeaningClauseValue::Projection { source, .. } => {
                observed_meaning(source, observe)?
            }
            FloatMeaningClauseValue::Application {
                contract, operands, ..
            } => {
                let operation = FloatSemanticOperation::for_contract_identity(contract)?;
                let mut arguments = Vec::with_capacity(operands.len());
                for operand in operands {
                    arguments.push(match operand {
                        FloatMeaningClauseOperand::Format(format) => {
                            FloatSemanticOperand::Format(match format {
                                IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                                IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                            })
                        }
                        FloatMeaningClauseOperand::Meaning(slot) => {
                            FloatSemanticOperand::Meaning(values.get(slot.0 as usize)?.clone())
                        }
                        FloatMeaningClauseOperand::Integer(ScalarTerm::Integer {
                            value, ..
                        }) => FloatSemanticOperand::Integer(crate::big_integer_literal(
                            IntegerMathLiteral::from_integer_value(*value),
                        )),
                        FloatMeaningClauseOperand::Integer(_) => return None,
                    });
                }
                let FloatSemanticResult::Meaning(result) =
                    operation.kernel_discharge(&arguments, 4096).ok()?.result
                else {
                    return None;
                };
                result
            }
        };
        values.push(value);
    }
    Some(values.get(clause.left as usize)? == values.get(clause.right as usize)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::float_semantics::FloatSemanticValueKind::Meaning;

    fn equation(name: &str, expected: u32, inputs: &[u32]) -> Proposition {
        let operation =
            FloatSemanticOperation::for_float_operands(name, inputs.len(), Meaning).unwrap();
        let literal = |bits| ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(bits));
        Proposition::DeclarationIdentity(
            ClauseIdentity::float_result_equation(
                literal(expected),
                operation.contract_identity(),
                IeeeFloatFormat::Binary32,
                inputs.iter().copied().map(literal),
            )
            .unwrap(),
        )
    }

    #[test]
    fn closed_result_equations_check_the_actual_return_for_each_rounding() {
        let inputs = [0x3f80_0001, 0x3f80_0001, 0xbf80_0002];
        for (operation, correct, wrong) in [
            ("fused_multiply_add", 0x2880_0000, 0),
            ("multiply_then_add", 0, 0x2880_0000),
        ] {
            let mut integers = crate::ClosedIntegerEvaluator::default();
            assert_eq!(
                crate::closed_guard(&equation(operation, correct, &inputs), &mut integers),
                Ok(Some(true))
            );
            assert_eq!(
                crate::closed_guard(&equation(operation, wrong, &inputs), &mut integers),
                Ok(Some(false))
            );
        }
    }

    #[test]
    fn meaning_equations_preserve_signed_zero_and_erase_nan_payloads() {
        let mut integers = crate::ClosedIntegerEvaluator::default();
        for (input, returned, expected) in [
            (0, 0x8000_0000, true),
            (0, 0, false),
            (0x7fc0_0001, 0x7fc0_0002, true),
        ] {
            assert_eq!(
                crate::closed_guard(&equation("negate", returned, &[input]), &mut integers),
                Ok(Some(expected))
            );
        }
    }

    #[test]
    fn raw_identity_compares_exact_bits_unlike_ieee_equality() {
        use semantic_vocabulary::{ClauseCarrier, ClauseOperand, ClauseValue, ScalarType};
        let identity = |left: IeeeFloatValue, right: IeeeFloatValue| {
            let value = |literal: IeeeFloatValue| ClauseValue {
                carrier: ClauseCarrier::Scalar(ScalarType::IeeeFloat(literal.format())),
                operand: ClauseOperand::Scalar(ScalarTerm::IeeeFloatLiteral(literal)),
            };
            Proposition::DeclarationIdentity(ClauseIdentity {
                values: vec![value(left), value(right)],
                left: 0,
                right: 1,
            })
        };
        let single = IeeeFloatValue::Binary32;
        let double = IeeeFloatValue::Binary64;
        let mut integers = crate::ClosedIntegerEvaluator::default();
        for (left, right, identical) in [
            // One NaN encoding is identical to itself, though IEEE `==` is false.
            (single(0x7fc0_0001), single(0x7fc0_0001), true),
            (
                double(0x7ff8_0000_0000_0001),
                double(0x7ff8_0000_0000_0001),
                true,
            ),
            // Different payloads and signs are different representations.
            (single(0x7fc0_0001), single(0x7fc0_0002), false),
            (single(0x7fc0_0000), single(0xffc0_0000), false),
            // Signed zeros are IEEE-equal but not identical.
            (single(0), single(0x8000_0000), false),
            (double(0), double(0x8000_0000_0000_0000), false),
            (
                double(0x3ff0_0000_0000_0000),
                double(0x3ff0_0000_0000_0000),
                true,
            ),
        ] {
            assert_eq!(
                crate::closed_guard(&identity(left, right), &mut integers),
                Ok(Some(identical)),
                "{left:?} {right:?}"
            );
        }
    }
}
