//! Pure interpretation of an already admitted numeric host realization.
//!
//! The selected descriptor supplies operation and format; the exact demanded
//! boundary supplies the signature. Numeric meaning stays in `numerics`, shared
//! with folding, proof and ordinary Terminal operations. Host floating controls
//! cannot select semantics. Payload-preserving operations retain operand bits,
//! and a trapping conversion reports a crash for the core to admit against the
//! declaration, never a made-up result or an unreconciled handler rejection.

use numerics::bignum::BigInt;
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics, IntegerFormat};
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, IntegerValue};
use target_operations::{
    IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation, IeeeFloatRoundingDirection,
    IeeeFloatToIntegerDomain, NearestIeeeFloatArithmeticRealization,
};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue,
};
use terminal_psi::{BoundaryCompletion, BoundaryMachineDeclaration, CrashCause};

use super::super::TerminalRuntimeError;

pub(super) struct NumericImport {
    signature: TerminalImportSignature,
    realization: NearestIeeeFloatArithmeticRealization,
}

impl NumericImport {
    pub(super) fn for_import(
        declaration: &BoundaryMachineDeclaration,
        realization: NearestIeeeFloatArithmeticRealization,
    ) -> Result<Self, TerminalRuntimeError> {
        let signature = TerminalImportSignature::from(declaration);
        if signature.completion != BoundaryCompletion::NormalReturn
            || !signature.structural_parameters.is_empty()
            || !signature.result.scalar().is_some_and(|result| {
                realization.signature_matches(&signature.scalar_parameters, result)
            })
        {
            return Err(TerminalRuntimeError::InvalidProvision(
                "numeric realization differs from its admitted scalar contract",
            ));
        }
        Ok(Self {
            signature,
            realization,
        })
    }
}

impl TerminalImportAdapter for NumericImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for NumericImport {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(rejected("numeric import requires result-aware dispatch"))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(rejected("numeric import received a non-boundary effect"));
        };
        if !structural_arguments.is_empty()
            || arguments.len() != self.signature.scalar_parameters.len()
            || !arguments
                .iter()
                .zip(&self.signature.scalar_parameters)
                .all(|(value, expected)| {
                    value.scalar_type() == *expected
                        && match value {
                            TerminalScalarValue::Integer { scalar_type, value } => {
                                scalar_type.admits(*value)
                            }
                            _ => true,
                        }
                })
        {
            return Err(rejected("numeric operands differ from the bound contract"));
        }
        let result = evaluate(self.realization, arguments)?;
        if let TerminalEffectResult::Scalar(value) = result
            && Some(value.scalar_type()) != self.signature.result.scalar()
        {
            return Err(rejected("numeric result differs from the bound contract"));
        }
        Ok(result)
    }
}

fn evaluate(
    realization: NearestIeeeFloatArithmeticRealization,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    use IeeeFloatArithmeticOperation as Operation;

    let format = match realization.format {
        IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
        IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
    };
    let scalar = |value| TerminalEffectResult::Scalar(value);
    if let Operation::FromInteger(_) = realization.operation {
        let [TerminalScalarValue::Integer { value, .. }] = arguments else {
            return Err(rejected(
                "integer-to-float import requires its integer operand",
            ));
        };
        let integer = match value {
            IntegerValue::Signed(value) => BigInt::from_i128(*value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(*value),
        };
        return Ok(scalar(encode(
            realization.format,
            FloatSemantics::from_integer(format, &integer),
        )));
    }
    let operands = arguments
        .iter()
        .map(|value| match value {
            TerminalScalarValue::IeeeFloat(value) => Ok(*value),
            _ => Err(rejected("float import requires floating operands")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Signature admission fixes arity and each operand format. Conversion
    // alone reads the other format; decode each actual landed operand once.
    let meanings = operands
        .iter()
        .map(|value| decode(*value))
        .collect::<Vec<_>>();
    let first = &meanings[0];
    let meaning =
        match realization.operation {
            Operation::FromInteger(_) => {
                return Err(rejected("integer conversion received floating operands"));
            }
            Operation::Negate => {
                let bits = match operands[0] {
                    IeeeFloatValue::Binary32(bits) => IeeeFloatValue::Binary32(bits ^ (1 << 31)),
                    IeeeFloatValue::Binary64(bits) => IeeeFloatValue::Binary64(bits ^ (1 << 63)),
                };
                return Ok(scalar(TerminalScalarValue::IeeeFloat(bits)));
            }
            Operation::Minimum | Operation::Maximum => {
                let take_first = if realization.operation == Operation::Minimum {
                    FloatSemantics::less(first, &meanings[1])
                } else {
                    FloatSemantics::greater(first, &meanings[1])
                };
                return Ok(scalar(TerminalScalarValue::IeeeFloat(
                    operands[if take_first { 0 } else { 1 }],
                )));
            }
            Operation::IsNan => return Ok(scalar(TerminalScalarValue::Boolean(first.is_nan()))),
            Operation::IsInfinite => {
                return Ok(scalar(TerminalScalarValue::Boolean(first.is_infinite())));
            }
            Operation::TruncateToInteger { target, domain } => {
                let target_type = target.integer_type();
                let integer_format = IntegerFormat {
                    bits: u32::from(target.bits()),
                    signed: target.is_signed(),
                };
                let integer = match domain {
                    IeeeFloatToIntegerDomain::Saturating => {
                        FloatSemantics::to_integer_saturating(first, integer_format)
                    }
                    IeeeFloatToIntegerDomain::Exact => {
                        FloatSemantics::to_integer_exact(first, integer_format).map_err(|_| {
                            rejected("exact float conversion violated its admitted precondition")
                        })?
                    }
                    IeeeFloatToIntegerDomain::Trapping => {
                        match FloatSemantics::to_integer_trapping(first, integer_format) {
                            Ok(integer) => integer,
                            Err(_) => return Ok(TerminalEffectResult::Crash(CrashCause::Trap)),
                        }
                    }
                };
                let value =
                    if target.is_signed() {
                        IntegerValue::Signed(
                            integer.to_i64().map(i128::from).ok_or_else(|| {
                                rejected("signed numeric result exceeds its carrier")
                            })?,
                        )
                    } else {
                        IntegerValue::Unsigned(integer.to_u64().map(u128::from).ok_or_else(
                            || rejected("unsigned numeric result exceeds its carrier"),
                        )?)
                    };
                if !target_type.admits(value) {
                    return Err(rejected(
                        "numeric conversion result exceeds its declared width",
                    ));
                }
                return Ok(scalar(TerminalScalarValue::Integer {
                    scalar_type: target_type,
                    value,
                }));
            }
            Operation::ConvertFormat => FloatSemantics::convert(format, first),
            Operation::SquareRoot => FloatSemantics::square_root(format, first),
            Operation::MultiplyThenAdd => {
                FloatSemantics::multiply_then_add(format, first, &meanings[1], &meanings[2])
            }
            Operation::FusedMultiplyAdd => {
                FloatSemantics::fused_multiply_add(format, first, &meanings[1], &meanings[2])
            }
            Operation::Add => FloatSemantics::add(format, first, &meanings[1]),
            Operation::Subtract => FloatSemantics::subtract(format, first, &meanings[1]),
            Operation::Multiply => FloatSemantics::multiply(format, first, &meanings[1]),
            Operation::Divide => FloatSemantics::divide(format, first, &meanings[1]),
            Operation::DirectedBinary {
                operation,
                rounding,
            } => directed(operation, rounding)(format, first, &meanings[1]),
        };
    Ok(scalar(encode(realization.format, meaning)))
}

fn directed(
    operation: IeeeFloatBinaryOperation,
    rounding: IeeeFloatRoundingDirection,
) -> fn(FloatFormat, &FloatMeaning, &FloatMeaning) -> FloatMeaning {
    use IeeeFloatBinaryOperation as Operation;
    use IeeeFloatRoundingDirection as Direction;
    match (operation, rounding) {
        (Operation::Add, Direction::TowardZero) => FloatSemantics::add_toward_zero,
        (Operation::Add, Direction::TowardPositive) => FloatSemantics::add_toward_positive,
        (Operation::Add, Direction::TowardNegative) => FloatSemantics::add_toward_negative,
        (Operation::Subtract, Direction::TowardZero) => FloatSemantics::subtract_toward_zero,
        (Operation::Subtract, Direction::TowardPositive) => {
            FloatSemantics::subtract_toward_positive
        }
        (Operation::Subtract, Direction::TowardNegative) => {
            FloatSemantics::subtract_toward_negative
        }
        (Operation::Multiply, Direction::TowardZero) => FloatSemantics::multiply_toward_zero,
        (Operation::Multiply, Direction::TowardPositive) => {
            FloatSemantics::multiply_toward_positive
        }
        (Operation::Multiply, Direction::TowardNegative) => {
            FloatSemantics::multiply_toward_negative
        }
        (Operation::Divide, Direction::TowardZero) => FloatSemantics::divide_toward_zero,
        (Operation::Divide, Direction::TowardPositive) => FloatSemantics::divide_toward_positive,
        (Operation::Divide, Direction::TowardNegative) => FloatSemantics::divide_toward_negative,
    }
}

fn decode(value: IeeeFloatValue) -> FloatMeaning {
    match value {
        IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
    }
}

fn encode(format: IeeeFloatFormat, meaning: FloatMeaning) -> TerminalScalarValue {
    TerminalScalarValue::IeeeFloat(match format {
        IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(meaning.to_f32().to_bits()),
        IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(meaning.to_f64().to_bits()),
    })
}

fn rejected(reason: &'static str) -> TerminalEffectRejection {
    TerminalEffectRejection::new(reason)
}
