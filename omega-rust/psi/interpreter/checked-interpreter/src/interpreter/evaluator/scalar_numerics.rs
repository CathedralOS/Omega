//! Scalar numerics the evaluator's operations share: integer bounds, widths
//! and wrapping, the semantic integer format of a primitive, big-integer
//! runtime values, float bit projections and landed-float projection.

use super::{EvalResult, Evaluator, Halt, Value, trap};
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics::{
    FloatFormat as SemanticFloatFormat, FloatMeaning, FloatToIntegerError,
    IntegerFormat as SemanticIntegerFormat,
};
use typed_trees::types::PrimitiveType;

impl Evaluator<'_> {
    /// Policy adapters run before this representation decision. Runtime
    /// interpretation retains its concrete payload behavior; semantic evaluation
    /// must not turn a payload-erased result into an observable bit pattern.
    pub(super) fn semantic_float_result(
        &self,
        meaning: FloatMeaning,
        format: SemanticFloatFormat,
    ) -> Value {
        if self.semantic_float_evaluation && meaning.is_nan() {
            Value::UndeterminedNaN(if format == SemanticFloatFormat::BINARY32 {
                numerics::literals::FloatFormat::F32
            } else {
                numerics::literals::FloatFormat::F64
            })
        } else {
            Value::Float(meaning.to_interpreter_value(format))
        }
    }

    pub(super) fn float_operand(
        &self,
        value: &Value,
        format: SemanticFloatFormat,
    ) -> EvalResult<FloatMeaning> {
        if let Value::UndeterminedNaN(landed) = value {
            let expected = match landed {
                numerics::literals::FloatFormat::F32 => SemanticFloatFormat::BINARY32,
                numerics::literals::FloatFormat::F64 => SemanticFloatFormat::BINARY64,
            };
            if expected != format {
                return trap("floating operand lost its landed format");
            }
            return Ok(FloatMeaning::NaN);
        }
        value
            .as_float()
            .map(|value| project_landed_float(format, value))
            .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))
    }
}

pub(super) fn project_landed_float(format: SemanticFloatFormat, value: f64) -> FloatMeaning {
    if format == SemanticFloatFormat::BINARY32 {
        FloatProjectionOperation::Meaning32
            .project_f32(value as f32)
            .expect("binary32 projection row accepts f32")
    } else {
        FloatProjectionOperation::Meaning64
            .project_f64(value)
            .expect("binary64 projection row accepts f64")
    }
}

pub(super) fn integer_bounds(ty: PrimitiveType) -> Option<(i64, i64)> {
    match ty {
        PrimitiveType::I8 => Some((i8::MIN as i64, i8::MAX as i64)),
        PrimitiveType::U8 => Some((0, u8::MAX as i64)),
        PrimitiveType::I16 => Some((i16::MIN as i64, i16::MAX as i64)),
        PrimitiveType::U16 => Some((0, u16::MAX as i64)),
        PrimitiveType::I32 => Some((i32::MIN as i64, i32::MAX as i64)),
        PrimitiveType::U32 => Some((0, u32::MAX as i64)),
        PrimitiveType::I64 => Some((i64::MIN, i64::MAX)),
        _ => None,
    }
}

pub(super) fn semantic_integer_format(ty: PrimitiveType) -> Option<SemanticIntegerFormat> {
    match ty {
        PrimitiveType::I8 => Some(SemanticIntegerFormat::I8),
        PrimitiveType::I16 => Some(SemanticIntegerFormat::I16),
        PrimitiveType::I32 => Some(SemanticIntegerFormat::I32),
        PrimitiveType::I64 => Some(SemanticIntegerFormat::I64),
        PrimitiveType::U8 => Some(SemanticIntegerFormat::U8),
        PrimitiveType::U16 => Some(SemanticIntegerFormat::U16),
        PrimitiveType::U32 => Some(SemanticIntegerFormat::U32),
        PrimitiveType::U64 | PrimitiveType::Addr => Some(SemanticIntegerFormat::U64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

pub(super) fn is_unsigned_integer_primitive(ty: PrimitiveType) -> bool {
    matches!(
        ty,
        PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::Addr
    )
}

pub(super) fn big_integer_runtime_value(value: &BigInt, ty: PrimitiveType) -> i64 {
    if is_unsigned_integer_primitive(ty) {
        value
            .to_u64()
            .expect("checked unsigned conversion fits its target") as i64
    } else {
        value
            .to_i64()
            .expect("checked signed conversion fits its target")
    }
}

pub(super) fn float_to_integer_trap_message(
    target: PrimitiveType,
    reason: FloatToIntegerError,
    trapping: bool,
) -> String {
    let operation = if trapping { "Trapping" } else { "Exact" };
    let reason = match reason {
        FloatToIntegerError::NonFinite => "the value is not finite",
        FloatToIntegerError::OutOfRange => "the truncated value is out of range",
    };
    format!("float-to-int conversion failed in {operation} domain: {reason} for {target:?}")
}

/// Preserve an f32 NaN's sign and payload while carrying interpreter floats in
/// the existing f64-backed `Value::Float`. Finite values and infinities remain
/// ordinary numeric f64 values; only NaNs use this reversible payload embedding.
pub(super) fn interpreter_f32_from_bits(bits: u32) -> f64 {
    let value = f32::from_bits(bits);
    if !value.is_nan() {
        return value as f64;
    }

    let sign = ((bits as u64) >> 31) << 63;
    let payload = (u64::from(bits) & 0x007f_ffff) << 29;
    f64::from_bits(sign | 0x7ff0_0000_0000_0000 | payload)
}

pub(super) fn interpreter_f32_to_bits(value: f64) -> u32 {
    if !value.is_nan() {
        return (value as f32).to_bits();
    }

    let bits = value.to_bits();
    let sign = ((bits >> 63) as u32) << 31;
    let mut payload = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32;
    payload &= 0x007f_ffff;
    if payload == 0 {
        payload = 0x0040_0000;
    }
    sign | 0x7f80_0000 | payload
}

/// Apply a write target's arithmetic domain (decision 17) to a raw i64 result,
/// mirroring the native backend so the differential oracle agrees:
/// Exact/Wrapping truncate to width; Saturating clamps to [min, max]; Trapping
/// halts (overflow trap) when the value is out of range.
pub(super) fn apply_arithmetic_domain(
    raw: i64,
    ty: PrimitiveType,
    domain: ArithmeticDomain,
) -> EvalResult<i64> {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => Ok(wrap_to_width(raw, ty)),
        ArithmeticDomain::Saturating => match integer_bounds(ty) {
            Some((min, max)) => Ok(raw.clamp(min, max)),
            None => Ok(wrap_to_width(raw, ty)),
        },
        ArithmeticDomain::Trapping => match integer_bounds(ty) {
            Some((min, max)) if raw < min || raw > max => trap(format!(
                "arithmetic overflow in Trapping domain: {raw} is out of range for {ty:?}"
            )),
            _ => Ok(wrap_to_width(raw, ty)),
        },
    }
}

/// The bit width a WRAPPING shift wraps at (the modular-arithmetic modulus
/// exponent). Pointer-width types are 64-bit in both engines.
pub(super) fn primitive_bit_width(ty: PrimitiveType) -> u64 {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => 8,
        PrimitiveType::I16 | PrimitiveType::U16 => 16,
        PrimitiveType::I32 | PrimitiveType::U32 => 32,
        _ => 64,
    }
}

pub(super) fn wrap_to_width(raw: i64, ty: PrimitiveType) -> i64 {
    match ty {
        PrimitiveType::I8 => raw as i8 as i64,
        PrimitiveType::U8 => raw as u8 as i64,
        PrimitiveType::I16 => raw as i16 as i64,
        PrimitiveType::U16 => raw as u16 as i64,
        PrimitiveType::I32 => raw as i32 as i64,
        PrimitiveType::U32 => raw as u32 as i64,
        // 64-bit and pointer-width types keep the full value (unsigned reinterpretation of a
        // u64 is still represented by the same bit pattern in i64).
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => raw,
        // Non-integer primitives do not reach this path.
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => raw,
    }
}

/// Byte width of an integer primitive -- the PROMOTION rank a mixed-width
/// binary node computes in. `None` for non-integer primitives.
pub(super) fn integer_primitive_byte_width(ty: PrimitiveType) -> Option<usize> {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => Some(1),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(2),
        PrimitiveType::I32 | PrimitiveType::U32 => Some(4),
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => Some(8),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

pub(super) fn primitive_is_unsigned64(primitive: Option<PrimitiveType>) -> bool {
    matches!(primitive, Some(PrimitiveType::U64 | PrimitiveType::Addr))
}

#[cfg(test)]
mod undetermined_nan_tests {
    use super::{
        ArithmeticDomain, Evaluator, FloatMeaning, PrimitiveType, SemanticFloatFormat, Value,
    };
    use crate::BuildTimeValue;
    use numerics::float_semantics::FloatSemantics;
    use numerics::literals::FloatFormat;
    use typed_trees::{TypedTrees, expression::BinaryOperator};

    #[test]
    fn computed_nan_has_bits_only_in_ordinary_interpretation() {
        let program = TypedTrees::default();
        let mut evaluator = Evaluator::new(&program, &[]);
        for primitive in [PrimitiveType::F32, PrimitiveType::F64] {
            let result = evaluator
                .eval_float_binary(
                    BinaryOperator::Divide,
                    &Value::Float(0.0),
                    &Value::Float(0.0),
                    Some((primitive, ArithmeticDomain::Exact)),
                    None,
                )
                .unwrap_or_else(|_| panic!("ordinary float division must succeed"));
            assert!(matches!(result, Value::Float(value) if value.is_nan()));
        }
        evaluator.configure_build_evaluation(1000, None);
        for (primitive, format) in [
            (PrimitiveType::F32, FloatFormat::F32),
            (PrimitiveType::F64, FloatFormat::F64),
        ] {
            let result = evaluator
                .eval_float_binary(
                    BinaryOperator::Divide,
                    &Value::Float(0.0),
                    &Value::Float(0.0),
                    Some((primitive, ArithmeticDomain::Exact)),
                    None,
                )
                .unwrap_or_else(|_| panic!("semantic float division must succeed"));
            assert!(matches!(result, Value::UndeterminedNaN(actual) if actual == format));
            assert!(result.as_float().is_none());
            let meaning = result.float_meaning().expect("semantic NaN");
            assert!(FloatSemantics::is_nan(&meaning));
            assert!(!FloatSemantics::is_finite(&meaning));
            assert!(
                evaluator
                    .eval_float_binary(
                        BinaryOperator::Add,
                        &result,
                        &Value::Float(1.0),
                        Some((primitive, ArithmeticDomain::Trapping)),
                        None,
                    )
                    .is_err()
            );
            assert!(matches!(evaluator.eval_float_binary(
                BinaryOperator::Add, &result, &Value::Float(1.0),
                Some((primitive, ArithmeticDomain::Saturating)), None,
            ), Ok(Value::UndeterminedNaN(actual)) if actual == format));
            for (operator, expected) in [
                (BinaryOperator::Equal, false),
                (BinaryOperator::NotEqual, true),
                (BinaryOperator::Less, false),
                (BinaryOperator::GreaterOrEqual, false),
            ] {
                assert!(matches!(evaluator.eval_binary(
                    operator, result.clone(), result.clone(), false,
                    Some((primitive, ArithmeticDomain::Exact)), None,
                ), Ok(Value::Bool(actual)) if actual == expected));
            }
        }
    }

    #[test]
    fn semantic_nan_round_trips_through_owned_aggregate_bridge() {
        let snapshot = BuildTimeValue::Struct {
            type_name: "FloatingPair".into(),
            fields: vec![(
                "values".into(),
                BuildTimeValue::Array(vec![
                    BuildTimeValue::UndeterminedNaN(FloatFormat::F32),
                    BuildTimeValue::Case {
                        variant: "Present".into(),
                        payload: vec![(
                            "value".into(),
                            BuildTimeValue::UndeterminedNaN(FloatFormat::F64),
                        )],
                    },
                ]),
            )],
        };
        let value = snapshot
            .clone()
            .into_value_with(&|value: Value| Ok::<_, String>(value.cell()), &|bytes| {
                Ok::<_, String>(Value::bytes(bytes))
            })
            .expect("fresh owned argument graph");
        let copied = value
            .deep_clone_with(&|value: Value| Ok::<_, String>(value.cell()))
            .expect("owned aggregate copy");
        assert_eq!(BuildTimeValue::from_value(&copied), snapshot);
        assert_eq!(
            BuildTimeValue::from_value(&Value::Ref(value.cell())),
            snapshot
        );
    }

    #[test]
    fn semantic_nan_converts_without_acquiring_bits_or_implicit_carrier_changes() {
        let program = TypedTrees::default();
        let mut evaluator = Evaluator::new(&program, &[]);
        evaluator.configure_build_evaluation(1000, None);
        let source = Value::UndeterminedNaN(FloatFormat::F32);
        assert!(
            evaluator
                .float_operand(&source, SemanticFloatFormat::BINARY64)
                .is_err()
        );
        assert!(
            evaluator
                .coerce_scalar_with(source.clone(), PrimitiveType::F64, ArithmeticDomain::Exact)
                .is_err()
        );
        assert!(matches!(
            evaluator.eval_cast(
                source.clone(),
                Some(PrimitiveType::F32),
                Some(PrimitiveType::F64),
                ArithmeticDomain::Exact,
            ),
            Ok(Value::UndeterminedNaN(FloatFormat::F64))
        ));
        assert!(matches!(
            evaluator.eval_cast(
                source.clone(),
                Some(PrimitiveType::F32),
                Some(PrimitiveType::I32),
                ArithmeticDomain::Saturating,
            ),
            Ok(Value::Int(0))
        ));
        for policy in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Trapping,
            ArithmeticDomain::Wrapping,
        ] {
            assert!(
                evaluator
                    .eval_cast(
                        source.clone(),
                        Some(PrimitiveType::F32),
                        Some(PrimitiveType::I32),
                        policy
                    )
                    .is_err()
            );
        }
        for format in [SemanticFloatFormat::BINARY32, SemanticFloatFormat::BINARY64] {
            let zero =
                evaluator.semantic_float_result(FloatMeaning::Zero { negative: true }, format);
            assert!(matches!(zero, Value::Float(value) if value.to_bits() == (-0.0f64).to_bits()));
        }
    }

    #[test]
    fn payloadless_nan_rejects_raw_recasts_and_byte_observations() {
        let program = TypedTrees::default();
        let mut evaluator = Evaluator::new(&program, &[]);
        evaluator.configure_build_evaluation(1000, None);
        for (format, primitive, integer, width) in [
            (FloatFormat::F32, PrimitiveType::F32, PrimitiveType::U32, 4),
            (FloatFormat::F64, PrimitiveType::F64, PrimitiveType::U64, 8),
        ] {
            let value = Value::UndeterminedNaN(format);
            assert!(value.as_float().is_none());
            for target in [primitive, integer] {
                assert!(evaluator.eval_recast(value.clone(), Some(target)).is_err());
                assert!(
                    evaluator
                        .eval_recast(Value::Ref(value.clone().cell()), Some(target))
                        .is_err()
                );
            }
            let bytes = (0..width)
                .map(|_| Value::Int(0xa5).cell())
                .collect::<Vec<_>>();
            assert!(
                evaluator
                    .write_scalar_byte_region(&bytes, 0, primitive, value.clone())
                    .is_err()
            );
            assert!(
                bytes
                    .iter()
                    .all(|cell| cell.borrow().as_int() == Some(0xa5))
            );
            *bytes[0].borrow_mut() = value;
            assert!(
                evaluator
                    .assemble_scalar_byte_region(&bytes, 0, primitive)
                    .is_err()
            );
        }
    }

    #[test]
    fn explicit_nan_bits_survive_storage_bridge_and_raw_recasts() {
        let program = TypedTrees::default();
        let mut evaluator = Evaluator::new(&program, &[]);
        evaluator.configure_build_evaluation(1000, None);
        for (primitive, integer, bits, width) in [
            (PrimitiveType::F32, PrimitiveType::U32, 0xff80_1234_u64, 4),
            (
                PrimitiveType::F64,
                PrimitiveType::U64,
                0xfff0_0000_0000_1234_u64,
                8,
            ),
        ] {
            let concrete = evaluator
                .eval_recast(Value::Int(bits as i64), Some(primitive))
                .unwrap_or_else(|_| panic!("authored raw bits must be accepted"));
            let concrete = evaluator
                .coerce_scalar_with(concrete, primitive, ArithmeticDomain::Exact)
                .unwrap_or_else(|_| panic!("storage must preserve explicit bits"));
            let snapshot = BuildTimeValue::from_value(&concrete);
            assert!(matches!(snapshot, BuildTimeValue::Float(value) if value.is_nan()));
            let restored = snapshot
                .into_value_with(&|value: Value| Ok::<_, String>(value.cell()), &|bytes| {
                    Ok::<_, String>(Value::bytes(bytes))
                })
                .expect("concrete bridge round trip");
            assert!(
                matches!(evaluator.eval_recast(restored.clone(), Some(integer)), Ok(Value::Int(value)) if value as u64 == bits)
            );
            let bytes = (0..width).map(|_| Value::Int(0).cell()).collect::<Vec<_>>();
            assert!(
                evaluator
                    .write_scalar_byte_region(&bytes, 0, primitive, restored)
                    .is_ok(),
                "explicit bits must be writable"
            );
            let reread = evaluator
                .assemble_scalar_byte_region(&bytes, 0, primitive)
                .unwrap_or_else(|_| panic!("explicit bytes must be readable"));
            assert!(
                matches!(evaluator.eval_recast(reread, Some(integer)), Ok(Value::Int(value)) if value as u64 == bits)
            );
        }
    }
}
