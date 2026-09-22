//! Integer literal, cast and primitive range fitting.

use crate::proof_contracts::arithmetic_domains::expression_analysis::analyze;
use crate::proof_contracts::arithmetic_domains::interval::Interval;
use crate::proof_contracts::arithmetic_domains::place_paths::place_path;
use crate::proof_contracts::arithmetic_domains::value_environment::ValueEnvironment;
use crate::value_custody::places::declared_place_type_raw;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Whether a wider fixed-integer carrier contains every source value.
/// Identity conversions and the distinct address carrier are not widenings.
pub fn integer_widen_is_total(source: PrimitiveType, target: PrimitiveType) -> bool {
    fn shape(primitive: PrimitiveType) -> Option<(bool, u8)> {
        Some(match primitive {
            PrimitiveType::I8 => (true, 8),
            PrimitiveType::I16 => (true, 16),
            PrimitiveType::I32 => (true, 32),
            PrimitiveType::I64 => (true, 64),
            PrimitiveType::U8 => (false, 8),
            PrimitiveType::U16 => (false, 16),
            PrimitiveType::U32 => (false, 32),
            PrimitiveType::U64 => (false, 64),
            PrimitiveType::Addr | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
                return None;
            }
        })
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
    program: &TypedTrees,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Interval, Option<PrimitiveType>)> {
    let primitive = program.primitive_type_reference(destination)?;
    validate_anonymous_integer_primitive_range(program, primitive, expression, owner, diagnostics)
}

pub(crate) fn validate_anonymous_integer_primitive_range(
    program: &TypedTrees,
    primitive: PrimitiveType,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Interval, Option<PrimitiveType>)> {
    // A destination policy governs operations after landing. It cannot give a
    // fraction an integer value or make an out-of-range anonymous value fit.
    if primitive == PrimitiveType::Addr || !primitive.accepts_integer_literal() {
        return None;
    }
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
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
            if let ExpressionNode::Match(nested) = program.expression_table.expression(value) {
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
        return Some((Interval::UNBOUNDED, None));
    };
    if let Some(literal) = crate::value_custody::literals::land_integer_value(&value, primitive) {
        return Some((literal_interval(&literal), Some(primitive)));
    }
    diagnostics.push(Diagnostic::error(format!(
        "anonymous integer value `{value}` does not fit destination `{}` in {owner}",
        primitive.name()
    )));
    Some((Interval::UNBOUNDED, None))
}

/// Like [`validate_arithmetic_domains`] but also returns the expression's source
/// integer primitive (the `None`-for-unknown result). The narrowing check needs
/// it: a value produced by a typed source is ALWAYS within that type's range (a
/// `u32 in Wrapping` sum is a u32 even when its mathematical interval spills past
/// `u32`), so the sound value range is `interval ∩ primitive_range(source)` --
/// intersecting keeps a flow-proven tighter interval while clamping a
/// domain-wrapped over-approximation back to the type.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_range(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Interval, Option<PrimitiveType>) {
    if !expression.is_valid() {
        return (Interval::UNBOUNDED, None);
    }
    let analysis = analyze(
        program,
        machine,
        state,
        expression,
        environment,
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    );
    (analysis.interval, analysis.primitive)
}

/// Retain every accepted occurrence-dependent exact fixed-integer cast under
/// the same flow environment used by validation. This is a query over the
/// validator's judgment, not a second range engine: operand intervals are
/// obtained by `validate_value_range`, and only fully bounded intervals that
/// fit the target are published. Later checked lowering can therefore discard
/// `ValueEnvironment` without turning validation success into ambient trust.
pub(crate) fn collect_exact_integer_cast_facts(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    facts: &mut Vec<crate::ExactIntegerCastFact>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            for child in crate::value_custody::expression_types::match_children(program, *dispatch)
            {
                collect_exact_integer_cast_facts(
                    program,
                    machine,
                    state,
                    child,
                    environment,
                    facts,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            if !cast.form.is_recast()
                && cast.semantic_domain.is_empty()
                && cast.domain == ArithmeticDomain::Exact
                && let Some(target_type) = program.primitive_type_reference(cast.target_type)
                && target_type != PrimitiveType::Addr
            {
                let mut diagnostics = Vec::new();
                let (interval, source_type) = validate_value_range(
                    program,
                    machine,
                    state,
                    cast.value,
                    environment,
                    None,
                    ArithmeticDomain::Exact,
                    "checked exact integer cast evidence",
                    &mut diagnostics,
                );
                if diagnostics.is_empty()
                    && let Some(source_type) = source_type
                    && source_type != target_type
                    && source_type != PrimitiveType::Addr
                    && primitive_range(source_type).is_some()
                    && primitive_range(target_type).is_some()
                    && integer_interval_fits_primitive(interval, source_type, target_type)
                {
                    let source_range = primitive_range(source_type)
                        .expect("fixed integer source has a primitive range");
                    let effective = if source_range.contains(interval) {
                        interval
                    } else {
                        source_range
                    };
                    if let (Some(minimum), Some(maximum)) = (effective.low, effective.high) {
                        facts.push(crate::ExactIntegerCastFact {
                            expression,
                            source_type,
                            target_type,
                            minimum: numerics::bignum::BigInt::from_i64(minimum),
                            maximum: numerics::bignum::BigInt::from_i64(maximum),
                        });
                    }
                }
            }
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                cast.value,
                environment,
                facts,
            );
        }
        ExpressionNode::Atomic(atomic) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                atomic.value,
                environment,
                facts,
            );
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_exact_integer_cast_facts(
                    program,
                    machine,
                    state,
                    *value,
                    environment,
                    facts,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                binary.left,
                environment,
                facts,
            );
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                binary.right,
                environment,
                facts,
            );
        }
        ExpressionNode::Call(call) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                call.receiver,
                environment,
                facts,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_exact_integer_cast_facts(
                    program,
                    machine,
                    state,
                    *argument,
                    environment,
                    facts,
                );
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                indexed.collection,
                environment,
                facts,
            );
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                indexed.index,
                environment,
                facts,
            );
        }
        ExpressionNode::Member(member) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                member.receiver,
                environment,
                facts,
            );
        }
        ExpressionNode::Borrow(value) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                value.target,
                environment,
                facts,
            );
        }
        ExpressionNode::Range(range) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                range.start,
                environment,
                facts,
            );
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                range.end,
                environment,
                facts,
            );
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_exact_integer_cast_facts(
                    program,
                    machine,
                    state,
                    field.value,
                    environment,
                    facts,
                );
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                unary.operand,
                environment,
                facts,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// The interval of an anonymous literal (D14). A literal that fits i64 is a
/// point; an oversize u64-magnitude literal is honestly over-approximated as
/// "above i64::MAX" (or "below i64::MIN" for its folded negation), so
/// narrowing checks against i64-bounded targets still REJECT by interval
/// math alone -- never by silently skipping.
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
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    };
    Some(Interval { low, high })
}

/// Whether every value admitted by `source` is representable by the target
/// integer carrier. Unbounded endpoints compare structurally: an unbounded
/// source side is accepted only when the target is unbounded on that side too.
pub(crate) fn integer_interval_fits_primitive(
    source: Interval,
    source_primitive: PrimitiveType,
    target: PrimitiveType,
) -> bool {
    let Some(source_range) = primitive_range(source_primitive) else {
        return false;
    };
    let Some(target) = primitive_range(target) else {
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
    let low_fits = match (source.low, target.low) {
        (_, None) => true,
        (Some(source), Some(target)) => source >= target,
        (None, Some(_)) => false,
    };
    let high_fits = match (source.high, target.high) {
        (_, None) => true,
        (Some(source), Some(target)) => source <= target,
        (None, Some(_)) => false,
    };
    low_fits && high_fits
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
        PrimitiveType::Addr | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

/// Prove an Exact multiplication total when both operands are value-preserving
/// unsigned widenings and the sum of their source widths fits the common target
/// carrier. The ordinary interval lattice is i64-backed, so it cannot express
/// the complete u64 upper endpoint even though `u32::MAX * u32::MAX` is
/// representable by u64. This structural proof closes only that representation
/// gap; arbitrary casts and mixed targets fail closed.
pub(crate) fn exact_unsigned_widened_multiply_fits(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let operand = |expression| -> Option<(i64, PrimitiveType)> {
        let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
            return None;
        };
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
pub(crate) fn integer_literal_value(program: &TypedTrees, value: ExpressionHandle) -> Option<i64> {
    let mut node = program.expression_table.expression(value);
    while let ExpressionNode::Borrow(inner) = node {
        node = program.expression_table.expression(inner.target);
    }
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
pub(crate) fn literal_i64(program: &TypedTrees, expression: ExpressionHandle) -> Option<i64> {
    program.expression_table.constant_integer_value(expression)
}

pub(crate) fn literal_u64(program: &TypedTrees, expression: ExpressionHandle) -> Option<u64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal.value_bignum()?.to_u64(),
        ExpressionNode::Borrow(inner) => literal_u64(program, inner.target),
        _ => None,
    }
}

pub(crate) fn known_u64_value(
    program: &TypedTrees,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
) -> Option<u64> {
    literal_u64(program, expression).or_else(|| {
        let path = place_path(program, expression)?;
        environment.known_u64_values.get(&path).copied()
    })
}

/// Whether a place's declared type is an atomic integer (`AtomicU32`, ...),
/// whose arithmetic wraps by hardware semantics, through reference/constraint
/// shells.
pub(crate) fn is_atomic_type(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
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
        PrimitiveType::F32 => "f32",
        PrimitiveType::F64 => "f64",
    }
}
