use crate::interpreter::evaluator::{
    ArithmeticDomain, BinaryOperator, Cell, DataDefinition, DataMember, EvalResult, Evaluator,
    ExpressionHandle, FloatMeaning, FloatPolicyTrap, FloatSemantics, Frame, Halt, PrimitiveType,
    SemanticFloatFormat, SymbolHandle, UnaryOperator, Value, integer_bounds, primitive_bit_width,
    primitive_is_unsigned64, trap, unsupported, wrap_to_width,
};
use numerics::float_semantics::RoundingDirection;
use symbols::BuiltinFunction;

fn byte_sequence_element(element: &Cell) -> EvalResult<u8> {
    let element = element.borrow();
    let Value::Int(value) = &*element else {
        return unsupported("byte-sequence equality has a non-byte element");
    };
    u8::try_from(*value).map_err(|_| {
        Halt::Unsupported("byte-sequence equality has an out-of-range element".to_owned())
    })
}

/// A compiler-minted named-float intrinsic: the executable semantics one
/// unnameable call target spells and the call arity it takes.
///
/// A state-local expression copy keeps only the target spelling, so the
/// spelling is resolved here once: the stem names the operation and the
/// `_f32`/`_f64` suffix names the format. Routes then dispatch the resolved
/// operation instead of re-listing its spellings and re-deriving its format.
#[derive(Clone, Copy)]
pub(in crate::interpreter::evaluator) enum NamedFloatIntrinsic {
    /// `float#multiply_then_add_*`: a separately rounded product, then a sum.
    MultiplyThenAdd,
    /// `float#fused_multiply_add_*`: one rounding over the exact sum.
    FusedMultiplyAdd,
    /// `float#fused_multiply_add_toward_<direction>_*`.
    FusedMultiplyAddDirected(RoundingDirection),
    /// `float#<add|subtract|multiply|divide>_toward_<direction>_*`.
    DirectedBinary(BinaryOperator, RoundingDirection),
    /// `float#sqrt_toward_<direction>_*`.
    DirectedSquareRoot(RoundingDirection),
}

impl NamedFloatIntrinsic {
    /// Resolve one compiler-minted spelling, or `None` when the target names
    /// some other builtin family.
    fn from_target(target: &str) -> Option<(Self, SemanticFloatFormat)> {
        let spelling = target.strip_prefix("float#")?;
        let (stem, format) = match spelling.strip_suffix("_f32") {
            Some(stem) => (stem, SemanticFloatFormat::BINARY32),
            None => (
                spelling.strip_suffix("_f64")?,
                SemanticFloatFormat::BINARY64,
            ),
        };
        let Some((operation, direction)) = stem.rsplit_once("_toward_") else {
            let intrinsic = match stem {
                "multiply_then_add" => Self::MultiplyThenAdd,
                "fused_multiply_add" => Self::FusedMultiplyAdd,
                _ => return None,
            };
            return Some((intrinsic, format));
        };
        let direction = match direction {
            "zero" => RoundingDirection::TowardZero,
            "positive" => RoundingDirection::TowardPositive,
            "negative" => RoundingDirection::TowardNegative,
            _ => return None,
        };
        let intrinsic = match operation {
            "fused_multiply_add" => Self::FusedMultiplyAddDirected(direction),
            "sqrt" => Self::DirectedSquareRoot(direction),
            "add" => Self::DirectedBinary(BinaryOperator::Add, direction),
            "subtract" => Self::DirectedBinary(BinaryOperator::Subtract, direction),
            "multiply" => Self::DirectedBinary(BinaryOperator::Multiply, direction),
            "divide" => Self::DirectedBinary(BinaryOperator::Divide, direction),
            _ => return None,
        };
        Some((intrinsic, format))
    }

    /// The call-operand count this intrinsic's semantic definition takes.
    const fn arity(self) -> usize {
        match self {
            Self::MultiplyThenAdd | Self::FusedMultiplyAdd | Self::FusedMultiplyAddDirected(_) => 3,
            Self::DirectedBinary(..) => 2,
            Self::DirectedSquareRoot(_) => 1,
        }
    }

    /// The operand refusal this operation class reports.
    const fn operand_refusal(self) -> &'static str {
        match self {
            Self::MultiplyThenAdd | Self::FusedMultiplyAdd | Self::FusedMultiplyAddDirected(_) => {
                "selected ternary float operand is not a float"
            }
            Self::DirectedBinary(..) => "selected directed-float operand is not a float",
            Self::DirectedSquareRoot(_) => "selected directed square-root operand is not a float",
        }
    }

    /// The operation name one trapped result reports, in the wording of the
    /// intrinsic class it belongs to.
    fn trapping_operation(self, target: &str) -> String {
        match self {
            Self::MultiplyThenAdd | Self::FusedMultiplyAdd | Self::FusedMultiplyAddDirected(_) => {
                format!(
                    "named float operation `{}`",
                    target
                        .strip_prefix("float#")
                        .and_then(|name| name.rsplit_once('_').map(|(operation, _)| operation))
                        .unwrap_or("ternary_float")
                )
            }
            Self::DirectedBinary(..) => "directed float operation".to_owned(),
            Self::DirectedSquareRoot(_) => "directed square root".to_owned(),
        }
    }
}

/// The exact `FloatSemantics` kernel one directed binary operation takes in one
/// rounding direction.
fn directed_binary_kernel(
    operation: BinaryOperator,
    direction: RoundingDirection,
) -> Option<fn(SemanticFloatFormat, &FloatMeaning, &FloatMeaning) -> FloatMeaning> {
    use RoundingDirection::{TowardNegative, TowardPositive, TowardZero};
    Some(match (operation, direction) {
        (BinaryOperator::Add, TowardZero) => FloatSemantics::add_toward_zero,
        (BinaryOperator::Add, TowardPositive) => FloatSemantics::add_toward_positive,
        (BinaryOperator::Add, TowardNegative) => FloatSemantics::add_toward_negative,
        (BinaryOperator::Subtract, TowardZero) => FloatSemantics::subtract_toward_zero,
        (BinaryOperator::Subtract, TowardPositive) => FloatSemantics::subtract_toward_positive,
        (BinaryOperator::Subtract, TowardNegative) => FloatSemantics::subtract_toward_negative,
        (BinaryOperator::Multiply, TowardZero) => FloatSemantics::multiply_toward_zero,
        (BinaryOperator::Multiply, TowardPositive) => FloatSemantics::multiply_toward_positive,
        (BinaryOperator::Multiply, TowardNegative) => FloatSemantics::multiply_toward_negative,
        (BinaryOperator::Divide, TowardZero) => FloatSemantics::divide_toward_zero,
        (BinaryOperator::Divide, TowardPositive) => FloatSemantics::divide_toward_positive,
        (BinaryOperator::Divide, TowardNegative) => FloatSemantics::divide_toward_negative,
        _ => return None,
    })
}

/// The exact `FloatSemantics` kernel one directed ternary operation takes in
/// one rounding direction.
fn fused_multiply_add_kernel(
    direction: RoundingDirection,
) -> fn(SemanticFloatFormat, &FloatMeaning, &FloatMeaning, &FloatMeaning) -> FloatMeaning {
    match direction {
        RoundingDirection::TowardZero => FloatSemantics::fused_multiply_add_toward_zero,
        RoundingDirection::TowardPositive => FloatSemantics::fused_multiply_add_toward_positive,
        RoundingDirection::TowardNegative => FloatSemantics::fused_multiply_add_toward_negative,
        RoundingDirection::NearestTiesToEven => FloatSemantics::fused_multiply_add,
    }
}

/// The exact `FloatSemantics` kernel one directed square root takes in one
/// rounding direction.
fn directed_square_root_kernel(
    direction: RoundingDirection,
) -> fn(SemanticFloatFormat, &FloatMeaning) -> FloatMeaning {
    match direction {
        RoundingDirection::TowardZero => FloatSemantics::square_root_toward_zero,
        RoundingDirection::TowardPositive => FloatSemantics::square_root_toward_positive,
        RoundingDirection::TowardNegative => FloatSemantics::square_root_toward_negative,
        RoundingDirection::NearestTiesToEven => FloatSemantics::square_root,
    }
}

impl<'program> Evaluator<'program> {
    pub(in crate::interpreter::evaluator) fn eval_unary(
        &self,
        operator: UnaryOperator,
        operand: Value,
    ) -> EvalResult<Value> {
        match operator {
            UnaryOperator::BitwiseNot => match operand {
                Value::Int(value) => Ok(Value::Int(!value)),
                _ => Err(Halt::Trap("bitwise-not of non-integer".to_owned())),
            },
            UnaryOperator::LogicalNot => operand
                .as_bool()
                .map(|value| Value::Bool(!value))
                .ok_or_else(|| Halt::Trap("logical-not of non-boolean".to_owned())),
        }
    }

    /// Recover the authored named float operation for a root-preserving
    /// intrinsic rewrite. Provider dispatch rewrites the expression node in
    /// place, so the checked use fact remains the stable bridge back to the
    /// selected `F32::...` or `F64::...` operation.
    pub(in crate::interpreter::evaluator) fn named_float_operation_name(
        &self,
        expression: ExpressionHandle,
    ) -> Option<&str> {
        let operator_use = self
            .operator_facts?
            .named_uses()
            .find(|operator_use| operator_use.expression == expression)?;
        let operator = self
            .program
            .operator_definition_by_symbol(operator_use.selected_operator_symbol)?;
        let members = self.program.operator_path_members(operator.name);
        match members.first()?.as_str() {
            "F32" | "F64" => members.last().map(|member| member.as_str()),
            _ => None,
        }
    }

    /// Execute one compiler-minted named-float intrinsic selected by an exact
    /// provider plan. The resolved intrinsic names the exact `FloatSemantics`
    /// kernel and the call arity, the spelling names the format, and the
    /// operands' declared arithmetic domain adapts only the final result --
    /// exactly like the native lowering.
    pub(in crate::interpreter::evaluator) fn eval_named_float_intrinsic(
        &mut self,
        target: &str,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some((intrinsic, format)) = NamedFloatIntrinsic::from_target(target) else {
            return Ok(None);
        };
        // A call whose operand count does not fit the intrinsic's semantic
        // definition is not this intrinsic; the next candidate decides.
        if arguments.len() != intrinsic.arity() {
            return Ok(None);
        }
        let domain = arguments
            .iter()
            .filter_map(|argument| self.expression_scalar_type(*argument, frame))
            .reduce(|left, right| {
                if left.1 != ArithmeticDomain::Exact {
                    left
                } else {
                    right
                }
            })
            .map(|(_, domain)| domain);
        let mut operands = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let value = self.eval_expression(*argument, frame)?;
            if !value.is_float() {
                return unsupported(intrinsic.operand_refusal());
            }
            operands.push(self.float_operand(&value, format)?);
        }
        let meaning = match intrinsic {
            NamedFloatIntrinsic::MultiplyThenAdd => {
                FloatSemantics::multiply_then_add(format, &operands[0], &operands[1], &operands[2])
            }
            NamedFloatIntrinsic::FusedMultiplyAdd => {
                FloatSemantics::fused_multiply_add(format, &operands[0], &operands[1], &operands[2])
            }
            NamedFloatIntrinsic::FusedMultiplyAddDirected(direction) => {
                fused_multiply_add_kernel(direction)(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                )
            }
            NamedFloatIntrinsic::DirectedBinary(operation, direction) => {
                let Some(kernel) = directed_binary_kernel(operation, direction) else {
                    return unsupported("unknown selected directed-float intrinsic");
                };
                kernel(format, &operands[0], &operands[1])
            }
            NamedFloatIntrinsic::DirectedSquareRoot(direction) => {
                directed_square_root_kernel(direction)(format, &operands[0])
            }
        };
        let meaning = match domain {
            Some(ArithmeticDomain::Saturating) => {
                let operand_refs = operands.iter().collect::<Vec<_>>();
                FloatSemantics::apply_saturating_policy(format, &operand_refs, meaning)
            }
            Some(ArithmeticDomain::Trapping) => FloatSemantics::apply_trapping_policy(meaning)
                .map_err(|trap_class| {
                    Halt::Trap(format!(
                        "{} produced {} in Trapping domain",
                        intrinsic.trapping_operation(target),
                        trap_class.label()
                    ))
                })?,
            Some(ArithmeticDomain::Exact | ArithmeticDomain::Wrapping) | None => meaning,
        };
        Ok(Some(self.semantic_float_result(meaning, format)))
    }

    /// Runtime integers use i64 storage even for u64. Decode each value with
    /// its own carrier before applying the shared mathematical relation.
    pub(in crate::interpreter::evaluator) fn eval_integer_comparison(
        &self,
        operator: BinaryOperator,
        left: i64,
        left_type: PrimitiveType,
        right: i64,
        right_type: PrimitiveType,
    ) -> EvalResult<Value> {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        let decode = |value: i64, primitive| -> EvalResult<(IntegerType, IntegerValue)> {
            let (sign, width) = match primitive {
                PrimitiveType::I8 => (IntegerSign::Signed, 8),
                PrimitiveType::I16 => (IntegerSign::Signed, 16),
                PrimitiveType::I32 => (IntegerSign::Signed, 32),
                PrimitiveType::I64 => (IntegerSign::Signed, 64),
                PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
                PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
                PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
                PrimitiveType::U64 | PrimitiveType::Addr => (IntegerSign::Unsigned, 64),
                _ => return unsupported("integer comparison requires integer carriers"),
            };
            let integer = if primitive == PrimitiveType::Addr {
                IntegerType::address(width)
            } else {
                IntegerType::new(sign, width)
            }
            .map_err(|_| Halt::Unsupported("invalid integer comparison carrier".into()))?;
            let value = if sign == IntegerSign::Signed {
                IntegerValue::Signed(i128::from(value))
            } else {
                IntegerValue::Unsigned(u128::from(value as u64))
            };
            Ok((integer, value))
        };
        let (left_type, left) = decode(left, left_type)?;
        let (right_type, right) = decode(right, right_type)?;
        let ordering = left_type
            .compare_with(left, right_type, right)
            .ok_or_else(|| {
                Halt::Unsupported("integer comparison operand outside its carrier".into())
            })?;
        Ok(Value::Bool(match operator {
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => ordering.is_le(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => ordering.is_ge(),
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => ordering.is_ne(),
            _ => return unsupported("integer comparison requires a comparison operator"),
        }))
    }

    pub(in crate::interpreter::evaluator) fn eval_binary(
        &self,
        operator: BinaryOperator,
        left: Value,
        right: Value,
        unsigned_operands: bool,
        scalar_type: Option<(PrimitiveType, ArithmeticDomain)>,
        named_float_operation: Option<&str>,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;

        // Logical short-circuit-style operators (already fully evaluated here).
        if matches!(operator, And | Or) {
            let l = left
                .as_bool()
                .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
            let r = right
                .as_bool()
                .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
            return Ok(Value::Bool(match operator {
                And => l && r,
                Or => l || r,
                _ => unreachable!(),
            }));
        }

        // Equality / inequality across scalar kinds (incl. enums).
        if matches!(operator, Equal | NotEqual) {
            let equal = self.values_equal(&left, &right)?;
            return Ok(Value::Bool(if operator == Equal { equal } else { !equal }));
        }

        // String concatenation: `a + b` over two strings yields a fresh string.
        if let (Value::Str(a), Value::Str(b)) = (&left, &right)
            && operator == Add
        {
            let mut joined = a.borrow().to_vec();
            joined.extend_from_slice(&b.borrow());
            return self.allocate_text(joined);
        }

        // Float arithmetic / comparison if either operand is float.
        if left.is_float() || right.is_float() {
            return self.eval_float_binary(
                operator,
                &left,
                &right,
                scalar_type,
                named_float_operation,
            );
        }

        // Integer arithmetic / comparison. A payload-free CASE operand
        // contributes its TAG ordinal: the value-position `match` desugar
        // (parser primary.rs) produces `default + (s == p) * (variant -
        // default)`, which natively IS tag arithmetic -- the oracle must
        // compute the same integers or every errno->ErrorKind classification
        // traps on `Enum - Enum`.
        let l = self.arithmetic_operand_int(&left)?;
        let r = self.arithmetic_operand_int(&right)?;
        // Saturating/Trapping ADD/SUB/MUL clamp/trap at the OPERATION itself
        // (decision 17): compute WIDE in i128 -- two in-bounds operands cannot
        // overflow it, and it also covers the 64-bit widths, which the i64
        // landing seams cannot express (a wrapped u64 MAX+5 arrives at the
        // seam as 4 with the overflow evidence gone; only the node, holding
        // BOTH operands, can clamp). 64-bit UNSIGNED views its `Value::Int`
        // bit patterns as u64 and clamps to [0, u64::MAX]. Other domains and
        // operators keep the wide i64 compute + landing-seam coercion.
        // SIGNED div/mod under a non-Exact domain resolve MIN/-1 at the node
        // (the one overflowing corner: |quotient| otherwise shrinks):
        // Wrapping wraps it back to MIN (matching aarch64 `sdiv` and the
        // x86_64 idiv guard), Saturating clamps it to MAX (`a % -1` is 0
        // either way), Trapping traps. Division by zero keeps the existing
        // trap. Unsigned div/mod never overflow and fall through.
        if matches!(operator, Divide | Modulo)
            && let Some((
                ty @ (PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64),
                domain @ (ArithmeticDomain::Wrapping
                | ArithmeticDomain::Saturating
                | ArithmeticDomain::Trapping),
            )) = scalar_type
        {
            if r == 0 {
                return if operator == Divide {
                    trap("integer division by zero")
                } else {
                    trap("integer modulo by zero")
                };
            }
            let wide = if operator == Divide {
                l as i128 / r as i128
            } else {
                l as i128 % r as i128
            };
            let (min, max) = integer_bounds(ty).unwrap_or((i64::MIN, i64::MAX));
            return match domain {
                ArithmeticDomain::Wrapping => Ok(Value::Int(wrap_to_width(wide as i64, ty))),
                ArithmeticDomain::Saturating => {
                    Ok(Value::Int(wide.clamp(min as i128, max as i128) as i64))
                }
                ArithmeticDomain::Trapping if wide < min as i128 || wide > max as i128 => {
                    trap(format!(
                        "arithmetic overflow in Trapping domain: {wide} is out of range for {ty:?}"
                    ))
                }
                _ => Ok(Value::Int(wide as i64)),
            };
        }
        // A WRAPPING Add/Sub/Mul likewise wraps at the node: with no landing
        // seam (a guard-direct `au + bu == 44`), the full-width comparison
        // would see the wide 300 while native's byte-width compare sees the
        // wrapped 44. Wrapping is congruence-preserving for +/-/* chains, so
        // truncating each intermediate agrees with native's wide-compute +
        // width-sensitive-op truncation everywhere.
        if let Some((ty, ArithmeticDomain::Wrapping)) = scalar_type
            && matches!(operator, Add | Subtract | Multiply | ShiftLeft | ShiftRight)
        {
            let wide = match operator {
                Add => l.wrapping_add(r),
                Subtract => l.wrapping_sub(r),
                Multiply => l.wrapping_mul(r),
                // MASKED COUNT at the operand width (F8, ch5 shift-count
                // ruling, settled 2026-07-18: Wrapping masks the count to
                // `k & (width - 1)` -- the genuinely modular reading, and
                // what the hardware computes anyway). This SUPERSEDES the
                // 2026-07-13 modular-VALUE semantics (at-width counts no
                // longer collapse to 0/sign-fill; they shift by the
                // masked count). Bit-masking the two's-complement count
                // is well-defined for negative counts too, exactly like
                // the register-form shifts on both ISAs.
                ShiftLeft => {
                    let masked = ((r as u64) & (primitive_bit_width(ty) - 1)) as u32;
                    l.wrapping_shl(masked)
                }
                ShiftRight => {
                    let masked = ((r as u64) & (primitive_bit_width(ty) - 1)) as u32;
                    if unsigned_operands {
                        ((l as u64).wrapping_shr(masked)) as i64
                    } else {
                        l.wrapping_shr(masked)
                    }
                }
                _ => unreachable!(),
            };
            return Ok(Value::Int(wrap_to_width(wide, ty)));
        }
        if let Some((ty, domain @ (ArithmeticDomain::Saturating | ArithmeticDomain::Trapping))) =
            scalar_type
        {
            // Domain-governed SHIFTS. F8c (ch5 shift-count ruling): under
            // TRAPPING an out-of-range count TRAPS -- regardless of the
            // shifted VALUE (`0 << 40` traps; the count is invalid, not the
            // result). Saturating cannot reach an out-of-range count (the
            // F8a validation obligation rejects it), so its floor/clamp arms
            // below only ever see in-range counts.
            if domain == ArithmeticDomain::Trapping
                && matches!(operator, ShiftLeft | ShiftRight)
                && (r as u64) >= primitive_bit_width(ty)
            {
                return trap(format!(
                    "shift count out of range in Trapping domain: the count is not below \
                     the operand width for {ty:?}"
                ));
            }
            // `>>` is floor(x / 2^n) and cannot overflow; the Saturating
            // floor semantics for an (unreachable) at/above-width count stay
            // for robustness.
            if operator == ShiftRight {
                return Ok(Value::Int(wrap_to_width(
                    if (r as u64) >= primitive_bit_width(ty) {
                        if unsigned_operands || l >= 0 { 0 } else { -1 }
                    } else if unsigned_operands {
                        ((l as u64).wrapping_shr(r as u32)) as i64
                    } else {
                        l.wrapping_shr(r as u32)
                    },
                    ty,
                )));
            }
            // `<<` is x * 2^n: Saturating clamps and Trapping traps when the
            // TRUE value leaves the type's range (in-range counts only here
            // -- the count trap above owns the out-of-range face).
            if operator == ShiftLeft {
                let (minimum, maximum, value) = if primitive_is_unsigned64(Some(ty)) {
                    (0i128, u64::MAX as i128, l as u64 as i128)
                } else {
                    let (minimum, maximum) = integer_bounds(ty).unwrap_or((i64::MIN, i64::MAX));
                    (minimum as i128, maximum as i128, l as i128)
                };
                let wide = if (r as u64) >= primitive_bit_width(ty) {
                    // Saturating only (Trapping trapped above): any nonzero x
                    // overflows once the count reaches the width; drive the
                    // clamp below with a synthetic out-of-range value on x's
                    // side of the range.
                    match value.signum() {
                        0 => 0,
                        1 => maximum + 1,
                        _ => minimum - 1,
                    }
                } else {
                    value << (r as u32)
                };
                return match domain {
                    ArithmeticDomain::Saturating => {
                        Ok(Value::Int(wide.clamp(minimum, maximum) as i64))
                    }
                    ArithmeticDomain::Trapping if wide < minimum || wide > maximum => {
                        trap(format!(
                            "arithmetic overflow in Trapping domain: shifted value is out of range for {ty:?}"
                        ))
                    }
                    _ => Ok(Value::Int(wide as i64)),
                };
            }
            if matches!(operator, Add | Subtract | Multiply) {
                let bounds_and_wide = if primitive_is_unsigned64(Some(ty)) {
                    let (lu, ru) = (l as u64 as i128, r as u64 as i128);
                    let wide = match operator {
                        Add => lu + ru,
                        Subtract => lu - ru,
                        Multiply => lu * ru,
                        _ => unreachable!(),
                    };
                    Some((0i128, u64::MAX as i128, wide))
                } else if let Some((min, max)) = integer_bounds(ty) {
                    let wide = match operator {
                        Add => l as i128 + r as i128,
                        Subtract => l as i128 - r as i128,
                        Multiply => l as i128 * r as i128,
                        _ => unreachable!(),
                    };
                    Some((min as i128, max as i128, wide))
                } else {
                    None
                };
                if let Some((min, max, wide)) = bounds_and_wide {
                    return match domain {
                        ArithmeticDomain::Saturating => Ok(Value::Int(wide.clamp(min, max) as i64)),
                        ArithmeticDomain::Trapping if wide < min || wide > max => trap(format!(
                            "arithmetic overflow in Trapping domain: {wide} is out of range for {ty:?}"
                        )),
                        _ => Ok(Value::Int(wide as i64)),
                    };
                }
            }
        }
        self.eval_int_binary(operator, l, r, unsigned_operands)
    }

    fn eval_int_binary(
        &self,
        operator: BinaryOperator,
        l: i64,
        r: i64,
        unsigned_operands: bool,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;
        Ok(match operator {
            Add => Value::Int(l.wrapping_add(r)),
            Subtract => Value::Int(l.wrapping_sub(r)),
            Multiply => Value::Int(l.wrapping_mul(r)),
            Divide => {
                if r == 0 {
                    return trap("integer division by zero");
                }
                if unsigned_operands {
                    Value::Int(((l as u64).wrapping_div(r as u64)) as i64)
                } else {
                    Value::Int(l.wrapping_div(r))
                }
            }
            Modulo => {
                if r == 0 {
                    return trap("integer modulo by zero");
                }
                if unsigned_operands {
                    Value::Int(((l as u64).wrapping_rem(r as u64)) as i64)
                } else {
                    Value::Int(l.wrapping_rem(r))
                }
            }
            ShiftLeft => Value::Int(l.wrapping_shl(r as u32)),
            // Logical (unsigned) shift when the operand is u64-classed;
            // arithmetic shift otherwise.
            ShiftRight if unsigned_operands => {
                Value::Int(((l as u64).wrapping_shr(r as u32)) as i64)
            }
            ShiftRight => Value::Int(l.wrapping_shr(r as u32)),
            BitwiseAnd => Value::Int(l & r),
            BitwiseOr => Value::Int(l | r),
            BitwiseXor => Value::Int(l ^ r),
            Less if unsigned_operands => Value::Bool((l as u64) < (r as u64)),
            LessOrEqual if unsigned_operands => Value::Bool((l as u64) <= (r as u64)),
            Greater if unsigned_operands => Value::Bool((l as u64) > (r as u64)),
            GreaterOrEqual if unsigned_operands => Value::Bool((l as u64) >= (r as u64)),
            Less => Value::Bool(l < r),
            LessOrEqual => Value::Bool(l <= r),
            Greater => Value::Bool(l > r),
            GreaterOrEqual => Value::Bool(l >= r),
            CaseMembership => return trap("case membership requires its retained classifier"),
            Equal | NotEqual | And | Or => unreachable!("handled earlier"),
        })
    }

    pub(in crate::interpreter::evaluator) fn eval_float_binary(
        &self,
        operator: BinaryOperator,
        left: &Value,
        right: &Value,
        scalar_type: Option<(PrimitiveType, ArithmeticDomain)>,
        named_float_operation: Option<&str>,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;
        // Decode landed values to the shared proof meaning, execute exact
        // arithmetic, and round once through the selected format record. The
        // interpreter's f64 storage is only a lossless window for f32 values;
        // it is not the arithmetic definition.
        let format = if matches!(scalar_type, Some((PrimitiveType::F32, _))) {
            SemanticFloatFormat::BINARY32
        } else {
            SemanticFloatFormat::BINARY64
        };
        let left = self.float_operand(left, format)?;
        let right = self.float_operand(right, format)?;
        // F7 policy adapters (float brief §8): SATURATING clamps MAGNITUDE
        // OVERFLOW only; division by zero, invalid results, and non-finite
        // propagation remain non-finite. TRAPPING is result-checked, so a
        // propagated NaN/infinity traps just like one created by this
        // operation. The shared semantic adapter owns those decisions; the
        // interpreter only selects it and renders a specific trap reason.
        let domain = scalar_type.map(|(_, domain)| domain);
        let arith = |meaning: FloatMeaning, division: bool| -> EvalResult<Value> {
            let meaning = match domain {
                Some(ArithmeticDomain::Saturating) if division => {
                    FloatSemantics::apply_saturating_divide_policy(format, &left, &right, meaning)
                }
                Some(ArithmeticDomain::Saturating) => {
                    FloatSemantics::apply_saturating_policy(format, &[&left, &right], meaning)
                }
                Some(ArithmeticDomain::Trapping) => {
                    match FloatSemantics::apply_trapping_policy(meaning) {
                        Ok(finite) => finite,
                        Err(FloatPolicyTrap::NaNResult) => {
                            if let Some(operation) = named_float_operation {
                                return trap(format!(
                                    "named float operation `{operation}` produced NaN in Trapping domain"
                                ));
                            }
                            let reason = if left.is_finite() && right.is_finite() {
                                "invalid float operation in Trapping domain"
                            } else {
                                "non-finite NaN result in Trapping domain"
                            };
                            return trap(reason.to_owned());
                        }
                        Err(FloatPolicyTrap::InfinityResult) => {
                            if let Some(operation) = named_float_operation {
                                return trap(format!(
                                    "named float operation `{operation}` produced infinity in Trapping domain"
                                ));
                            }
                            let reason = if division && right.is_zero() {
                                "float division by zero in Trapping domain"
                            } else if left.is_finite() && right.is_finite() {
                                "float overflow in Trapping domain"
                            } else {
                                "non-finite infinity result in Trapping domain"
                            };
                            return trap(reason.to_owned());
                        }
                    }
                }
                _ => meaning,
            };
            Ok(self.semantic_float_result(meaning, format))
        };
        Ok(match operator {
            Add => return arith(FloatSemantics::add(format, &left, &right), false),
            Subtract => return arith(FloatSemantics::subtract(format, &left, &right), false),
            Multiply => return arith(FloatSemantics::multiply(format, &left, &right), false),
            Divide => return arith(FloatSemantics::divide(format, &left, &right), true),
            Less => Value::Bool(FloatSemantics::less(&left, &right)),
            LessOrEqual => Value::Bool(FloatSemantics::less_or_equal(&left, &right)),
            Greater => Value::Bool(FloatSemantics::greater(&left, &right)),
            GreaterOrEqual => Value::Bool(FloatSemantics::greater_or_equal(&left, &right)),
            Modulo | ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => {
                return unsupported("float modulo/shift/bitwise not supported");
            }
            Equal => Value::Bool(FloatSemantics::equal(&left, &right)),
            NotEqual => Value::Bool(!FloatSemantics::equal(&left, &right)),
            CaseMembership => return trap("case membership requires a nominal sum subject"),
            And | Or => unreachable!("logical operations have Boolean operands"),
        })
    }

    /// Admitted minimum/maximum preserves the selected operand's runtime bits.
    /// IEEE equality and unordered comparisons select the right operand; host
    /// min/max helpers have different NaN behavior and cannot implement this.
    pub(in crate::interpreter::evaluator) fn eval_float_extremum(
        &self,
        builtin: BuiltinFunction,
        left: Value,
        right: Value,
    ) -> EvalResult<Value> {
        let left_meaning = left
            .float_meaning()
            .ok_or_else(|| Halt::Trap("floating extremum requires a float operand".to_owned()))?;
        let right_meaning = right
            .float_meaning()
            .ok_or_else(|| Halt::Trap("floating extremum requires a float operand".to_owned()))?;
        let pick_left = match builtin {
            BuiltinFunction::FloatMaximum => FloatSemantics::greater(&left_meaning, &right_meaning),
            BuiltinFunction::FloatMinimum => FloatSemantics::less(&left_meaning, &right_meaning),
            _ => return unsupported("not an admitted floating extremum"),
        };
        Ok(if pick_left { left } else { right })
    }

    fn values_equal(&self, left: &Value, right: &Value) -> EvalResult<bool> {
        Ok(match (left, right) {
            // Enum equality is a TAG compare only -- a case-pattern guard desugars to
            // `subject == Type::Case` where the right side is a bare (payload-less)
            // case reference, and the native backend compares the constant tag.
            // Payloads participate in `==` only through Equatable synthesis, and the
            // FRONTEND expands that into explicit tag-guarded payload field compares
            // before the interpreter runs, so this compare stays tag-only.
            (
                Value::Enum {
                    variant_name: a, ..
                },
                Value::Enum {
                    variant_name: b, ..
                },
            ) => a == b,
            // A tag INT beside a case value: the value-position `match` desugar
            // computes its result as TAG ARITHMETIC (an Int), which then flows
            // into enum-typed places -- natively both sides are the same tag
            // constant, so the oracle compares the Int against the case's tag
            // ordinal.
            (
                Value::Int(tag),
                Value::Enum {
                    type_symbol,
                    variant_name,
                    ..
                },
            )
            | (
                Value::Enum {
                    type_symbol,
                    variant_name,
                    ..
                },
                Value::Int(tag),
            ) => self.enum_variant_tag(*type_symbol, variant_name) == Some(*tag),
            (Value::Str(a), Value::Str(b)) => *a.borrow() == *b.borrow(),
            // Packed text and cell-backed byte views denote the same live
            // sequence. Compare the window, without decoding or trimming bytes.
            (Value::Array(elements), Value::Str(text))
            | (Value::Str(text), Value::Array(elements)) => {
                let bytes = text.borrow();
                if elements.len() != bytes.len() {
                    return Ok(false);
                }
                for (element, byte) in elements.iter().zip(bytes.iter()) {
                    let value = byte_sequence_element(element)?;
                    if value != *byte {
                        return Ok(false);
                    }
                }
                true
            }
            (Value::Array(left), Value::Array(right)) => {
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (left, right) in left.iter().zip(right) {
                    if byte_sequence_element(left)? != byte_sequence_element(right)? {
                        return Ok(false);
                    }
                }
                true
            }
            (Value::Bool(a), Value::Bool(b)) => a == b,
            _ => {
                if left.is_float() || right.is_float() {
                    let (Some(left), Some(right)) = (left.float_meaning(), right.float_meaning())
                    else {
                        return unsupported("equality has a non-numeric floating-point operand");
                    };
                    FloatSemantics::equal(&left, &right)
                } else {
                    let (Some(left), Some(right)) = (left.as_int(), right.as_int()) else {
                        return unsupported("equality operands have no supported comparison");
                    };
                    left == right
                }
            }
        })
    }

    /// An integer ARITHMETIC operand: a scalar's value, or a payload-free case
    /// value's TAG ordinal (the value-position `match` desugar does tag
    /// arithmetic over bare cases -- natively a case IS its tag constant).
    fn arithmetic_operand_int(&self, value: &Value) -> EvalResult<i64> {
        if let Some(int) = value.as_int() {
            return Ok(int);
        }
        if let Value::Enum {
            type_symbol,
            variant_name,
            payload,
        } = value
            && payload.is_empty()
            && let Some(tag) = self.enum_variant_tag(*type_symbol, variant_name)
        {
            return Ok(tag);
        }
        Err(Halt::Trap("non-integer operand".to_owned()))
    }

    /// The tag ORDINAL of a case: resolved WITHIN the declaring type when the
    /// value carries a valid `type_symbol` (tag 0 = the first variant,
    /// matching the ZII zero case and native tag layout), so same-name
    /// variants at different ordinals across enums (`Ok` = 0 in `UnitResult`
    /// but 1 in `MetadataResult`) never cross-resolve. A symbol-less value
    /// (the build-time boundary) falls back to the name-global scan -- the
    /// same name-keyed grain `values_equal` uses for enum equality.
    fn enum_variant_tag(&self, type_symbol: SymbolHandle, variant_name: &str) -> Option<i64> {
        let ordinal_in = |data: &DataDefinition| {
            let mut ordinal: i64 = 0;
            for member in self.program.data_members(data) {
                if let DataMember::Variant(variant) = member {
                    if variant.name.as_str() == variant_name {
                        return Some(ordinal);
                    }
                    ordinal += 1;
                }
            }
            None
        };
        if type_symbol.is_valid() {
            let data = self.program.data_definition_by_symbol(type_symbol)?;
            return ordinal_in(data);
        }
        self.program.data_definitions().iter().find_map(ordinal_in)
    }
}
