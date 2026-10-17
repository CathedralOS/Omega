//! Integer literal, cast and primitive range fitting.
//!
//! Statement checking retains cast and divisor decisions with its value-range
//! result.
//! Queries may discard those decisions, but lowering never reconstructs them
//! by re-running an expression under a different destination or branch scope.

use crate::proof_contracts::arithmetic_domains::expression_analysis::analyze;
use crate::proof_contracts::arithmetic_domains::interval::Interval;
use crate::proof_contracts::arithmetic_domains::place_paths::place_path;
use crate::proof_contracts::arithmetic_domains::value_environment::ValueEnvironment;
use crate::value_custody::places::declared_place_type_raw;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Whether a wider fixed-integer carrier contains every source value.
/// Identity conversions and the distinct address carrier are not widenings.
pub fn integer_widen_is_total(source: PrimitiveType, target: PrimitiveType) -> bool {
    fn shape(primitive: PrimitiveType) -> Option<(bool, u8)> {
        primitive
            .integer_sign_bits()
            .map(|(sign, bits)| (sign == semantic_vocabulary::IntegerSign::Signed, bits as u8))
    }
    let Some((source_signed, source_bits)) = shape(source) else {
        return false;
    };
    let Some((target_signed, target_bits)) = shape(target) else {
        return false;
    };
    source_bits < target_bits && (!source_signed || target_signed)
}

pub(crate) fn validate_anonymous_integer_range(
    program: &dyn crate::CheckingInput,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Interval, Option<PrimitiveType>, Option<ArithmeticDomain>)> {
    let primitive = program.primitive_type_reference(destination)?;
    validate_anonymous_integer_primitive_range(program, primitive, expression, owner, diagnostics)
}

pub(crate) fn validate_anonymous_integer_primitive_range(
    program: &dyn crate::CheckingInput,
    primitive: PrimitiveType,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Interval, Option<PrimitiveType>, Option<ArithmeticDomain>)> {
    // A destination policy governs operations after landing. It cannot give a
    // fraction an integer value or make an out-of-range anonymous value fit.
    if !primitive.accepts_integer_literal() {
        return None;
    }
    if let Some(dispatch) = program.expression_table.match_node(expression) {
        // Every arm must form a compatible result, even after a wildcard. The
        // destination checks the complete anonymous arm; a successful arm is
        // not an unconditional interval fact about the whole dispatch.
        let mut pending = program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .map(|arm| arm.value)
            .collect::<Vec<_>>();
        let mut visited = vec![expression];
        while let Some(value) = pending.pop() {
            if !program.expression_table.expression_is_valid(value) || visited.contains(&value) {
                continue;
            }
            visited.push(value);
            if let Some(nested) = program.expression_table.match_node(value) {
                pending.extend(
                    program
                        .expression_table
                        .match_arms(nested.arms)
                        .iter()
                        .map(|arm| arm.value),
                );
            } else {
                validate_anonymous_integer_primitive_range(
                    program,
                    primitive,
                    value,
                    owner,
                    diagnostics,
                );
            }
        }
        return None;
    }
    let evaluated = crate::value_custody::literals::anonymous_numeric_value(
        program,
        expression,
        &mut |expression| {
            crate::value_custody::literals::has_anonymous_operator_meaning(program, expression)
        },
    )?;
    let Some(value) = evaluated.value.to_integer_exact() else {
        diagnostics.push(
            Diagnostic::error(format!(
                "anonymous value `{}` is not an integer and cannot land in `{}` in {owner}; \
             type an operand before division if integer division was intended",
                evaluated.value,
                primitive.name(),
            ))
            .with_source_span(program.expression_table.source_span(expression)),
        );
        return Some((Interval::UNBOUNDED, None, None));
    };
    if primitive == PrimitiveType::Addr && value.to_u64().is_some() {
        // Address meaning and target fit remain with the address owner. Its
        // current literal payload is an unsigned 64-bit word, so an anonymous
        // value outside that representation cannot reach address lowering.
        // Passing this bound does not stamp a fixed-integer landing or prove
        // fit in a selected target's address model.
        return None;
    }
    if let Some(literal) = crate::value_custody::literals::land_integer_value(&value, primitive) {
        // The landed literal adopts the destination carrier; it has no
        // arithmetic policy of its own for the store parity check.
        return Some((literal_interval(&literal), Some(primitive), None));
    }
    diagnostics.push(Diagnostic::error(format!(
        "anonymous integer value `{value}` does not fit destination `{}` in {owner}",
        primitive.name()
    )));
    Some((Interval::UNBOUNDED, None, None))
}

/// Like [`validate_arithmetic_domains`] but also returns the expression's source
/// integer primitive and arithmetic domain (the `None`-for-unknown results).
/// The narrowing check needs the primitive: a value produced by a typed source
/// is ALWAYS within that type's range (a `u32 in Wrapping` sum is a u32 even
/// when its mathematical interval spills past `u32`), so the sound value range
/// is `interval ∩ primitive_range(source)` -- intersecting keeps a flow-proven
/// tighter interval while clamping a domain-wrapped over-approximation back to
/// the type. The carrier-parity check needs the pair: a typed value keeps its
/// own type and arithmetic policy across a store.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_range(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Interval, Option<PrimitiveType>, Option<ArithmeticDomain>) {
    validate_value_range_retaining_decisions(
        program,
        machine,
        state,
        expression,
        environment,
        target_primitive,
        target_domain,
        owner,
        diagnostics,
        &mut super::ArithmeticDecisions::default(),
    )
}

/// Return occurrence decisions from the same check that supplies the value's
/// range. Re-analyzing the expression later can lose its destination and its
/// branch-local premises, even if the surrounding statement has not changed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_range_retaining_decisions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    decisions: &mut super::ArithmeticDecisions,
) -> (Interval, Option<PrimitiveType>, Option<ArithmeticDomain>) {
    if !expression.is_valid() {
        return (Interval::UNBOUNDED, None, None);
    }
    let first_diagnostic = diagnostics.len();
    let analysis = analyze(
        program,
        machine,
        state,
        expression,
        environment,
        target_primitive,
        target_domain,
        owner,
        true,
        diagnostics,
    );
    if !diagnostics[first_diagnostic..]
        .iter()
        .any(Diagnostic::is_error)
    {
        decisions.extend(analysis.decisions);
    }
    (
        analysis.value.interval,
        analysis.value.primitive,
        analysis.value.domain,
    )
}

/// The interval of an anonymous literal (D14). A literal that fits i64 is a
/// point; an oversize u64-magnitude literal is honestly over-approximated as
/// "above i64::MAX" (or "below i64::MIN" for its folded negation), so
/// intervals cannot establish exact large-literal representability. Scalar
/// destination admission uses the complete mathematical value instead.
pub(crate) fn literal_interval(literal: &numerics::literals::IntegerLiteral) -> Interval {
    match literal.value_i64() {
        Some(value) => Interval::constant(value),
        None if !literal.text().starts_with('-') => Interval {
            low: Some(i64::MAX),
            high: None,
        },
        None => Interval {
            low: None,
            high: Some(i64::MIN),
        },
    }
}

/// The representable range of an integer primitive. `None` for non-integers
/// (`bool`/`f32`/`f64`/`String`) and for `u64`/`usize` whose maximum exceeds
/// `i64` (their high end is left unbounded -- an over-approximation that still
/// rejects genuine overflow).
pub(crate) fn primitive_range(primitive: PrimitiveType) -> Option<Interval> {
    let (low, high): (Option<i64>, Option<i64>) = match primitive {
        PrimitiveType::I8 => (Some(i8::MIN as i64), Some(i8::MAX as i64)),
        PrimitiveType::U8 => (Some(0), Some(u8::MAX as i64)),
        PrimitiveType::I16 => (Some(i16::MIN as i64), Some(i16::MAX as i64)),
        PrimitiveType::U16 => (Some(0), Some(u16::MAX as i64)),
        PrimitiveType::I32 => (Some(i32::MIN as i64), Some(i32::MAX as i64)),
        PrimitiveType::U32 => (Some(0), Some(u32::MAX as i64)),
        PrimitiveType::I64 => (Some(i64::MIN), Some(i64::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => (Some(0), None),
        PrimitiveType::Selector | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    };
    Some(Interval { low, high })
}

/// Whether every value admitted by `source` is representable by the target
/// integer carrier. Unknown interval endpoints use exact fixed-carrier bounds,
/// never infinity. The address ceiling remains symbolic; a finite observed
/// address interval can still prove conversion to a fixed integer.
pub(crate) fn integer_interval_fits_primitive(
    source: Interval,
    source_primitive: PrimitiveType,
    target: PrimitiveType,
) -> bool {
    // An unknown endpoint is an interval approximation, not an unbounded
    // destination carrier. Address fit needs the selected model's symbolic
    // bound; identity alone is independent of that model.
    if target == PrimitiveType::Addr {
        return source_primitive == target;
    }
    let Some(source_range) = primitive_range(source_primitive) else {
        return false;
    };
    let Some((target_minimum, target_maximum)) = target.integer_bounds() else {
        return false;
    };
    // Every runtime value is already constrained by its source carrier. Keep a
    // tighter flow interval only when it is wholly inside that carrier. A
    // wrapped expression's mathematical interval can lie outside the carrier;
    // intersecting a disjoint interval would manufacture an empty range and
    // falsely prove arbitrary targets, so fall back to the full carrier.
    let source = if source_range.contains(source) {
        source
    } else {
        source_range
    };
    let fixed_source = (source_primitive != PrimitiveType::Addr)
        .then(|| source_primitive.integer_bounds())
        .flatten();
    let minimum = source
        .low
        .map(i128::from)
        .or_else(|| fixed_source.map(|bounds| bounds.0));
    let maximum = source
        .high
        .map(i128::from)
        .or_else(|| fixed_source.map(|bounds| bounds.1));
    minimum.is_some_and(|minimum| minimum >= target_minimum)
        && maximum.is_some_and(|maximum| maximum <= target_maximum)
}

/// The bit width of an integer primitive (the F8 shift-count bound), or `None`
/// for non-integer types. `Addr` is excluded deliberately: shifting an address
/// is already rejected by the address-arithmetic model above.
pub(crate) fn integer_bit_width(primitive: PrimitiveType) -> Option<i64> {
    match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 => Some(8),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(16),
        PrimitiveType::I32 | PrimitiveType::U32 => Some(32),
        PrimitiveType::I64 | PrimitiveType::U64 => Some(64),
        PrimitiveType::Addr
        | PrimitiveType::Selector
        | PrimitiveType::Bool
        | PrimitiveType::F32
        | PrimitiveType::F64 => None,
    }
}

/// Prove an Exact multiplication total when both operands are value-preserving
/// unsigned widenings and the sum of their source widths fits the common target
/// carrier. The ordinary interval lattice is i64-backed, so it cannot express
/// the complete u64 upper endpoint even though `u32::MAX * u32::MAX` is
/// representable by u64. This structural proof closes only that representation
/// gap; arbitrary casts and mixed targets fail closed.
pub(crate) fn exact_unsigned_widened_multiply_fits(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let operand = |expression| -> Option<(i64, PrimitiveType)> {
        let cast = program.expression_table.cast_node(expression)?;
        if cast.form.is_recast()
            || !cast.semantic_domain.is_empty()
            || cast.domain != ArithmeticDomain::Exact
        {
            return None;
        }
        let source_type = declared_place_type_raw(program, machine, state, cast.value)?;
        let source = program.primitive_type_reference(source_type)?;
        let target = program.primitive_type_reference(cast.target_type)?;
        if !matches!(
            source,
            PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32
        ) || !matches!(
            target,
            PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
        ) {
            return None;
        }
        let source_width = integer_bit_width(source)?;
        let target_width = integer_bit_width(target)?;
        (source_width < target_width).then_some((source_width, target))
    };
    let Some((left_width, left_target)) = operand(left) else {
        return false;
    };
    let Some((right_width, right_target)) = operand(right) else {
        return false;
    };
    left_target == right_target
        && integer_bit_width(left_target).is_some_and(|width| left_width + right_width <= width)
}

pub(crate) fn u64_exact_shift_left_fits(value: Interval, count: Interval) -> bool {
    let (Some(value_low), Some(value_high), Some(count_low), Some(count_high)) =
        (value.low, value.high, count.low, count.high)
    else {
        return false;
    };
    if value_low < 0 || count_low < 0 || count_high >= 64 {
        return false;
    }
    u128::try_from(value_high)
        .ok()
        .and_then(|value| value.checked_shl(count_high as u32))
        .is_some_and(|maximum| maximum <= u128::from(u64::MAX))
}

/// The integer value of a literal operand (through a `Mutable` wrapper), or `None`
/// when the operand is not a plain integer literal.
pub(crate) fn integer_literal_value(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> Option<i64> {
    let node = program.expression_table.peeled_expression(value);
    match node {
        ExpressionNode::Integer(literal) => literal.value_i64(),
        _ => None,
    }
}

/// A type-constraint range bound as an i64: a literal integer (`[0..100]`) or
/// a CONSTANT integer expression (`[0 - 1..=40]` folds to `-1` -- expression
/// bounds used to silently behave UNBOUNDED). A non-constant bound is not
/// narrowed (the caller falls back to the full type range -- sound; the
/// declaration check in type_references.rs rejects it loudly).
pub(crate) fn literal_i64(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<i64> {
    program.expression_table.constant_integer_value(expression)
}

pub(crate) fn literal_u64(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<u64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal.value_bignum()?.to_u64(),
        ExpressionNode::Borrow(inner) => literal_u64(program, inner.target),
        _ => None,
    }
}

pub(crate) fn known_u64_value(
    program: &dyn crate::CheckingInput,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
) -> Option<u64> {
    literal_u64(program, expression).or_else(|| {
        let path = place_path(program, expression)?;
        environment.known_u64_values.get(&path).copied()
    })
}

/// Source bounds at this use, through the ordinary arithmetic analysis and
/// its live environment. Complete open endpoints with the source carrier,
/// never with a coercion's receiving type. Exact unsigned snapshots retain
/// values beyond the signed compatibility interval.
pub(crate) fn integer_source_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    primitive: PrimitiveType,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(i128, i128)> {
    let (interval, _, _) = validate_value_range(
        program,
        machine,
        Some(state),
        expression,
        environment,
        None,
        ArithmeticDomain::Exact,
        "integer-to-float coercion source",
        diagnostics,
    );
    integer_source_bounds_from_interval(program, environment, expression, primitive, interval)
}

/// Complete a source owner's interval with that source carrier and its exact
/// unsigned snapshot. Abstract and executable coercion obligations share this
/// endpoint completion; the receiving format supplies no source bound.
pub(crate) fn integer_source_bounds_from_interval(
    program: &dyn crate::CheckingInput,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    primitive: PrimitiveType,
    interval: Interval,
) -> Option<(i128, i128)> {
    let carrier = primitive_range(primitive)?;
    if let Some(value) = known_u64_value(program, environment, expression) {
        return Some((i128::from(value), i128::from(value)));
    }
    let interval = interval.intersect(carrier);
    let minimum = i128::from(interval.low?);
    let maximum = interval
        .high
        .map(i128::from)
        .or_else(|| (primitive == PrimitiveType::U64).then_some(i128::from(u64::MAX)))?;
    (minimum <= maximum).then_some((minimum, maximum))
}

/// Whether a place's declared type is an atomic integer (`AtomicU32`, ...),
/// whose arithmetic wraps by hardware semantics, through reference/constraint
/// shells.
pub(crate) fn is_atomic_type(
    program: &dyn crate::CheckingInput,
    handle: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => name.as_str().starts_with("Atomic"),
        TypeReferenceNode::Reference { referee, .. } => is_atomic_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => is_atomic_type(program, *base_type),
        _ => false,
    }
}

pub(crate) fn primitive_name(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::I8 => "i8",
        PrimitiveType::U8 => "u8",
        PrimitiveType::I16 => "i16",
        PrimitiveType::U16 => "u16",
        PrimitiveType::I32 => "i32",
        PrimitiveType::U32 => "u32",
        PrimitiveType::I64 => "i64",
        PrimitiveType::U64 => "u64",
        PrimitiveType::Addr => "addr",
        PrimitiveType::Bool => "bool",
        PrimitiveType::Selector => "selector",
        PrimitiveType::F32 => "f32",
        PrimitiveType::F64 => "f64",
    }
}
