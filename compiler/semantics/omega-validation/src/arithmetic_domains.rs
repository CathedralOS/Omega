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
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::types::PrimitiveType;

use crate::places::declared_place_type_raw;

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
}

/// Walk a value expression, apply the domain + overflow rules to every nested
/// arithmetic binary, and return the expression's proven interval (so the caller
/// can record it for the assigned place). `owner` describes the site.
pub(crate) fn validate_arithmetic_domains(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Interval {
    if !expression.is_valid() {
        return Interval::UNBOUNDED;
    }
    analyze(program, machine, state, expression, env, owner, diagnostics).interval
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
#[derive(Debug, Clone, Copy)]
pub(crate) struct Interval {
    low: Option<i64>,
    high: Option<i64>,
}

impl Interval {
    const UNBOUNDED: Interval = Interval {
        low: None,
        high: None,
    };

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
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let operator = binary.operator;
            let left = analyze(program, machine, state, binary.left, env, owner, diagnostics);
            let right = analyze(program, machine, state, binary.right, env, owner, diagnostics);
            if !is_arithmetic(operator) {
                // Comparison / logical `and`/`or`: a `bool`, no arithmetic domain.
                return NEUTRAL;
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
            let primitive = left.primitive.or(right.primitive);
            let interval = match operator {
                BinaryOperator::Add => left.interval.add(right.interval),
                BinaryOperator::Subtract => left.interval.subtract(right.interval),
                BinaryOperator::Multiply => left.interval.multiply(right.interval),
                // Division/modulo cannot grow magnitude beyond the dividend (modulo
                // is bounded by the divisor); shifts are not bounded here. None of
                // these are flagged for overflow below, so the interval only needs
                // to be a safe over-approximation for any ENCLOSING op.
                _ => Interval::UNBOUNDED,
            };

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
            let _ = analyze(program, machine, state, cast.value, env, owner, diagnostics);
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
                let interval = place_path(program, expression)
                    .and_then(|path| env.get(&path))
                    .unwrap_or(type_range);
                Analysis {
                    domain: Some(program.arithmetic_domain_for_type_reference(handle)),
                    interval,
                    primitive,
                }
            }
            None => NEUTRAL,
        },
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
