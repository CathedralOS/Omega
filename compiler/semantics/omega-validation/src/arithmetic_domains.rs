//! Arithmetic-domain checks (frozen decision 17). Two rules, both OPERAND-driven
//! (the domain lives on each value's type, not the assignment target):
//!
//! - **S2 mixed-domain rejection**: a binary arithmetic op whose operands carry
//!   DIFFERENT explicit domains is illegal (cross with an `as` cast). Literals are
//!   neutral and adopt the other operand's domain.
//! - **S3 exact-by-default enforcement**: an `Exact` (default, undomained) integer
//!   `+`/`-`/`*` must be PROVEN not to overflow its type, else it is a compile
//!   error directing the user to widen (`as`) or pick a domain. Wrapping/
//!   Saturating/Trapping ops have defined overflow behaviour and are exempt.
//!
//! Operand ranges come from declared type bounds (an `i32` is its full range),
//! narrowed for literals to their exact value; the interval engine then bounds
//! the result and checks it fits the result type. (Range-constraint and
//! loop-bound narrowing -- the ergonomics that keep this from being annotation-
//! hell -- are S4.)

use std::collections::BTreeMap;

use omega_core::arithmetic::ArithmeticDomain;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use crate::places::declared_place_type_raw;

/// S4: build a value environment pre-seeded with the integer bounds a machine's
/// `requires` clause places on its parameters (`requires amount <= 100`). Used to
/// seed the ENTRY state's env so param arithmetic with a declared bound stays
/// exact instead of being forced into a domain. Only simple `param <OP> literal`
/// (and the flipped `literal <OP> param`) comparisons are read; anything else is
/// ignored (sound -- a missing bound just falls back to the type width).
pub(crate) fn requires_value_env(program: &TypedTrees, machine: &Machine) -> ValueEnv {
    let mut bounds: BTreeMap<String, (Option<i64>, Option<i64>)> = BTreeMap::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some((name, low, high)) = comparison_bound(program, *expression) {
                let entry = bounds.entry(name).or_insert((None, None));
                // Intersect across facts: tightest lower (max) and upper (min).
                if let Some(low) = low {
                    entry.0 = Some(entry.0.map_or(low, |existing| existing.max(low)));
                }
                if let Some(high) = high {
                    entry.1 = Some(entry.1.map_or(high, |existing| existing.min(high)));
                }
            }
        }
    }
    let mut env = ValueEnv::new();
    for (name, (low, high)) in bounds {
        env.set(name, Interval { low, high });
    }
    env
}

/// Read a `requires` comparison as `(param_name, lower, upper)` -- one of the
/// bounds is `None` (open). `None` when the fact is not a simple
/// `name <OP> literal` / `literal <OP> name` integer comparison.
fn comparison_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(String, Option<i64>, Option<i64>)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if let (Some(name), Some(literal)) = (
        place_path(program, binary.left),
        literal_i64(program, binary.right),
    ) {
        return Some(bound_from(name, binary.operator, literal, true));
    }
    if let (Some(name), Some(literal)) = (
        place_path(program, binary.right),
        literal_i64(program, binary.left),
    ) {
        return Some(bound_from(name, binary.operator, literal, false));
    }
    None
}

/// Convert a single `name <OP> literal` (`name_on_left`) or `literal <OP> name`
/// comparison into a one-sided (or, for `==`, two-sided) bound.
fn bound_from(
    name: String,
    operator: BinaryOperator,
    literal: i64,
    name_on_left: bool,
) -> (String, Option<i64>, Option<i64>) {
    // Normalise to `name <OP> literal` by flipping the operator when the name is
    // on the right.
    let operator = if name_on_left {
        operator
    } else {
        match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            other => other,
        }
    };
    let (low, high) = match operator {
        BinaryOperator::LessOrEqual => (None, Some(literal)),
        BinaryOperator::Less => (None, Some(literal.saturating_sub(1))),
        BinaryOperator::GreaterOrEqual => (Some(literal), None),
        BinaryOperator::Greater => (Some(literal.saturating_add(1)), None),
        BinaryOperator::Equal => (Some(literal), Some(literal)),
        _ => (None, None),
    };
    (name, low, high)
}

/// The comparison operator whose truth is the LOGICAL NEGATION of `operator`
/// (`>` ⟺ `<=`, ...). `None` for `==`/`!=` (their negation has no single-interval
/// bound). Used to narrow a transition's FALSE arm by the negated guard.
fn negate_comparison(operator: BinaryOperator) -> Option<BinaryOperator> {
    Some(match operator {
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        _ => return None,
    })
}

/// S4 dominating-guard narrowing: a transition arm fires only when its guard
/// holds, so the arm's argument arithmetic can assume that bound. Returns `base`
/// refined by the arm's guard. The desugared arm guard is `<comparison> ==
/// true|false`; the comparison's bound (negated for the `false` arm) is
/// INTERSECTED with the guarded place's type range so a one-sided guard (`n >
/// 0`) keeps the type's other end (else `n - 1` loses its `u32` upper bound).
/// Only simple `place <OP> literal` comparisons narrow; anything else leaves the
/// env unchanged (sound -- the arm's arithmetic then has to prove on its own).
pub(crate) fn guard_narrowed_env(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    guard: &omega_typed_trees::statement::TransitionGuardNode,
    base: &ValueEnv,
) -> ValueEnv {
    use omega_typed_trees::statement::TransitionGuardNode;
    let mut env = base.clone();
    let TransitionGuardNode::When(guard_expr) = guard else {
        return env;
    };
    let ExpressionNode::Binary(equality) = program.expression_table.expression(*guard_expr) else {
        return env;
    };
    if equality.operator != BinaryOperator::Equal {
        return env;
    }
    let ExpressionNode::Boolean(arm_true) = program.expression_table.expression(equality.right)
    else {
        return env;
    };
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(equality.left)
    else {
        return env;
    };
    // Identify the (place, literal) sides.
    let (place_expr, literal, name_on_left) =
        if let Some(literal) = literal_i64(program, comparison.right) {
            (comparison.left, literal, true)
        } else if let Some(literal) = literal_i64(program, comparison.left) {
            (comparison.right, literal, false)
        } else {
            return env;
        };
    // The `false` arm narrows by the NEGATED comparison.
    let operator = if *arm_true {
        comparison.operator
    } else {
        let Some(negated) = negate_comparison(comparison.operator) else {
            return env;
        };
        negated
    };
    let Some(name) = place_path(program, place_expr) else {
        return env;
    };
    let (_, low, high) = bound_from(name.clone(), operator, literal, name_on_left);
    let guard_interval = Interval { low, high };
    // Intersect with the place's type range to retain the other bound.
    let type_interval = declared_place_type_raw(program, machine, state, place_expr)
        .and_then(|handle| program.primitive_type_reference(handle))
        .and_then(primitive_range);
    let interval = match type_interval {
        Some(type_interval) => guard_interval.intersect(type_interval),
        None => guard_interval,
    };
    env.narrow(name, interval);
    env
}

/// S4 flow-sensitive value environment: the proven interval of each place
/// (`self.field`, local) along the straight-line prefix of a state body. Lets the
/// overflow proof discharge `self.v = 10; self.v += 5` (v is known to be 10, so
/// 15 fits) instead of falling back to the full type range. Conservative: an
/// entry is only present when its value is definitely established on the linear
/// path; on anything we cannot model (a call that may mutate, a branch) the
/// relevant entries are dropped and the place falls back to its type bounds.
#[derive(Debug, Default, Clone)]
pub(crate) struct ValueEnv {
    intervals: BTreeMap<String, Interval>,
}

impl ValueEnv {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Drop all tracked values (after an opaque effect like a call that may
    /// mutate fields through `&mut`, or when leaving the linear prefix).
    pub(crate) fn clear(&mut self) {
        self.intervals.clear();
    }

    fn get(&self, path: &str) -> Option<Interval> {
        self.intervals.get(path).copied()
    }

    fn set(&mut self, path: String, interval: Interval) {
        self.intervals.insert(path, interval);
    }

    /// Intersect a place's tracked interval with `interval` (tightening it).
    /// Used by guard narrowing so an arm's guard refines the env without
    /// discarding a value already proven on the linear path.
    fn narrow(&mut self, path: String, interval: Interval) {
        let merged = match self.intervals.get(&path) {
            Some(existing) => existing.intersect(interval),
            None => interval,
        };
        self.intervals.insert(path, merged);
    }
}

/// Walk a value expression, apply the domain + overflow rules to every nested
/// arithmetic binary, and return the expression's proven interval (so the caller
/// can record it for the assigned place). `owner` describes the site.
/// `target_primitive` is the declared integer type of the value's destination
/// (the local/field/return type). It is the FALLBACK integer type for an
/// otherwise-untyped operand tree -- so a bare-literal computation like
/// `let c: u8 = 200 + 100` is range-checked against `u8` (and rejected) instead
/// of slipping through with no primitive.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_arithmetic_domains(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    target_primitive: Option<PrimitiveType>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Interval {
    if !expression.is_valid() {
        return Interval::UNBOUNDED;
    }
    analyze(
        program,
        machine,
        state,
        expression,
        env,
        target_primitive,
        owner,
        diagnostics,
    )
    .interval
}

/// The straight-line place path an expression denotes (`self.v`, `count`), for
/// the value environment. `None` for non-place expressions.
pub(crate) fn place_path(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.is_empty() {
                return None;
            }
            Some(
                members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = place_path(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        _ => None,
    }
}

/// Record an assignment's proven interval into the environment (decision 17 S4).
/// A place whose path cannot be formed (a complex lvalue) just is not tracked.
pub(crate) fn record_assignment(env: &mut ValueEnv, path: Option<String>, interval: Interval) {
    if let Some(path) = path {
        env.set(path, interval);
    }
}

/// An integer value range with optional (= unbounded) ends; all arithmetic is
/// checked, so an overflowing corner becomes `None` (unbounded) -- which fails
/// the containment test and so is reported as a possible overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Interval {
    low: Option<i64>,
    high: Option<i64>,
}

impl Interval {
    const UNBOUNDED: Interval = Interval {
        low: None,
        high: None,
    };

    pub(crate) fn low(&self) -> Option<i64> {
        self.low
    }

    pub(crate) fn high(&self) -> Option<i64> {
        self.high
    }

    fn constant(value: i64) -> Self {
        Self {
            low: Some(value),
            high: Some(value),
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            low: pair(self.low, other.low, i64::checked_add),
            high: pair(self.high, other.high, i64::checked_add),
        }
    }

    fn subtract(self, other: Self) -> Self {
        // [a,b] - [c,d] = [a-d, b-c]
        Self {
            low: pair(self.low, other.high, i64::checked_sub),
            high: pair(self.high, other.low, i64::checked_sub),
        }
    }

    fn multiply(self, other: Self) -> Self {
        let (Some(a), Some(b), Some(c), Some(d)) = (self.low, self.high, other.low, other.high)
        else {
            return Interval::UNBOUNDED;
        };
        let corners = [
            a.checked_mul(c),
            a.checked_mul(d),
            b.checked_mul(c),
            b.checked_mul(d),
        ];
        if corners.iter().any(Option::is_none) {
            return Interval::UNBOUNDED;
        }
        let values: Vec<i64> = corners.into_iter().flatten().collect();
        Self {
            low: values.iter().min().copied(),
            high: values.iter().max().copied(),
        }
    }

    /// `a % b`: the remainder's magnitude is strictly below the divisor's
    /// magnitude (truncated-division semantics: the remainder takes the
    /// dividend's sign). SOUND only when the divisor is provably nonzero with a
    /// finite magnitude bound; otherwise unbounded (as before). This is what lets
    /// `self.seed % 100` feed exact arithmetic (`% 100` is in `[-99, 99]`)
    /// instead of poisoning the enclosing op with an unbounded operand. The
    /// result interval can only SHRINK relative to the old unbounded value, so it
    /// is strictly permissive for any enclosing overflow check (never a new
    /// rejection).
    fn modulo(self, divisor: Self) -> Self {
        let Some(magnitude) = divisor.nonzero_magnitude_bound() else {
            return Interval::UNBOUNDED;
        };
        let bound = magnitude.saturating_sub(1);
        // Remainder sign follows the dividend: a provably non-negative dividend
        // yields a non-negative remainder, a non-positive one a non-positive
        // remainder, an unknown-sign dividend either sign.
        let low = if self.low.is_some_and(|low| low >= 0) {
            0
        } else {
            -bound
        };
        let high = if self.high.is_some_and(|high| high <= 0) {
            0
        } else {
            bound
        };
        Self {
            low: Some(low),
            high: Some(high),
        }
    }

    /// `a / b`: truncated division by a divisor of magnitude >= 1 never grows the
    /// dividend's magnitude and preserves its sign, so the quotient stays within
    /// the dividend's own bounds widened to include 0 (the quotient can reach 0,
    /// e.g. `small / large`). SOUND only when the divisor is provably nonzero;
    /// else unbounded. Like `modulo`, the result can only shrink, so it is
    /// strictly permissive.
    fn divide(self, divisor: Self) -> Self {
        if divisor.nonzero_magnitude_bound().is_none() {
            return Interval::UNBOUNDED;
        }
        let (Some(low), Some(high)) = (self.low, self.high) else {
            return Interval::UNBOUNDED;
        };
        Self {
            low: Some(low.min(0)),
            high: Some(high.max(0)),
        }
    }

    /// `min(a, b)`: the result is <= both operands and >= the smaller of the two
    /// possible values. Unbounded ends behave as the appropriate infinity (a
    /// `None` low is -inf, a `None` high is +inf), so `min(x, 100)` upper-bounds
    /// at 100 even when `x` is unbounded.
    fn min_with(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
        }
    }

    /// `max(a, b)`: the dual of `min_with` -- `max(0, x)` lower-bounds at 0 even
    /// when `x` is unbounded.
    fn max_with(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            },
        }
    }

    /// `Some(max|b|)` when this interval (a divisor) is finite and provably
    /// excludes 0 -- either entirely positive (`low >= 1`) or entirely negative
    /// (`high <= -1`). `None` otherwise (the divisor may be 0 or is unbounded, so
    /// `%`/`/` cannot be magnitude-bounded soundly).
    fn nonzero_magnitude_bound(self) -> Option<i64> {
        let (low, high) = (self.low?, self.high?);
        if low >= 1 || high <= -1 {
            Some(low.saturating_abs().max(high.saturating_abs()))
        } else {
            None
        }
    }

    /// Tightest interval contained in both (`[max(lows), min(highs)]`, an
    /// unbounded end deferring to the other). Used to intersect a guard bound
    /// with a place's type range.
    fn intersect(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
        }
    }

    /// Does `self` (a type's range) fully contain `inner` (a computed value
    /// range)? An unbounded `inner` end against a bounded `self` end is NOT
    /// contained -- the value might exceed the type.
    fn contains(self, inner: Interval) -> bool {
        let low_ok = match (self.low, inner.low) {
            (Some(bound), Some(value)) => value >= bound,
            (Some(_), None) => false,
            (None, _) => true,
        };
        let high_ok = match (self.high, inner.high) {
            (Some(bound), Some(value)) => value <= bound,
            (Some(_), None) => false,
            (None, _) => true,
        };
        low_ok && high_ok
    }
}

fn pair(left: Option<i64>, right: Option<i64>, op: fn(i64, i64) -> Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(a), Some(b)) => op(a, b),
        _ => None,
    }
}

/// The representable range of an integer primitive. `None` for non-integers
/// (`bool`/`f32`/`f64`/`String`) and for `u64`/`usize` whose maximum exceeds
/// `i64` (their high end is left unbounded -- an over-approximation that still
/// rejects genuine overflow).
fn primitive_range(primitive: PrimitiveType) -> Option<Interval> {
    let (low, high): (Option<i64>, Option<i64>) = match primitive {
        PrimitiveType::I8 => (Some(i8::MIN as i64), Some(i8::MAX as i64)),
        PrimitiveType::U8 => (Some(0), Some(u8::MAX as i64)),
        PrimitiveType::I16 => (Some(i16::MIN as i64), Some(i16::MAX as i64)),
        PrimitiveType::U16 => (Some(0), Some(u16::MAX as i64)),
        PrimitiveType::I32 => (Some(i32::MIN as i64), Some(i32::MAX as i64)),
        PrimitiveType::U32 => (Some(0), Some(u32::MAX as i64)),
        PrimitiveType::I64 | PrimitiveType::Isize => (Some(i64::MIN), Some(i64::MAX)),
        PrimitiveType::U64 | PrimitiveType::Usize => (Some(0), None),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::String => {
            return None;
        }
    };
    Some(Interval { low, high })
}

fn is_arithmetic(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
    )
}

/// The result of analysing an expression for the domain + overflow rules.
struct Analysis {
    /// The arithmetic domain (`None` = neutral: a literal or `bool` result).
    domain: Option<ArithmeticDomain>,
    /// The value range, for the overflow proof obligation.
    interval: Interval,
    /// The integer primitive type, for the overflow range bound (`None` when it
    /// cannot be determined, e.g. a bare literal).
    primitive: Option<PrimitiveType>,
}

const NEUTRAL: Analysis = Analysis {
    domain: None,
    interval: Interval::UNBOUNDED,
    primitive: None,
};

fn analyze(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    target_primitive: Option<PrimitiveType>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let operator = binary.operator;
            let left = analyze(
                program,
                machine,
                state,
                binary.left,
                env,
                target_primitive,
                owner,
                diagnostics,
            );
            let right = analyze(
                program,
                machine,
                state,
                binary.right,
                env,
                target_primitive,
                owner,
                diagnostics,
            );
            if !is_arithmetic(operator) {
                // Comparison / logical `and`/`or`: a `bool` whose integer value is
                // 0 or 1. Its interval is [0, 1] (NOT unbounded) so it does not
                // poison an enclosing arithmetic op -- e.g. the match desugar
                // `d + (s == p) * (v - d)` stays bounded. No arithmetic domain.
                return Analysis {
                    domain: None,
                    interval: Interval {
                        low: Some(0),
                        high: Some(1),
                    },
                    primitive: None,
                };
            }

            // S2: a binary mixing two different explicit domains is illegal.
            if let (Some(left_domain), Some(right_domain)) = (left.domain, right.domain)
                && left_domain != right_domain
            {
                diagnostics.push(Diagnostic::error(format!(
                    "mixed arithmetic domains in {owner}: one operand is `{}` and the other is \
                     `{}`. Decision 17 forbids implicit domain mixing -- cross domains with an \
                     explicit `as` cast, or declare both operands in the same domain.",
                    left_domain.name(),
                    right_domain.name(),
                )));
            }

            let domain = match (left.domain, right.domain) {
                (Some(left_domain), Some(right_domain)) => Some(if left_domain
                    == ArithmeticDomain::Exact
                {
                    right_domain
                } else {
                    left_domain
                }),
                (Some(domain), None) | (None, Some(domain)) => Some(domain),
                (None, None) => None,
            };
            let interval = match operator {
                BinaryOperator::Add => left.interval.add(right.interval),
                BinaryOperator::Subtract => left.interval.subtract(right.interval),
                BinaryOperator::Multiply => left.interval.multiply(right.interval),
                // S4: modulo is bounded by the divisor's magnitude and division
                // never grows the dividend's magnitude -- bounding both lets a
                // `(a % K)` / `(a / K)` result feed exact arithmetic instead of
                // poisoning the enclosing op with an unbounded operand. Neither is
                // overflow-flagged below; the tighter interval is purely a better
                // (still sound) over-approximation for any ENCLOSING op. Shifts
                // stay unbounded.
                BinaryOperator::Modulo => left.interval.modulo(right.interval),
                BinaryOperator::Divide => left.interval.divide(right.interval),
                _ => Interval::UNBOUNDED,
            };
            // Operand primitives win. The destination type is a fallback ONLY when
            // the result is a BOUNDED constant (a bare-literal computation like
            // `let c: u8 = 200 + 100`), so it is range-checked against `c`. An
            // UNbounded result (an unknown operand -- a call result, a param) keeps
            // no primitive and stays unchecked, as before -- the target fallback
            // must not turn "unknown" into a spurious overflow.
            let primitive = left.primitive.or(right.primitive).or_else(|| {
                if interval.low.is_some() && interval.high.is_some() {
                    target_primitive
                } else {
                    None
                }
            });

            // S3: an EXACT (undomained) `+`/`-`/`*` must be provably in range.
            let effective_domain = domain.unwrap_or(ArithmeticDomain::Exact);
            if effective_domain == ArithmeticDomain::Exact
                && matches!(
                    operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                )
                && let Some(primitive) = primitive
                && let Some(range) = primitive_range(primitive)
                && !range.contains(interval)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "exact arithmetic in {owner} may overflow `{}`: the operands are not provably \
                     in range (decision 17 -- exact arithmetic is a proof obligation). Widen with \
                     an `as` cast to a larger type, constrain the operands' range, or opt into a \
                     defined-overflow domain (`{} in Wrapping`/`Saturating`/`Trapping`).",
                    primitive_name(primitive),
                    primitive_name(primitive),
                )));
            }

            Analysis {
                domain,
                interval,
                primitive,
            }
        }
        ExpressionNode::Cast(cast) => {
            // A cast re-types its operand, so the outer target does not flow in.
            let _ = analyze(program, machine, state, cast.value, env, None, owner, diagnostics);
            let primitive = program
                .expression_table
                .name_path_members(cast.target_type)
                .last()
                .and_then(|name| PrimitiveType::from_name(name.as_str()));
            // The cast bounds the value to the target type's range (and re-tags its
            // domain), so it is a widening/narrowing escape from an overflow.
            let interval = primitive
                .and_then(primitive_range)
                .unwrap_or(Interval::UNBOUNDED);
            Analysis {
                domain: Some(cast.domain),
                interval,
                primitive,
            }
        }
        ExpressionNode::Call(call) => {
            // S4: the `min`/`max` builtins bound their result by their operands'
            // intervals (`max(0, x)` is >= 0, `min(x, 100)` is <= 100), so a
            // clamped value can feed exact arithmetic instead of poisoning the
            // enclosing op. Reserved-builtin name + a free (receiverless) call +
            // exactly two arguments. Each operand is analyzed into a THROWAWAY
            // diagnostic buffer and its interval is trusted ONLY if it proves
            // clean -- the bound then rests on sound operand ranges, while an
            // unproven operand's diagnostics are dropped (call arguments are not
            // otherwise overflow-checked today, so this stays strictly permissive:
            // it can only tighten a previously-unbounded call result, never add a
            // rejection).
            if !call.receiver.is_valid()
                && matches!(call.target.as_str(), "min" | "max")
                && let [left_arg, right_arg] =
                    program.expression_table.expression_handles(call.arguments)
            {
                let mut throwaway = Vec::new();
                let left = analyze(
                    program, machine, state, *left_arg, env, target_primitive, owner,
                    &mut throwaway,
                );
                let right = analyze(
                    program, machine, state, *right_arg, env, target_primitive, owner,
                    &mut throwaway,
                );
                if throwaway.is_empty() {
                    let interval = if call.target.as_str() == "max" {
                        left.interval.max_with(right.interval)
                    } else {
                        left.interval.min_with(right.interval)
                    };
                    let domain = match (left.domain, right.domain) {
                        (Some(left_domain), Some(right_domain)) if left_domain == right_domain => {
                            Some(left_domain)
                        }
                        (Some(domain), None) | (None, Some(domain)) => Some(domain),
                        _ => None,
                    };
                    return Analysis {
                        domain,
                        interval,
                        primitive: left.primitive.or(right.primitive),
                    };
                }
            }

            // S4 return-range inference: a machine whose return type declares a
            // literal range constraint (`-> i32 [0..=10]`) is ENFORCED to
            // return within that range (validate_return_value_range, below), so a
            // caller doing exact arithmetic on the result can rely on the narrowed
            // interval instead of being forced into a domain. Narrow ONLY when the
            // return type carries a range constraint -- a plain `-> u32` stays
            // NEUTRAL (opaque/unbounded, as before); attaching a bare type's full
            // range + primitive to an otherwise-unbounded call result would turn a
            // previously-unchecked expression into a spurious overflow.
            match call_return_type(program, machine, call)
                .and_then(|return_type| {
                    range_constraint_interval(program, return_type)
                        .map(|interval| (program.primitive_type_reference(return_type), interval))
                }) {
                Some((primitive, interval)) => Analysis {
                    domain: None,
                    interval,
                    primitive,
                },
                None => NEUTRAL,
            }
        }
        ExpressionNode::Integer(value) => Analysis {
            domain: None,
            interval: Interval::constant(*value),
            primitive: None,
        },
        ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => NEUTRAL,
        // A place (`x`, `self.field`): its declared type gives the domain and the
        // integer primitive. The value range is the FLOW-tracked interval (S4) if
        // we have proven one on this linear path, else the primitive's full range.
        _ => match declared_place_type_raw(program, machine, state, expression) {
            Some(handle) => {
                let primitive = program.primitive_type_reference(handle);
                let type_range = primitive
                    .and_then(primitive_range)
                    .unwrap_or(Interval::UNBOUNDED);
                // Narrowest sound interval: a value PROVEN on this path (flow env)
                // wins; else a declared `[min..max]` range constraint (S4); else
                // the full type width.
                let interval = place_path(program, expression)
                    .and_then(|path| env.get(&path))
                    .or_else(|| range_constraint_interval(program, handle))
                    .unwrap_or(type_range);
                // Atomic integer types (AtomicU32, ...) have hardware wrap-around
                // semantics, so their arithmetic is Wrapping, not Exact -- a
                // `fetch_add` never raises an overflow proof obligation.
                let domain = if is_atomic_type(program, handle) {
                    ArithmeticDomain::Wrapping
                } else {
                    program.arithmetic_domain_for_type_reference(handle)
                };
                Analysis {
                    domain: Some(domain),
                    interval,
                    primitive,
                }
            }
            None => NEUTRAL,
        },
    }
}

/// S4: the value range a type declares via a `Range` constraint (`x: i32 [0..N]`),
/// so a bounded value's exact arithmetic can be proven in-range instead of using
/// the full type width. Inclusive bounds (a sound over-approximation either way).
/// `None` when the type has no literal range constraint. Looks through reference
/// shells.
pub(crate) fn range_constraint_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => range_constraint_interval(program, *referee),
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraintNode::Range { minimum, maximum } => Some(Interval {
                    low: Some(literal_i64(program, *minimum)?),
                    high: Some(literal_i64(program, *maximum)?),
                }),
                _ => None,
            }),
        _ => None,
    }
}

/// S4 return-range inference: the DECLARED return type of a value-position call,
/// when it resolves soundly and UNIQUELY from `program` alone. Conservative --
/// only `self`/receiver-free calls to a sibling machine (one attached to the same
/// data as the caller, with a state named after the call target) are resolved;
/// an external receiver, an ambiguous match, or no match returns `None` (no
/// narrowing). Soundness rests on uniqueness: bail rather than guess.
fn call_return_type(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
) -> Option<TypeReferenceHandle> {
    let receiver_is_self = !call.receiver.is_valid()
        || matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == "self"
                )
        );
    if !receiver_is_self {
        return None;
    }
    let target = call.target.as_str();
    let attached_data = current_machine.attached_data.as_ref()?;
    let mut returns = program
        .machines()
        .iter()
        .filter(|candidate| {
            candidate
                .attached_data
                .as_ref()
                .is_some_and(|data| data.as_str() == attached_data.as_str())
        })
        .filter_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name.as_str() == target)
        })
        .filter(|state| state.return_type.is_valid())
        .map(|state| state.return_type);
    let first = returns.next()?;
    // Unique match only -- bail on ambiguity (sound: fall back to unbounded).
    if returns.next().is_some() {
        return None;
    }
    Some(first)
}

/// S4 return-range ENFORCEMENT (companion to the call-site narrowing): when a
/// return type declares a literal `[a..=b]`, the returned value's proven
/// interval must fit inside it, else callers that trust the declared range are
/// unsound. No-op when the return type carries no range constraint (so plain
/// returns are unaffected -- range-constrained return types are a new
/// capability). `interval` is the return expression's already-analyzed interval.
pub(crate) fn enforce_declared_return_range(
    program: &TypedTrees,
    return_type: TypeReferenceHandle,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(range) = range_constraint_interval(program, return_type)
        && !range.contains(interval)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} returns a value not provably within its declared range: callers rely on the \
             declared `[a..=b]` for exact arithmetic on the result, so the returned value must \
             be proven to honor it (decision 17). Constrain the returned value, or widen/remove the \
             return range constraint."
        )));
    }
}

/// S4: analyze a transition VALUE-return expression and enforce its declared
/// return range. Gated on the return type carrying a range constraint -- only
/// then is the (otherwise un-validated) transition-value return analyzed, so
/// existing plain returns are byte-for-byte unaffected.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_return_value_range(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    return_expression: ExpressionHandle,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if range_constraint_interval(program, state.return_type).is_none() {
        return;
    }
    let interval = validate_arithmetic_domains(
        program,
        machine,
        Some(state),
        return_expression,
        env,
        program.primitive_type_reference(state.return_type),
        owner,
        diagnostics,
    );
    enforce_declared_return_range(program, state.return_type, interval, owner, diagnostics);
}

/// A type-constraint range bound as an i64, when it is a literal integer (the
/// common case, `[0..100]`). Non-literal bounds are not narrowed (the caller
/// falls back to the full type range -- sound).
fn literal_i64(program: &TypedTrees, expression: ExpressionHandle) -> Option<i64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        _ => None,
    }
}

/// Whether a place's declared type is an atomic integer (`AtomicU32`, ...),
/// whose arithmetic wraps by hardware semantics, through reference/constraint
/// shells.
fn is_atomic_type(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => name.as_str().starts_with("Atomic"),
        TypeReferenceNode::Reference { referee, .. } => is_atomic_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => is_atomic_type(program, *base_type),
        _ => false,
    }
}

fn primitive_name(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::I8 => "i8",
        PrimitiveType::U8 => "u8",
        PrimitiveType::I16 => "i16",
        PrimitiveType::U16 => "u16",
        PrimitiveType::I32 => "i32",
        PrimitiveType::U32 => "u32",
        PrimitiveType::I64 => "i64",
        PrimitiveType::U64 => "u64",
        PrimitiveType::Isize => "isize",
        PrimitiveType::Usize => "usize",
        PrimitiveType::Bool => "bool",
        PrimitiveType::F32 => "f32",
        PrimitiveType::F64 => "f64",
        PrimitiveType::String => "String",
    }
}

#[cfg(test)]
mod tests {
    use super::Interval;

    fn iv(low: i64, high: i64) -> Interval {
        Interval {
            low: Some(low),
            high: Some(high),
        }
    }

    #[test]
    fn modulo_by_positive_constant_bounds_by_divisor() {
        // unknown-sign dividend % 100 -> [-99, 99]
        assert_eq!(Interval::UNBOUNDED.modulo(iv(100, 100)), iv(-99, 99));
        // non-negative dividend -> non-negative remainder
        assert_eq!(iv(0, i64::MAX).modulo(iv(100, 100)), iv(0, 99));
        // non-positive dividend -> non-positive remainder
        assert_eq!(iv(i64::MIN, 0).modulo(iv(100, 100)), iv(-99, 0));
        // divisor given as a positive RANGE uses its max magnitude
        assert_eq!(Interval::UNBOUNDED.modulo(iv(2, 8)), iv(-7, 7));
    }

    #[test]
    fn modulo_unsound_divisors_stay_unbounded() {
        // divisor may be 0 -> cannot bound
        assert_eq!(iv(0, 10).modulo(iv(0, 5)), Interval::UNBOUNDED);
        // divisor spans 0
        assert_eq!(iv(0, 10).modulo(iv(-3, 3)), Interval::UNBOUNDED);
        // unbounded divisor
        assert_eq!(iv(0, 10).modulo(Interval::UNBOUNDED), Interval::UNBOUNDED);
        // negative divisor range bounds by magnitude
        assert_eq!(Interval::UNBOUNDED.modulo(iv(-8, -2)), iv(-7, 7));
    }

    #[test]
    fn divide_by_nonzero_never_grows_magnitude() {
        // bounded dividend / nonzero divisor stays within the dividend bounds,
        // widened to include 0 (quotient can reach 0)
        assert_eq!(iv(-99, 99).divide(iv(2, 2)), iv(-99, 99));
        assert_eq!(iv(10, 50).divide(iv(3, 3)), iv(0, 50));
        assert_eq!(iv(-50, -10).divide(iv(3, 3)), iv(-50, 0));
        // unbounded dividend cannot be bounded by division
        assert_eq!(Interval::UNBOUNDED.divide(iv(2, 2)), Interval::UNBOUNDED);
        // maybe-zero divisor: cannot assume magnitude >= 1
        assert_eq!(iv(10, 50).divide(iv(0, 5)), Interval::UNBOUNDED);
    }

    #[test]
    fn min_max_clamp_against_unbounded() {
        assert_eq!(
            Interval::UNBOUNDED.max_with(iv(0, 0)),
            Interval { low: Some(0), high: None }
        );
        assert_eq!(
            Interval::UNBOUNDED.min_with(iv(100, 100)),
            Interval { low: None, high: Some(100) }
        );
        assert_eq!(iv(0, 50).max_with(iv(10, 10)), iv(10, 50));
        assert_eq!(iv(0, 50).min_with(iv(10, 10)), iv(0, 10));
        // chained clamp: max(seed,0) then min(_,60) -> [0,60]
        assert_eq!(
            Interval::UNBOUNDED.max_with(iv(0, 0)).min_with(iv(60, 60)),
            iv(0, 60)
        );
    }

    #[test]
    fn nonzero_magnitude_bound_requires_excluding_zero() {
        assert_eq!(iv(1, 100).nonzero_magnitude_bound(), Some(100));
        assert_eq!(iv(-100, -1).nonzero_magnitude_bound(), Some(100));
        assert_eq!(iv(0, 100).nonzero_magnitude_bound(), None); // includes 0
        assert_eq!(iv(-5, 5).nonzero_magnitude_bound(), None); // spans 0
        assert_eq!(Interval::UNBOUNDED.nonzero_magnitude_bound(), None);
    }
}
