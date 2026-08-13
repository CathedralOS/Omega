use psi_checked_trees::expression::{BinaryExpression, BinaryOperator, Expression};
use psi_checked_trees::types::PrimitiveType;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::float_semantics::{FloatFormat as SemanticFloatFormat, FloatSemantics};
use psi_numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral};
use psi_symbols::SymbolHandle;

/// The folder reads literals through the i64 VALUE WINDOW (D14): an anonymous
/// literal that fits i64 folds exactly as before; an oversize (u64-magnitude)
/// literal DEFERS -- the expression is left unfolded rather than interpreted
/// through the wrong width. (The deeper type-aware folder is a separate rung.)
fn literal_pair(a: &IntegerLiteral, b: &IntegerLiteral) -> Option<(i64, i64)> {
    Some((a.value_i64()?, b.value_i64()?))
}

/// The LANDED type of a constant fold (CM2, ch5 "Constants: Two Phases"):
/// once a constant lands, its type, signedness, and arithmetic domain ride
/// with it, and every subsequent fold happens at the landed type's semantics.
/// Derived by the caller from the ORIGINAL (pre-substitution) operands'
/// declared types; `None` keeps the transitional bare-i64 window.
#[derive(Clone, Copy)]
pub(super) struct IntegerLanding {
    pub primitive: PrimitiveType,
    pub domain: ArithmeticDomain,
}

#[derive(Clone, Copy)]
pub(super) struct FloatLanding {
    pub format: FloatFormat,
    pub domain: ArithmeticDomain,
}

#[derive(Clone, Copy)]
pub(super) enum ValueLanding {
    Integer(IntegerLanding),
    Float(FloatLanding),
}

impl IntegerLanding {
    fn width_bits(self) -> u32 {
        self.primitive
            .scalar_byte_size()
            .map(|bytes| bytes as u32 * 8)
            .unwrap_or(64)
    }

    fn is_signed(self) -> bool {
        self.primitive.is_signed_integer()
    }

    /// The foundation-layer landing this fold ran at (CR2: fold results are
    /// STAMPED so the fact rides the literal through every later clone,
    /// splice, and table insertion -- the two-phase law's phase-B carrier).
    pub(super) fn as_carrier_landing(self) -> Option<psi_numerics::literals::IntegerLanding> {
        use psi_numerics::literals::LandedIntegerType;
        let landed_type = match self.primitive {
            PrimitiveType::I8 => LandedIntegerType::I8,
            PrimitiveType::I16 => LandedIntegerType::I16,
            PrimitiveType::I32 => LandedIntegerType::I32,
            PrimitiveType::I64 => LandedIntegerType::I64,
            PrimitiveType::U8 => LandedIntegerType::U8,
            PrimitiveType::U16 => LandedIntegerType::U16,
            PrimitiveType::U32 => LandedIntegerType::U32,
            PrimitiveType::U64 => LandedIntegerType::U64,
            PrimitiveType::Addr => LandedIntegerType::Addr,
            PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
                return None;
            }
        };
        Some(psi_numerics::literals::IntegerLanding {
            landed_type,
            domain: self.domain,
        })
    }

    /// The reverse mapping (CR3): a STAMPED literal's carrier landing back to
    /// the fold-side landing, so an operand that already landed can drive a
    /// fold whose destination is anonymous (arg/index positions).
    fn from_carrier_landing(carrier: psi_numerics::literals::IntegerLanding) -> Option<Self> {
        use psi_numerics::literals::LandedIntegerType;
        let primitive = match carrier.landed_type {
            LandedIntegerType::I8 => PrimitiveType::I8,
            LandedIntegerType::I16 => PrimitiveType::I16,
            LandedIntegerType::I32 => PrimitiveType::I32,
            LandedIntegerType::I64 => PrimitiveType::I64,
            LandedIntegerType::U8 => PrimitiveType::U8,
            LandedIntegerType::U16 => PrimitiveType::U16,
            LandedIntegerType::U32 => PrimitiveType::U32,
            LandedIntegerType::U64 => PrimitiveType::U64,
            LandedIntegerType::Addr => PrimitiveType::Addr,
        };
        Some(Self {
            primitive,
            domain: carrier.domain,
        })
    }
}

/// Normalize a plain literal AT a landing and stamp it (the binding-capture
/// face of CR3: a `let`'s captured constant IS a value of the declared type,
/// so the landing rides the substituted literal into every use position).
pub(super) fn land_literal(
    literal: &IntegerLiteral,
    landing: IntegerLanding,
) -> Option<Expression> {
    if literal.landing().is_some() {
        return Some(Expression::Integer(literal.clone()));
    }
    let value = literal.value_i64()?;
    land_result(landed_value(value, landing), landing)
}

/// A stored i64 representative read as the landed type's mathematical value:
/// truncate to the landed width, then sign- or zero-extend per signedness.
fn landed_value(value: i64, landing: IntegerLanding) -> i128 {
    let width = landing.width_bits();
    if width == 64 {
        return if landing.is_signed() {
            value as i128
        } else {
            (value as u64) as i128
        };
    }
    let mask = (1u64 << width) - 1;
    let bits = (value as u64) & mask;
    if landing.is_signed() && bits & (1u64 << (width - 1)) != 0 {
        (bits as i128) - (1i128 << width)
    } else {
        bits as i128
    }
}

/// The i64 representative a mathematical result is STORED as after wrapping
/// to the landed width: sign-extended for signed types, zero-extended (always
/// non-negative) for narrow unsigned types, and the raw bit pattern for
/// 64-bit unsigned (bit-faithful; an 8-byte store materializes it exactly).
/// Keeping representatives normalized is what makes downstream sign-sensitive
/// folds (`>>`, `/`, `%`, comparisons) read the right value.
fn landed_representative(result: i128, landing: IntegerLanding) -> i64 {
    let width = landing.width_bits();
    if width == 64 {
        return result as u64 as i64;
    }
    let mask = (1u64 << width) - 1;
    let bits = (result as u64) & mask;
    if landing.is_signed() && bits & (1u64 << (width - 1)) != 0 {
        (bits | !mask) as i64
    } else {
        bits as i64
    }
}

fn landed_bounds(landing: IntegerLanding) -> (i128, i128) {
    let width = landing.width_bits();
    if landing.is_signed() {
        (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
    } else {
        (0, (1i128 << width) - 1)
    }
}

/// Land an exact mathematical result per the landed arithmetic domain:
/// Exact and Wrapping wrap to width (an Exact overflow was already rejected
/// by the validation obligation upstream, so wrapping is the identity there);
/// Saturating clamps to the type's bounds; a Trapping result that overflows
/// CANNOT fold to a value -- the runtime op must trap -- so it defers.
fn land_result(result: i128, landing: IntegerLanding) -> Option<Expression> {
    let (minimum, maximum) = landed_bounds(landing);
    let representative = match landing.domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {
            landed_representative(result, landing)
        }
        ArithmeticDomain::Saturating => {
            landed_representative(result.clamp(minimum, maximum), landing)
        }
        ArithmeticDomain::Trapping => {
            if result < minimum || result > maximum {
                // TRANSITIONAL: pass the EXACT value through UNWRAPPED (and
                // unstamped -- it is deliberately NOT a value of the landed
                // type). The static-store path detects an out-of-range
                // Trapping constant and emits the guaranteed runtime trap
                // (trapping_frame_slot_constant_overflow_write and its field
                // twin); deferring instead would lower a typeless runtime op
                // whose domain resolution cannot see Trapping. An overflow
                // past i64 cannot ride the i64 window and defers.
                return i64::try_from(result)
                    .ok()
                    .map(|value| Expression::Integer(IntegerLiteral::from_value(value)));
            }
            landed_representative(result, landing)
        }
    };
    // CR2: stamp the fold result with the landing it was computed at, so the
    // fact survives every later substitution and table insertion.
    let literal = IntegerLiteral::from_value(representative);
    let literal = match landing.as_carrier_landing() {
        Some(carrier) => literal.with_landing(carrier),
        None => literal,
    };
    Some(Expression::Integer(literal))
}

/// Fold an integer op AT THE LANDED TYPE. `None` means "do not fold": the
/// expression stays a runtime op whose domain-aware instruction selection is
/// correct (never unsound, merely unfolded). This path OWNS the fold decision
/// when a landing is known -- it never falls back to the type-blind i64
/// window, which is exactly the representation error it exists to retire.
fn fold_landed(
    operator: BinaryOperator,
    a: i64,
    b: i64,
    landing: IntegerLanding,
) -> Option<Expression> {
    use BinaryOperator as Op;

    let left = landed_value(a, landing);
    let right = landed_value(b, landing);
    let width = landing.width_bits();

    match operator {
        Op::Add => land_result(left + right, landing),
        Op::Subtract => land_result(left - right, landing),
        Op::Multiply => match left.checked_mul(right) {
            Some(result) => land_result(result, landing),
            // Only reachable at 64-bit-unsigned extremes (the exact product
            // exceeds i128). Exact/Wrapping want the mod-2^64 bits; a
            // Saturating product this large is past the maximum; Trapping
            // must trap at runtime.
            None => match landing.domain {
                ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {
                    let bits = (left as u64).wrapping_mul(right as u64);
                    land_result(bits as i128, landing)
                }
                ArithmeticDomain::Saturating => land_result(landed_bounds(landing).1, landing),
                ArithmeticDomain::Trapping => None,
            },
        },
        Op::Divide | Op::Modulo => {
            if right == 0 {
                return None;
            }
            // i128 division on normalized values is exact for every landed
            // type (unsigned values are non-negative, so `/` is unsigned
            // division); the one out-of-range case, signed MIN / -1, lands
            // per domain like any other overflow.
            let result = if matches!(operator, Op::Divide) {
                left / right
            } else {
                left % right
            };
            land_result(result, landing)
        }
        Op::ShiftLeft | Op::ShiftRight => {
            // The count is read RAW (an exact anonymous value, not a value of
            // the landed type); an out-of-range count is proof-or-policy
            // territory (float semantics F8) -- defer rather than pick a
            // semantics here.
            if b < 0 || b >= width as i64 {
                return None;
            }
            let count = b as u32;
            if matches!(operator, Op::ShiftLeft) {
                // `x << n` is x * 2^n at the value face. Land the exact
                // widened result under the selected policy: Wrapping wraps,
                // Saturating clamps, and Trapping preserves the overflow for
                // the static-store trap path. The count face was checked
                // independently above.
                land_result(left << count, landing)
            } else {
                // Arithmetic i128 shift IS the landed shift: signed values
                // are sign-extended (arithmetic), unsigned values are
                // non-negative (logical automatically).
                land_result(left >> count, landing)
            }
        }
        Op::BitwiseAnd => land_result(left & right, landing),
        Op::BitwiseOr => land_result(left | right, landing),
        Op::BitwiseXor => land_result(left ^ right, landing),
        // Normalized mathematical values compare correctly for every landed
        // signedness -- this is the comparison face of the same disease.
        Op::Equal => Some(Expression::Boolean(left == right)),
        Op::NotEqual => Some(Expression::Boolean(left != right)),
        Op::Greater => Some(Expression::Boolean(left > right)),
        Op::GreaterOrEqual => Some(Expression::Boolean(left >= right)),
        Op::Less => Some(Expression::Boolean(left < right)),
        Op::LessOrEqual => Some(Expression::Boolean(left <= right)),
        Op::And | Op::Or => None,
    }
}

pub(super) fn fold_binary_expression(
    operator: BinaryOperator,
    left: Expression,
    right: Expression,
    landing: Option<ValueLanding>,
) -> Expression {
    use BinaryOperator as Op;

    let float_landing = match landing {
        Some(ValueLanding::Float(landing)) => Some(landing),
        _ => float_landing_from_literals(&left, &right),
    };
    if let Some(landing) = float_landing
        && let (Expression::Float(left_literal), Expression::Float(right_literal)) = (&left, &right)
        && let Some(folded) = fold_float_binary(operator, left_literal, right_literal, landing)
    {
        return folded;
    }

    // A landing also DERIVES from a LANDED operand (CR3, ch5 two-phase law:
    // the type rides ON the constant): at an anonymous destination (argument
    // and index positions thread None) a substituted stamped local still
    // folds at ITS OWN landed type -- one witness, left first, the signedness
    // probe's exact discipline. Two anonymous literals keep the transitional
    // window below.
    let landing = match landing {
        Some(ValueLanding::Integer(landing)) => Some(landing),
        _ => None,
    }
    .or_else(|| match (&left, &right) {
        (Expression::Integer(a), _) if a.landing().is_some() => {
            a.landing().and_then(IntegerLanding::from_carrier_landing)
        }
        (_, Expression::Integer(b)) => b.landing().and_then(IntegerLanding::from_carrier_landing),
        _ => None,
    });

    // The landed path (CM2) owns integer-literal folds when the expression's
    // landed type is known: fold at that type's width/signedness/domain, or
    // leave the op for the domain-aware runtime lowering. And/Or stay on the
    // boolean path below.
    if let Some(landing) = landing
        && !matches!(operator, Op::And | Op::Or)
        && let (Expression::Integer(a), Expression::Integer(b)) = (&left, &right)
        && let Some((a, b)) = literal_pair(a, b)
    {
        return match fold_landed(operator, a, b, landing) {
            Some(folded) => folded,
            None => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        };
    }

    match operator {
        Op::And => boolean_and(left, right),
        Op::Or => boolean_or(left, right),
        Op::Equal | Op::NotEqual => {
            if let Expression::Boolean(flag) = right {
                let positive = matches!(operator, Op::Equal) == flag;
                return if positive { left } else { boolean_not(left) };
            }
            if let Expression::Boolean(flag) = left {
                let positive = matches!(operator, Op::Equal) == flag;
                return if positive { right } else { boolean_not(right) };
            }
            // NOTE: the REFLEXIVE fold (structurally-equal non-literal
            // operands) is NOT here -- it lives in
            // `simplify_binary_expression`, TYPE-GATED, because
            // `x == x -> true` / `x != x -> false` is an INVALID identity for
            // floats (IEEE: NaN != NaN is TRUE; the canonical isNaN idiom
            // `f != f` was silently folded to `false` before the gate).
            match operator {
                Op::Equal => match (&left, &right) {
                    (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a == b),
                    (Expression::Integer(a), Expression::Integer(b))
                        if literal_pair(a, b).is_some() =>
                    {
                        let (a, b) = literal_pair(a, b).expect("guard checked");
                        Expression::Boolean(a == b)
                    }
                    (Expression::String(a), Expression::String(b)) => Expression::Boolean(a == b),
                    _ => Expression::Binary(Box::new(BinaryExpression {
                        left,
                        operator,
                        right,
                    })),
                },
                Op::NotEqual => match (&left, &right) {
                    (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a != b),
                    (Expression::Integer(a), Expression::Integer(b))
                        if literal_pair(a, b).is_some() =>
                    {
                        let (a, b) = literal_pair(a, b).expect("guard checked");
                        Expression::Boolean(a != b)
                    }
                    (Expression::String(a), Expression::String(b)) => Expression::Boolean(a != b),
                    _ => Expression::Binary(Box::new(BinaryExpression {
                        left,
                        operator,
                        right,
                    })),
                },
                _ => unreachable!(),
            }
        }
        Op::Greater => fold_integer_compare(left, right, |a, b| a > b, operator),
        Op::GreaterOrEqual => fold_integer_compare(left, right, |a, b| a >= b, operator),
        Op::Less => fold_integer_compare(left, right, |a, b| a < b, operator),
        Op::LessOrEqual => fold_integer_compare(left, right, |a, b| a <= b, operator),
        // Wrapping arithmetic so const-folding a value that overflows i64 (`9e18 +
        // 9e18`, `4e9 * 4e9`) does NOT panic the compiler ("attempt to add/multiply
        // with overflow"). Overflow into an EXACT domain is caught earlier by the
        // decision-17 obligation (validation, before this backend fold); the folder's
        // job is only to compute a value without crashing, and i64 wrapping matches
        // native execution.
        Op::Add => fold_integer_math(left, right, |a, b| a.wrapping_add(b), operator),
        Op::Subtract => fold_integer_math(left, right, |a, b| a.wrapping_sub(b), operator),
        Op::Multiply => fold_integer_math(left, right, |a, b| a.wrapping_mul(b), operator),
        // `wrapping_div`/`wrapping_rem` so `i64::MIN / -1` (reachable as
        // `(1 << 63) / -1`) does not panic; a zero divisor and any oversize
        // operand are left unfolded.
        Op::Divide => fold_integer_division(left, right, operator, i64::wrapping_div),
        Op::Modulo => fold_integer_division(left, right, operator, i64::wrapping_rem),

        Op::ShiftLeft => fold_integer_shift(left, right, operator, i64::checked_shl),
        Op::ShiftRight => fold_integer_shift(left, right, operator, i64::checked_shr),
        Op::BitwiseAnd => fold_integer_math(left, right, |a, b| a & b, operator),
        Op::BitwiseOr => fold_integer_math(left, right, |a, b| a | b, operator),
        Op::BitwiseXor => fold_integer_math(left, right, |a, b| a ^ b, operator),
    }
}

fn float_landing_from_literals(left: &Expression, right: &Expression) -> Option<FloatLanding> {
    let format = match (left, right) {
        (Expression::Float(literal), _) if literal.landing().is_some() => literal.landing(),
        (_, Expression::Float(literal)) => literal.landing(),
        _ => None,
    }?;
    Some(FloatLanding {
        format,
        domain: ArithmeticDomain::Exact,
    })
}

fn fold_float_binary(
    operator: BinaryOperator,
    left: &FloatLiteral,
    right: &FloatLiteral,
    landing: FloatLanding,
) -> Option<Expression> {
    use BinaryOperator as Op;

    let format = match landing.format {
        FloatFormat::F32 => SemanticFloatFormat::BINARY32,
        FloatFormat::F64 => SemanticFloatFormat::BINARY64,
    };
    let left = FloatSemantics::from_decimal(format, left.text())?;
    let right = FloatSemantics::from_decimal(format, right.text())?;
    let arithmetic = match operator {
        Op::Add => Some((FloatSemantics::add(format, &left, &right), false)),
        Op::Subtract => Some((FloatSemantics::subtract(format, &left, &right), false)),
        Op::Multiply => Some((FloatSemantics::multiply(format, &left, &right), false)),
        Op::Divide => Some((FloatSemantics::divide(format, &left, &right), true)),
        _ => None,
    };
    if let Some((meaning, division)) = arithmetic {
        let meaning = match landing.domain {
            ArithmeticDomain::Exact => meaning,
            ArithmeticDomain::Saturating if division => {
                FloatSemantics::apply_saturating_divide_policy(format, &left, &right, meaning)
            }
            ArithmeticDomain::Saturating => {
                FloatSemantics::apply_saturating_policy(format, &[&left, &right], meaning)
            }
            ArithmeticDomain::Trapping => FloatSemantics::apply_trapping_policy(meaning).ok()?,
            ArithmeticDomain::Wrapping => return None,
        };
        let value = meaning.to_interpreter_value(format);
        return Some(Expression::Float(
            FloatLiteral::from_f64(value).with_landing(landing.format),
        ));
    }

    Some(Expression::Boolean(match operator {
        Op::Equal => FloatSemantics::equal(&left, &right),
        Op::NotEqual => FloatSemantics::not_equal(&left, &right),
        Op::Less => FloatSemantics::less(&left, &right),
        Op::LessOrEqual => FloatSemantics::less_or_equal(&left, &right),
        Op::Greater => FloatSemantics::greater(&left, &right),
        Op::GreaterOrEqual => FloatSemantics::greater_or_equal(&left, &right),
        _ => return None,
    }))
}

/// Fold a constant SHIFT (`1 << 100`) only when the shift amount is a valid,
/// in-range count. `i64::checked_shl`/`checked_shr` return `None` for an amount
/// >= 64, and a negative amount fails the `u32` conversion -- in both cases the
/// naive `a << b` would PANIC the compiler ("attempt to shift with overflow"), so
/// we leave the expression unfolded for the backend/runtime (whose out-of-range
/// shift semantics are a separate, target-defined question) instead of crashing.
fn fold_integer_shift(
    left: Expression,
    right: Expression,
    operator: BinaryOperator,
    operation: impl FnOnce(i64, u32) -> Option<i64>,
) -> Expression {
    if let (Expression::Integer(a), Expression::Integer(b)) = (&left, &right)
        && let Some((a, b)) = literal_pair(a, b)
        && let Ok(amount) = u32::try_from(b)
        && let Some(result) = operation(a, amount)
    {
        return Expression::Integer(IntegerLiteral::from_value(result));
    }
    Expression::Binary(Box::new(BinaryExpression {
        left,
        operator,
        right,
    }))
}

fn fold_integer_math(
    left: Expression,
    right: Expression,
    operation: impl FnOnce(i64, i64) -> i64,
    operator: BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => match literal_pair(a, b) {
            Some((a, b)) => Expression::Integer(IntegerLiteral::from_value(operation(a, b))),
            None => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        },
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator,
            right,
        })),
    }
}

fn fold_integer_division(
    left: Expression,
    right: Expression,
    operator: BinaryOperator,
    operation: impl FnOnce(i64, i64) -> i64,
) -> Expression {
    if let (Expression::Integer(a), Expression::Integer(b)) = (&left, &right)
        && let Some((a, b)) = literal_pair(a, b)
        && b != 0
    {
        return Expression::Integer(IntegerLiteral::from_value(operation(a, b)));
    }
    Expression::Binary(Box::new(BinaryExpression {
        left,
        operator,
        right,
    }))
}

fn fold_integer_compare(
    left: Expression,
    right: Expression,
    comparison: impl FnOnce(i64, i64) -> bool,
    operator: BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => match literal_pair(a, b) {
            Some((a, b)) => Expression::Boolean(comparison(a, b)),
            None => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        },
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator,
            right,
        })),
    }
}

/// Depth budget for the mutually recursive boolean simplification family
/// (`boolean_and` distribute-over-Or <-> `boolean_or` <->
/// `factor_common_conjuncts` <-> `boolean_not`). The distribution rewrite
/// DOUBLES the tree per level (DNF expansion is exponential), so a deep
/// accumulated disjunction -- the pending length_reverse repro built one
/// through a proof machine's citation sub-state -- ran the family off the
/// compile-thread stack. Past the budget every rule falls back to RAW
/// And/Or/Not node construction, which is always semantically correct
/// (the rewrites are simplifications, never obligations).
const BOOLEAN_SIMPLIFY_DEPTH_BUDGET: usize = 256;

/// Node budget for the DISTRIBUTE-over-Or rewrite specifically: distribution
/// is the exponential rule (it clones one side into both arms of the other),
/// so it only runs on SMALL trees. Bigger trees keep the raw And shape --
/// semantically identical, and the accumulated guards the length_reverse
/// repro built stay linear instead of exploding into a DNF (the depth budget
/// alone converted that crash into a hang; the size gate removes the work).
const BOOLEAN_DISTRIBUTION_NODE_BUDGET: usize = 96;

/// Iterative (explicit-stack) node count, capped: returns `None` once the
/// count exceeds `budget` -- never recurses, so it is safe on trees deep
/// enough to have overflowed the simplifier itself.
fn expression_nodes_within(expression: &Expression, budget: usize) -> bool {
    let mut count = 0usize;
    let mut stack: Vec<&Expression> = vec![expression];
    while let Some(node) = stack.pop() {
        count += 1;
        if count > budget {
            return false;
        }
        match node {
            Expression::Binary(binary) => {
                stack.push(&binary.left);
                stack.push(&binary.right);
            }
            Expression::Unary(unary) => stack.push(&unary.operand),
            Expression::Mutable(inner) => stack.push(inner),
            _ => {}
        }
    }
    true
}

pub(super) fn boolean_and(left: Expression, right: Expression) -> Expression {
    boolean_and_at(left, right, 0)
}

fn boolean_and_at(left: Expression, right: Expression, depth: usize) -> Expression {
    if depth >= BOOLEAN_SIMPLIFY_DEPTH_BUDGET {
        return Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::And,
            right,
        }));
    }
    let distribution_fits = |a: &Expression, b: &Expression| {
        expression_nodes_within(a, BOOLEAN_DISTRIBUTION_NODE_BUDGET)
            && expression_nodes_within(b, BOOLEAN_DISTRIBUTION_NODE_BUDGET)
    };
    if let Expression::Binary(binary) = &left
        && binary.operator == BinaryOperator::Or
        && distribution_fits(&left, &right)
    {
        return boolean_or_at(
            boolean_and_at(binary.left.clone(), right.clone(), depth + 1),
            boolean_and_at(binary.right.clone(), right, depth + 1),
            depth + 1,
        );
    }

    if let Expression::Binary(binary) = &right
        && binary.operator == BinaryOperator::Or
        && distribution_fits(&left, &right)
    {
        return boolean_or_at(
            boolean_and_at(left.clone(), binary.left.clone(), depth + 1),
            boolean_and_at(left, binary.right.clone(), depth + 1),
            depth + 1,
        );
    }

    if let Some(simplified) = simplify_comparison_conjunction(&left, &right) {
        return simplified;
    }

    match (&left, &right) {
        (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => {
            Expression::Boolean(false)
        }
        (Expression::Boolean(true), _) => right,
        (_, Expression::Boolean(true)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::And,
            right,
        })),
    }
}

pub(super) fn boolean_or(left: Expression, right: Expression) -> Expression {
    boolean_or_at(left, right, 0)
}

fn boolean_or_at(left: Expression, right: Expression, depth: usize) -> Expression {
    if depth >= BOOLEAN_SIMPLIFY_DEPTH_BUDGET {
        return Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::Or,
            right,
        }));
    }
    if let Some(simplified) = simplify_comparison_disjunction(&left, &right) {
        return simplified;
    }

    if let Some(factored) = factor_common_conjuncts(&left, &right, depth) {
        return factored;
    }

    match (&left, &right) {
        (Expression::Boolean(true), _) | (_, Expression::Boolean(true)) => {
            Expression::Boolean(true)
        }
        (Expression::Boolean(false), _) => right,
        (_, Expression::Boolean(false)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::Or,
            right,
        })),
    }
}

fn factor_common_conjuncts(
    left: &Expression,
    right: &Expression,
    depth: usize,
) -> Option<Expression> {
    let mut left_conjuncts = Vec::new();
    let mut right_conjuncts = Vec::new();
    collect_conjuncts(left, &mut left_conjuncts);
    collect_conjuncts(right, &mut right_conjuncts);

    let mut common = Vec::new();
    let mut remaining_right = right_conjuncts;

    for left_conjunct in left_conjuncts {
        if let Some(index) = remaining_right
            .iter()
            .position(|candidate| expressions_equivalent(left_conjunct, candidate))
        {
            common.push(left_conjunct.clone());
            remaining_right.remove(index);
        }
    }

    if common.is_empty() {
        return None;
    }

    let mut remaining_left = Vec::new();
    collect_unique_non_common_conjuncts(left, &common, &mut remaining_left);

    let left_rest = combine_conjuncts(remaining_left);
    let right_rest = combine_conjuncts(remaining_right.into_iter().cloned().collect::<Vec<_>>());
    let mut factored = boolean_or_at(left_rest, right_rest, depth + 1);
    for conjunct in common.into_iter().rev() {
        // NOT `boolean_and`: that distributes the conjunct back over the
        // disjunction it was just factored out of, and `boolean_or` would
        // then factor it again -- non-terminating mutual recursion (first
        // reachable through mixed-shape equality, whose synthesized
        // `common-field compares && (case arms...)` repeats the common
        // compares across every disjunction arm).
        factored = conjoin_without_distribution(conjunct, factored);
    }
    Some(factored)
}

/// `boolean_and` minus the distribute-over-`Or` rewrite: the constant,
/// duplicate, and comparison-conjunction rules only, never recursing into
/// `boolean_or`. Used where an And node must wrap a disjunction AS-IS
/// (re-attaching factored common conjuncts).
fn conjoin_without_distribution(left: Expression, right: Expression) -> Expression {
    if let Some(simplified) = simplify_comparison_conjunction(&left, &right) {
        return simplified;
    }

    match (&left, &right) {
        (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => {
            Expression::Boolean(false)
        }
        (Expression::Boolean(true), _) => right,
        (_, Expression::Boolean(true)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::And,
            right,
        })),
    }
}

fn simplify_comparison_conjunction(left: &Expression, right: &Expression) -> Option<Expression> {
    let left_compare = parse_integer_comparison(left)?;
    let right_compare = parse_integer_comparison(right)?;

    if !expressions_equivalent(left_compare.subject, right_compare.subject) {
        return None;
    }

    if left_compare.operator == right_compare.operator && left_compare.value == right_compare.value
    {
        return Some(left.clone());
    }

    let mut lower_bound = None;
    let mut upper_bound = None;

    for comparison in [left_compare, right_compare] {
        match comparison.operator {
            BinaryOperator::Greater => {
                lower_bound = tighten_lower_bound(lower_bound, comparison.value, false);
            }
            BinaryOperator::GreaterOrEqual => {
                lower_bound = tighten_lower_bound(lower_bound, comparison.value, true);
            }
            BinaryOperator::Less => {
                upper_bound = tighten_upper_bound(upper_bound, comparison.value, false);
            }
            BinaryOperator::LessOrEqual => {
                upper_bound = tighten_upper_bound(upper_bound, comparison.value, true);
            }
            _ => return None,
        }
    }

    if let (Some((lower, lower_inclusive)), Some((upper, upper_inclusive))) =
        (lower_bound, upper_bound)
    {
        let impossible =
            lower > upper || (lower == upper && (!lower_inclusive || !upper_inclusive));
        if impossible {
            return Some(Expression::Boolean(false));
        }
    }

    if lower_bound.is_some() && upper_bound.is_none() {
        let (value, inclusive) = lower_bound?;
        return Some(Expression::Binary(Box::new(BinaryExpression {
            left: left_compare.subject.clone(),
            operator: if inclusive {
                BinaryOperator::GreaterOrEqual
            } else {
                BinaryOperator::Greater
            },
            right: Expression::Integer(IntegerLiteral::from_value(value)),
        })));
    }

    if upper_bound.is_some() && lower_bound.is_none() {
        let (value, inclusive) = upper_bound?;
        return Some(Expression::Binary(Box::new(BinaryExpression {
            left: left_compare.subject.clone(),
            operator: if inclusive {
                BinaryOperator::LessOrEqual
            } else {
                BinaryOperator::Less
            },
            right: Expression::Integer(IntegerLiteral::from_value(value)),
        })));
    }

    None
}

fn simplify_comparison_disjunction(left: &Expression, right: &Expression) -> Option<Expression> {
    let left_compare = parse_integer_comparison(left)?;
    let right_compare = parse_integer_comparison(right)?;

    if !expressions_equivalent(left_compare.subject, right_compare.subject) {
        return None;
    }

    if left_compare.operator == right_compare.operator && left_compare.value == right_compare.value
    {
        return Some(left.clone());
    }

    use BinaryOperator as Op;
    match (left_compare.operator, right_compare.operator) {
        (Op::Greater, Op::Greater)
        | (Op::Greater, Op::GreaterOrEqual)
        | (Op::GreaterOrEqual, Op::Greater)
        | (Op::GreaterOrEqual, Op::GreaterOrEqual) => {
            let (value, inclusive) = loosen_lower_bound_for_disjunction(
                (
                    left_compare.value,
                    left_compare.operator == Op::GreaterOrEqual,
                ),
                (
                    right_compare.value,
                    right_compare.operator == Op::GreaterOrEqual,
                ),
            );
            Some(Expression::Binary(Box::new(BinaryExpression {
                left: left_compare.subject.clone(),
                operator: if inclusive {
                    Op::GreaterOrEqual
                } else {
                    Op::Greater
                },
                right: Expression::Integer(IntegerLiteral::from_value(value)),
            })))
        }
        (Op::Less, Op::Less)
        | (Op::Less, Op::LessOrEqual)
        | (Op::LessOrEqual, Op::Less)
        | (Op::LessOrEqual, Op::LessOrEqual) => {
            let (value, inclusive) = loosen_upper_bound_for_disjunction(
                (left_compare.value, left_compare.operator == Op::LessOrEqual),
                (
                    right_compare.value,
                    right_compare.operator == Op::LessOrEqual,
                ),
            );
            Some(Expression::Binary(Box::new(BinaryExpression {
                left: left_compare.subject.clone(),
                operator: if inclusive { Op::LessOrEqual } else { Op::Less },
                right: Expression::Integer(IntegerLiteral::from_value(value)),
            })))
        }
        (Op::Greater, Op::LessOrEqual)
        | (Op::LessOrEqual, Op::Greater)
        | (Op::GreaterOrEqual, Op::Less)
        | (Op::Less, Op::GreaterOrEqual) => {
            if comparisons_cover_all_integers(left_compare, right_compare) {
                Some(Expression::Boolean(true))
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct IntegerComparison<'expression> {
    subject: &'expression Expression,
    operator: BinaryOperator,
    value: i64,
}

fn parse_integer_comparison(expression: &Expression) -> Option<IntegerComparison<'_>> {
    let Expression::Binary(binary) = expression else {
        return None;
    };

    let operator = match binary.operator {
        BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual => binary.operator,
        _ => return None,
    };

    if let Expression::Integer(value) = &binary.right {
        return Some(IntegerComparison {
            subject: &binary.left,
            operator,
            value: value.value_i64()?,
        });
    }

    if let Expression::Integer(value) = &binary.left {
        let flipped_operator = match binary.operator {
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            _ => unreachable!(),
        };

        return Some(IntegerComparison {
            subject: &binary.right,
            operator: flipped_operator,
            value: value.value_i64()?,
        });
    }

    None
}

fn tighten_lower_bound(
    current: Option<(i64, bool)>,
    candidate_value: i64,
    candidate_inclusive: bool,
) -> Option<(i64, bool)> {
    match current {
        None => Some((candidate_value, candidate_inclusive)),
        Some((current_value, current_inclusive)) => {
            if candidate_value > current_value {
                Some((candidate_value, candidate_inclusive))
            } else if candidate_value < current_value {
                Some((current_value, current_inclusive))
            } else {
                Some((current_value, current_inclusive && candidate_inclusive))
            }
        }
    }
}

fn tighten_upper_bound(
    current: Option<(i64, bool)>,
    candidate_value: i64,
    candidate_inclusive: bool,
) -> Option<(i64, bool)> {
    match current {
        None => Some((candidate_value, candidate_inclusive)),
        Some((current_value, current_inclusive)) => {
            if candidate_value < current_value {
                Some((candidate_value, candidate_inclusive))
            } else if candidate_value > current_value {
                Some((current_value, current_inclusive))
            } else {
                Some((current_value, current_inclusive && candidate_inclusive))
            }
        }
    }
}

fn loosen_lower_bound_for_disjunction(left: (i64, bool), right: (i64, bool)) -> (i64, bool) {
    match left.0.cmp(&right.0) {
        std::cmp::Ordering::Less => left,
        std::cmp::Ordering::Greater => right,
        std::cmp::Ordering::Equal => (left.0, left.1 || right.1),
    }
}

fn loosen_upper_bound_for_disjunction(left: (i64, bool), right: (i64, bool)) -> (i64, bool) {
    match left.0.cmp(&right.0) {
        std::cmp::Ordering::Less => right,
        std::cmp::Ordering::Greater => left,
        std::cmp::Ordering::Equal => (left.0, left.1 || right.1),
    }
}

fn comparisons_cover_all_integers(
    left: IntegerComparison<'_>,
    right: IntegerComparison<'_>,
) -> bool {
    use BinaryOperator as Op;

    let (lower, upper) = match (left.operator, right.operator) {
        (Op::Greater, Op::LessOrEqual)
        | (Op::GreaterOrEqual, Op::Less)
        | (Op::Greater, Op::Less)
        | (Op::GreaterOrEqual, Op::LessOrEqual) => (left, right),
        (Op::LessOrEqual, Op::Greater)
        | (Op::Less, Op::GreaterOrEqual)
        | (Op::Less, Op::Greater)
        | (Op::LessOrEqual, Op::GreaterOrEqual) => (right, left),
        _ => return false,
    };

    let lower_min = match lower.operator {
        Op::Greater => lower.value + 1,
        Op::GreaterOrEqual => lower.value,
        _ => return false,
    };
    let upper_max = match upper.operator {
        Op::Less => upper.value - 1,
        Op::LessOrEqual => upper.value,
        _ => return false,
    };
    lower_min <= upper_max + 1
}

fn collect_conjuncts<'a>(expression: &'a Expression, output: &mut Vec<&'a Expression>) {
    if let Expression::Binary(binary) = expression
        && binary.operator == BinaryOperator::And
    {
        collect_conjuncts(&binary.left, output);
        collect_conjuncts(&binary.right, output);
        return;
    }
    output.push(expression);
}

fn collect_unique_non_common_conjuncts(
    expression: &Expression,
    common: &[Expression],
    output: &mut Vec<Expression>,
) {
    let mut conjuncts = Vec::new();
    collect_conjuncts(expression, &mut conjuncts);
    let mut remaining_common = common.to_vec();
    for conjunct in conjuncts {
        if let Some(index) = remaining_common
            .iter()
            .position(|candidate| expressions_equivalent(conjunct, candidate))
        {
            remaining_common.remove(index);
        } else {
            output.push(conjunct.clone());
        }
    }
}

fn combine_conjuncts(conjuncts: Vec<Expression>) -> Expression {
    conjuncts
        .into_iter()
        .reduce(boolean_and)
        .unwrap_or(Expression::Boolean(true))
}

pub(super) fn boolean_not(expression: Expression) -> Expression {
    use BinaryOperator as Op;

    match expression {
        Expression::Boolean(value) => Expression::Boolean(!value),
        Expression::Binary(binary) => {
            let inverted = match binary.operator {
                Op::Equal => Some(Op::NotEqual),
                Op::NotEqual => Some(Op::Equal),
                // Do not complement ordered comparisons here. `Expression`
                // carries no operand type, and the familiar total-order
                // identities are false for IEEE values: when either operand
                // is NaN, both `a < b` and `a >= b` are false. Preserve the
                // boolean negation explicitly so runtime float comparison can
                // retain the unordered leg. A future type-aware integer-only
                // pass may recover the total-order optimization.
                Op::Greater | Op::GreaterOrEqual | Op::Less | Op::LessOrEqual => None,
                Op::And => {
                    return boolean_or(boolean_not(binary.left), boolean_not(binary.right));
                }
                Op::Or => {
                    return boolean_and(boolean_not(binary.left), boolean_not(binary.right));
                }
                Op::Add
                | Op::BitwiseAnd
                | Op::BitwiseOr
                | Op::BitwiseXor
                | Op::Divide
                | Op::Modulo
                | Op::Multiply
                | Op::ShiftLeft
                | Op::ShiftRight
                | Op::Subtract => None,
            };

            if let Some(operator) = inverted {
                Expression::Binary(Box::new(BinaryExpression {
                    left: binary.left,
                    operator,
                    right: binary.right,
                }))
            } else {
                Expression::Binary(Box::new(BinaryExpression {
                    left: Expression::Binary(binary),
                    operator: Op::Equal,
                    right: Expression::Boolean(false),
                }))
            }
        }
        other => Expression::Binary(Box::new(BinaryExpression {
            left: other,
            operator: BinaryOperator::Equal,
            right: Expression::Boolean(false),
        })),
    }
}

pub(super) fn expressions_equivalent(left: &Expression, right: &Expression) -> bool {
    if let Some(are_equivalent) = expression_paths_equivalent(left, right) {
        return are_equivalent;
    }

    match (left, right) {
        (Expression::ArrayLiteral(left), Expression::ArrayLiteral(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| expressions_equivalent(left, right))
        }
        (Expression::Binary(left), Expression::Binary(right)) => {
            left.operator == right.operator
                && expressions_equivalent(&left.left, &right.left)
                && expressions_equivalent(&left.right, &right.right)
        }
        (Expression::Boolean(left), Expression::Boolean(right)) => left == right,
        (Expression::Call(left), Expression::Call(right)) => {
            left.target == right.target
                && left.arguments.len() == right.arguments.len()
                && left
                    .receiver
                    .as_deref()
                    .zip(right.receiver.as_deref())
                    .map(|(left, right)| expressions_equivalent(left, right))
                    .unwrap_or(left.receiver.is_none() && right.receiver.is_none())
                && left
                    .arguments
                    .iter()
                    .zip(right.arguments.iter())
                    .all(|(left, right)| expressions_equivalent(left, right))
        }
        (Expression::Cast(left), Expression::Cast(right)) => {
            left.target_type == right.target_type
                && left.domain == right.domain
                && left.form == right.form
                && expressions_equivalent(&left.value, &right.value)
        }
        (Expression::Float(left), Expression::Float(right)) => left == right,
        (Expression::Indexed(left), Expression::Indexed(right)) => {
            expressions_equivalent(&left.collection, &right.collection)
                && expressions_equivalent(&left.index, &right.index)
        }
        (Expression::Integer(left), Expression::Integer(right)) => left == right,
        (Expression::Member(left), Expression::Member(right)) => {
            ((left.member_symbol.is_valid()
                && right.member_symbol.is_valid()
                && left.member_symbol == right.member_symbol)
                || (!left.member_symbol.is_valid()
                    && !right.member_symbol.is_valid()
                    && left.member == right.member))
                && expressions_equivalent(&left.receiver, &right.receiver)
        }
        (Expression::Mutable(left), Expression::Mutable(right)) => {
            expressions_equivalent(left, right)
        }
        (Expression::Name(left), Expression::Name(right)) => {
            left.symbol().is_valid() && right.symbol().is_valid() && left.symbol() == right.symbol()
        }
        (Expression::StructLiteral(left), Expression::StructLiteral(right)) => {
            left.type_name == right.type_name
                && left.fields.len() == right.fields.len()
                && left
                    .fields
                    .iter()
                    .zip(right.fields.iter())
                    .all(|(left, right)| {
                        left.name == right.name && expressions_equivalent(&left.value, &right.value)
                    })
        }
        (Expression::String(left), Expression::String(right)) => left == right,
        _ => false,
    }
}

fn expression_paths_equivalent(left: &Expression, right: &Expression) -> Option<bool> {
    let left_count = expression_path_segment_count(left)?;
    let right_count = expression_path_segment_count(right)?;

    if left_count != right_count {
        return Some(false);
    }

    Some((0..left_count).all(|index| {
        let left_symbol = expression_path_segment_symbol(left, index);
        let right_symbol = expression_path_segment_symbol(right, index);
        left_symbol.is_valid() && right_symbol.is_valid() && left_symbol == right_symbol
    }))
}

fn expression_path_segment_count(expression: &Expression) -> Option<usize> {
    match expression {
        Expression::Name(path) => Some(path.len()),
        Expression::Member(member) => Some(expression_path_segment_count(&member.receiver)? + 1),
        _ => None,
    }
}

fn expression_path_segment_symbol(expression: &Expression, index: usize) -> SymbolHandle {
    match expression {
        Expression::Name(path) => path.member_symbol(index),
        Expression::Member(member) => {
            let Some(receiver_count) = expression_path_segment_count(&member.receiver) else {
                return SymbolHandle::invalid();
            };
            if index == receiver_count {
                member.member_symbol
            } else {
                expression_path_segment_symbol(&member.receiver, index)
            }
        }
        _ => SymbolHandle::invalid(),
    }
}

#[cfg(test)]
mod tests {
    use super::{FloatLanding, ValueLanding, fold_binary_expression};
    use psi_checked_trees::expression::{BinaryOperator, Expression};
    use psi_numerics::arithmetic::ArithmeticDomain;
    use psi_numerics::literals::{FloatFormat, FloatLiteral};

    fn float(text: &str, format: FloatFormat) -> Expression {
        Expression::Float(
            FloatLiteral::parse(text)
                .expect("test float literal")
                .with_landing(format),
        )
    }

    fn fold_f32(operator: BinaryOperator, left: Expression, right: Expression) -> FloatLiteral {
        let Expression::Float(result) = fold_binary_expression(
            operator,
            left,
            right,
            Some(ValueLanding::Float(FloatLanding {
                format: FloatFormat::F32,
                domain: ArithmeticDomain::Exact,
            })),
        ) else {
            panic!("constant float arithmetic must fold to a float literal");
        };
        result
    }

    #[test]
    fn binary32_constant_fold_rounds_at_binary32_precision() {
        let result = fold_f32(
            BinaryOperator::Add,
            float("16777216.0", FloatFormat::F32),
            float("1.0", FloatFormat::F32),
        );

        assert_eq!(result.value_f32().to_bits(), 16_777_216.0f32.to_bits());
        assert_eq!(result.landing(), Some(FloatFormat::F32));
    }

    #[test]
    fn binary32_constant_fold_preserves_nan_and_negative_zero() {
        let nan = fold_f32(
            BinaryOperator::Divide,
            float("0.0", FloatFormat::F32),
            float("0.0", FloatFormat::F32),
        );
        assert!(nan.value_f32().is_nan());

        let negative_one = fold_f32(
            BinaryOperator::Subtract,
            float("0.0", FloatFormat::F32),
            float("1.0", FloatFormat::F32),
        );
        let negative_zero = fold_f32(
            BinaryOperator::Divide,
            float("0.0", FloatFormat::F32),
            Expression::Float(negative_one),
        );
        assert_eq!(negative_zero.value_f32().to_bits(), (-0.0f32).to_bits());
    }
}
