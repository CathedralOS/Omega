use super::*;

impl<'program> Evaluator<'program> {
    pub(super) fn eval_unary(&self, operator: UnaryOperator, operand: Value) -> EvalResult<Value> {
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
    pub(super) fn named_float_operation_name(&self, expression: ExpressionHandle) -> Option<&str> {
        let operator_use = self
            .operator_facts?
            .named_uses()
            .find(|operator_use| operator_use.expression == expression)?;
        let operator = self
            .program
            .operators()
            .iter()
            .find(|operator| operator.symbol == operator_use.selected_operator_symbol)?;
        let members = self.program.operator_path_members(operator.name);
        match members.first()?.as_str() {
            "F32" | "F64" => members.last().map(|member| member.as_str()),
            _ => None,
        }
    }

    /// Execute an unnameable provider builtin for a selected ternary float
    /// operation. The named contract chooses fused versus separately rounded
    /// semantics and any explicit direction, then adapts only the final result
    /// using all original operands.
    pub(super) fn eval_rewritten_ternary_float(
        &mut self,
        arguments: &[ExpressionHandle],
        format: SemanticFloatFormat,
        intrinsic: &str,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let scalar_type = arguments
            .iter()
            .filter_map(|argument| self.expression_scalar_type(*argument, frame))
            .reduce(|left, right| {
                if left.1 != ArithmeticDomain::Exact {
                    left
                } else {
                    right
                }
            });
        let mut operands = Vec::with_capacity(3);
        for operand in arguments {
            let Value::Float(value) = self.eval_expression(*operand, frame)? else {
                return unsupported("selected ternary float operand is not a float");
            };
            operands.push(project_landed_float(format, value));
        }
        let meaning = match intrinsic {
            "float#multiply_then_add_f32" | "float#multiply_then_add_f64" => {
                FloatSemantics::multiply_then_add(format, &operands[0], &operands[1], &operands[2])
            }
            "float#fused_multiply_add_f32" | "float#fused_multiply_add_f64" => {
                FloatSemantics::fused_multiply_add(format, &operands[0], &operands[1], &operands[2])
            }
            "float#fused_multiply_add_toward_zero_f32"
            | "float#fused_multiply_add_toward_zero_f64" => {
                FloatSemantics::fused_multiply_add_toward_zero(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                )
            }
            "float#fused_multiply_add_toward_positive_f32"
            | "float#fused_multiply_add_toward_positive_f64" => {
                FloatSemantics::fused_multiply_add_toward_positive(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                )
            }
            "float#fused_multiply_add_toward_negative_f32"
            | "float#fused_multiply_add_toward_negative_f64" => {
                FloatSemantics::fused_multiply_add_toward_negative(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                )
            }
            _ => return unsupported("unknown selected ternary-float intrinsic"),
        };
        let operation = intrinsic
            .strip_prefix("float#")
            .and_then(|name| name.rsplit_once('_').map(|(operation, _)| operation))
            .unwrap_or("ternary_float");
        let meaning = match scalar_type.map(|(_, domain)| domain) {
            Some(ArithmeticDomain::Saturating) => {
                let operand_refs = operands.iter().collect::<Vec<_>>();
                FloatSemantics::apply_saturating_policy(format, &operand_refs, meaning)
            }
            Some(ArithmeticDomain::Trapping) => FloatSemantics::apply_trapping_policy(meaning)
                .map_err(|trap_class| {
                    Halt::Trap(format!(
                        "named float operation `{operation}` produced {} in Trapping domain",
                        match trap_class {
                            FloatPolicyTrap::NaNResult => "NaN",
                            FloatPolicyTrap::InfinityResult => "infinity",
                        }
                    ))
                })?,
            Some(ArithmeticDomain::Exact | ArithmeticDomain::Wrapping) | None => meaning,
        };
        Ok(Value::Float(meaning.to_interpreter_value(format)))
    }

    /// Execute an unnameable one-step directed binary operation selected by an exact
    /// provider plan. The shared semantic engine supplies the result; policy
    /// adapts only that result exactly like the native lowering.
    pub(super) fn eval_rewritten_directed_binary(
        &mut self,
        arguments: &[ExpressionHandle],
        format: SemanticFloatFormat,
        intrinsic: &str,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let scalar_type = arguments
            .iter()
            .filter_map(|argument| self.expression_scalar_type(*argument, frame))
            .reduce(|left, right| {
                if left.1 != ArithmeticDomain::Exact {
                    left
                } else {
                    right
                }
            });
        let mut operands = Vec::with_capacity(2);
        for operand in arguments {
            let Value::Float(value) = self.eval_expression(*operand, frame)? else {
                return unsupported("selected directed-float operand is not a float");
            };
            operands.push(project_landed_float(format, value));
        }
        let operation = intrinsic
            .strip_prefix("float#")
            .and_then(|name| name.split_once("_toward_").map(|(operation, _)| operation));
        let direction = if intrinsic.contains("toward_zero") {
            Some("zero")
        } else if intrinsic.contains("toward_positive") {
            Some("positive")
        } else if intrinsic.contains("toward_negative") {
            Some("negative")
        } else {
            None
        };
        let meaning = match (operation, direction) {
            (Some("add"), Some("zero")) => {
                FloatSemantics::add_toward_zero(format, &operands[0], &operands[1])
            }
            (Some("add"), Some("positive")) => {
                FloatSemantics::add_toward_positive(format, &operands[0], &operands[1])
            }
            (Some("add"), Some("negative")) => {
                FloatSemantics::add_toward_negative(format, &operands[0], &operands[1])
            }
            (Some("subtract"), Some("zero")) => {
                FloatSemantics::subtract_toward_zero(format, &operands[0], &operands[1])
            }
            (Some("subtract"), Some("positive")) => {
                FloatSemantics::subtract_toward_positive(format, &operands[0], &operands[1])
            }
            (Some("subtract"), Some("negative")) => {
                FloatSemantics::subtract_toward_negative(format, &operands[0], &operands[1])
            }
            (Some("multiply"), Some("zero")) => {
                FloatSemantics::multiply_toward_zero(format, &operands[0], &operands[1])
            }
            (Some("multiply"), Some("positive")) => {
                FloatSemantics::multiply_toward_positive(format, &operands[0], &operands[1])
            }
            (Some("multiply"), Some("negative")) => {
                FloatSemantics::multiply_toward_negative(format, &operands[0], &operands[1])
            }
            (Some("divide"), Some("zero")) => {
                FloatSemantics::divide_toward_zero(format, &operands[0], &operands[1])
            }
            (Some("divide"), Some("positive")) => {
                FloatSemantics::divide_toward_positive(format, &operands[0], &operands[1])
            }
            (Some("divide"), Some("negative")) => {
                FloatSemantics::divide_toward_negative(format, &operands[0], &operands[1])
            }
            _ => return unsupported("unknown selected directed-float intrinsic"),
        };
        let meaning = match scalar_type.map(|(_, domain)| domain) {
            Some(ArithmeticDomain::Saturating) => {
                let operand_refs = operands.iter().collect::<Vec<_>>();
                FloatSemantics::apply_saturating_policy(format, &operand_refs, meaning)
            }
            Some(ArithmeticDomain::Trapping) => FloatSemantics::apply_trapping_policy(meaning)
                .map_err(|trap_class| {
                    Halt::Trap(format!(
                        "directed float operation produced {} in Trapping domain",
                        match trap_class {
                            FloatPolicyTrap::NaNResult => "NaN",
                            FloatPolicyTrap::InfinityResult => "infinity",
                        }
                    ))
                })?,
            Some(ArithmeticDomain::Exact | ArithmeticDomain::Wrapping) | None => meaning,
        };
        Ok(Value::Float(meaning.to_interpreter_value(format)))
    }

    /// Execute an unnameable one-step directed square root selected by an exact
    /// provider plan. Its single authored argument is evaluated once.
    pub(super) fn eval_rewritten_directed_square_root(
        &mut self,
        arguments: &[ExpressionHandle],
        format: SemanticFloatFormat,
        intrinsic: &str,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let scalar_type = self.expression_scalar_type(arguments[0], frame);
        let Value::Float(value) = self.eval_expression(arguments[0], frame)? else {
            return unsupported("selected directed square-root operand is not a float");
        };
        let operand = project_landed_float(format, value);
        let meaning = if intrinsic.contains("toward_zero") {
            FloatSemantics::square_root_toward_zero(format, &operand)
        } else if intrinsic.contains("toward_positive") {
            FloatSemantics::square_root_toward_positive(format, &operand)
        } else if intrinsic.contains("toward_negative") {
            FloatSemantics::square_root_toward_negative(format, &operand)
        } else {
            return unsupported("unknown selected directed square-root intrinsic");
        };
        let meaning = match scalar_type.map(|(_, domain)| domain) {
            Some(ArithmeticDomain::Saturating) => {
                FloatSemantics::apply_saturating_policy(format, &[&operand], meaning)
            }
            Some(ArithmeticDomain::Trapping) => FloatSemantics::apply_trapping_policy(meaning)
                .map_err(|trap_class| {
                    Halt::Trap(format!(
                        "directed square root produced {} in Trapping domain",
                        match trap_class {
                            FloatPolicyTrap::NaNResult => "NaN",
                            FloatPolicyTrap::InfinityResult => "infinity",
                        }
                    ))
                })?,
            Some(ArithmeticDomain::Exact | ArithmeticDomain::Wrapping) | None => meaning,
        };
        Ok(Value::Float(meaning.to_interpreter_value(format)))
    }

    pub(super) fn eval_binary(
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
        if let (Value::Str(a), Value::Str(b)) = (&left, &right) {
            if operator == Add {
                let mut joined = a.borrow().clone();
                joined.extend_from_slice(&b.borrow());
                return self.allocate_text(joined);
            }
        }

        // Float arithmetic / comparison if either operand is float.
        if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
            let l = left
                .as_float()
                .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))?;
            let r = right
                .as_float()
                .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))?;
            return self.eval_float_binary(operator, l, r, scalar_type, named_float_operation);
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
        if matches!(operator, Divide | Modulo) {
            if let Some((
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
        }
        // A WRAPPING Add/Sub/Mul likewise wraps at the node: with no landing
        // seam (a guard-direct `au + bu == 44`), the full-width comparison
        // would see the wide 300 while native's byte-width compare sees the
        // wrapped 44. Wrapping is congruence-preserving for +/-/* chains, so
        // truncating each intermediate agrees with native's wide-compute +
        // width-sensitive-op truncation everywhere.
        if let Some((ty, ArithmeticDomain::Wrapping)) = scalar_type {
            if matches!(operator, Add | Subtract | Multiply | ShiftLeft | ShiftRight) {
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
            Equal | NotEqual | And | Or => unreachable!("handled earlier"),
        })
    }

    fn eval_float_binary(
        &self,
        operator: BinaryOperator,
        l: f64,
        r: f64,
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
        let decode = |value: f64| project_landed_float(format, value);
        let left = decode(l);
        let right = decode(r);
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
            let landed = meaning.to_interpreter_value(format);
            Ok(Value::Float(landed))
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
            Equal | NotEqual | And | Or => unreachable!("handled earlier"),
        })
    }

    pub(super) fn eval_min_max(
        &self,
        name: &str,
        left: Value,
        right: Value,
        unsigned: bool,
    ) -> EvalResult<Value> {
        if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
            let l = left
                .as_float()
                .ok_or_else(|| Halt::Trap("min/max float".to_owned()))?;
            let r = right
                .as_float()
                .ok_or_else(|| Halt::Trap("min/max float".to_owned()))?;
            // Match the native SSE semantics exactly: `maxsd a, b` returns b
            // when the values are unordered (any NaN) or equal, and the larger
            // otherwise -- i.e. `if a > b { a } else { b }` (partial `>` is
            // false for NaN). `minsd` is the mirror. Rust's `f64::max`/`min`
            // differ (they return the non-NaN operand), which would diverge
            // from the backend on a NaN second operand.
            let left_meaning = FloatMeaning::from_f64(l);
            let right_meaning = FloatMeaning::from_f64(r);
            let pick_left = if name == "max" {
                FloatSemantics::greater(&left_meaning, &right_meaning)
            } else {
                FloatSemantics::less(&left_meaning, &right_meaning)
            };
            // Preserve the selected runtime operand's honest NaN bits even
            // though the base FloatMeaning contract erases their payload.
            return Ok(Value::Float(if pick_left { l } else { r }));
        }
        let l = left
            .as_int()
            .ok_or_else(|| Halt::Trap("min/max int".to_owned()))?;
        let r = right
            .as_int()
            .ok_or_else(|| Halt::Trap("min/max int".to_owned()))?;
        // Compare as u64 when a u64-classed operand is present (the larger/smaller
        // u64 bit pattern IS one of {l, r}, reinterpreted back to i64); signed
        // otherwise. Without this `max`/`min` on an msb-set u64 picks the wrong
        // operand (u64::MAX reads as -1 under signed compare).
        let picked = if unsigned {
            let (lu, ru) = (l as u64, r as u64);
            (if name == "max" {
                lu.max(ru)
            } else {
                lu.min(ru)
            }) as i64
        } else if name == "max" {
            l.max(r)
        } else {
            l.min(r)
        };
        Ok(Value::Int(picked))
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
            (Value::Bool(a), Value::Bool(b)) => a == b,
            _ => {
                if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
                    let left = FloatMeaning::from_f64(left.as_float().unwrap_or(f64::NAN));
                    let right = FloatMeaning::from_f64(right.as_float().unwrap_or(f64::NAN));
                    FloatSemantics::equal(&left, &right)
                } else {
                    left.as_int() == right.as_int()
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
            let data = self
                .program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == type_symbol)?;
            return ordinal_in(data);
        }
        self.program.data_definitions().iter().find_map(ordinal_in)
    }
}
