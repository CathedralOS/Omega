//! `as` conversions: the source value, the cast-ruling obligations, and the
//! interval the converted value keeps.

use super::{Analysis, BOOLEAN_INTERVAL, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, integer_interval_fits_primitive, literal_interval, primitive_name,
    primitive_range, validate_anonymous_integer_primitive_range,
};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionNode, Interval, PrimitiveType, cast_ranges,
    float_source_proves_integer_conversion,
};
use crate::value_custody::literals;
use typed_trees::expression::TableCastExpression;

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: typed_trees::expression::ExpressionHandle,
    cast: &TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
    decisions: &mut super::ArithmeticDecisions,
) -> Analysis {
    let first_diagnostic = diagnostics.len();
    let program = walk.program;
    let primitive = program.primitive_type_reference(cast.target_type);
    // A recast is a byte-preserving view (`&x as &T`), not a numeric
    // conversion.  In particular, viewing an f32/f64 place through an
    // equal-width unsigned referee must not acquire F4's float-to-int proof
    // obligation: no floating value is being truncated or rounded.
    // `expression_types::validate_cast_types` already keeps this distinction;
    // preserve it in the arithmetic-domain walk as well so later integer
    // operations see the stated referee type.
    if cast.form.is_recast() {
        walk.with_destination(None, ArithmeticDomain::Exact)
            .analyze(cast.value, diagnostics, decisions);
        return Analysis {
            domain: Some(ArithmeticDomain::Exact),
            interval: primitive
                .and_then(primitive_range)
                .unwrap_or(Interval::UNBOUNDED),
            primitive,
        };
    }
    let source = source_value(walk, cast, primitive, diagnostics, decisions);
    // Source facts precede target qualification. In particular a Wrapping cast
    // must not use its asserted target range to prove that same range, or
    // repair an out-of-range initial value.
    let source_interval = source
        .primitive
        .and_then(primitive_range)
        .map(|carrier| {
            if carrier.contains(source.interval) {
                source.interval
            } else {
                carrier
            }
        })
        .unwrap_or(source.interval);
    cast_ranges::validate_target_ranges(
        program,
        walk.machine,
        walk.state,
        cast,
        source_interval,
        source.primitive,
        walk.environment,
        walk.owner,
        diagnostics,
    );
    require_float_to_integer_policy(walk, cast, &source, primitive, diagnostics);
    require_exact_integer_fit(walk, cast, &source, primitive, diagnostics);
    require_trapping_conversion_coverage(walk, expression, cast, &source, primitive, diagnostics);
    if !diagnostics[first_diagnostic..]
        .iter()
        .any(Diagnostic::is_error)
        && cast.semantic_domain.is_empty()
        && cast.domain == ArithmeticDomain::Exact
        && let (Some(source_type), Some(target_type)) = (source.primitive, primitive)
        && source_type != target_type
        && source_type != PrimitiveType::Addr
        && target_type != PrimitiveType::Addr
        && primitive_range(source_type).is_some()
        && primitive_range(target_type).is_some()
        && integer_interval_fits_primitive(source.interval, source_type, target_type)
        && let (Some(minimum), Some(maximum)) = (source_interval.low, source_interval.high)
    {
        decisions
            .exact_integer_casts
            .push(crate::ExactIntegerCastFact {
                expression,
                source_type,
                target_type,
                minimum: numerics::bignum::BigInt::from_i64(minimum),
                maximum: numerics::bignum::BigInt::from_i64(maximum),
            });
    }
    Analysis {
        domain: Some(cast.domain),
        interval: result_interval(cast, &source, primitive),
        primitive,
    }
}

/// The explicit cast supplies the first rendering of a wholly anonymous
/// calculation. Its target cannot retype already-landed operations, and the
/// enclosing destination never flows inward.
fn source_value(
    walk: &ExpressionWalk,
    cast: &TableCastExpression,
    primitive: Option<PrimitiveType>,
    diagnostics: &mut Vec<Diagnostic>,
    decisions: &mut super::ArithmeticDecisions,
) -> Analysis {
    let program = walk.program;
    if cast.semantic_domain.is_empty()
        && let Some(primitive) = primitive
        && matches!(
            program.expression_table.expression(cast.value),
            ExpressionNode::Match(_)
        )
        && literals::has_anonymous_numeric_results(program, cast.value)
    {
        // A retained arm carrier precedes conversion. Its anonymous peers land
        // at that result join, not in this cast target.
        validate_anonymous_integer_primitive_range(
            program,
            primitive,
            cast.value,
            walk.owner,
            diagnostics,
        );
    }
    let anonymous = literals::anonymous_numeric_value(program, cast.value, &mut |expression| {
        literals::has_anonymous_operator_meaning(program, expression)
    });
    let integer_target = primitive.filter(|primitive| integer_bit_width(*primitive).is_some());
    let (Some(evaluated), Some(primitive)) = (anonymous, integer_target) else {
        return walk
            .with_destination(None, ArithmeticDomain::Exact)
            .analyze(cast.value, diagnostics, decisions);
    };
    match evaluated
        .value
        .to_integer_exact()
        .and_then(|value| literals::land_integer_value(&value, primitive))
    {
        Some(literal) => Analysis {
            domain: None,
            interval: literal_interval(&literal),
            primitive: Some(primitive),
        },
        None => {
            diagnostics.push(
                Diagnostic::error(format!(
                    "anonymous value `{}` cannot land exactly in cast target `{}` in {}; type \
                     an operand before division if integer division was intended",
                    evaluated.value,
                    primitive.name(),
                    walk.owner,
                ))
                .with_source_span(program.expression_table.source_span(cast.value)),
            );
            NEUTRAL
        }
    }
}

/// F4, the float-to-int cast ruling: proof or policy, never a target-defined
/// number.
fn require_float_to_integer_policy(
    walk: &ExpressionWalk,
    cast: &TableCastExpression,
    source: &Analysis,
    primitive: Option<PrimitiveType>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(
        source.primitive,
        Some(PrimitiveType::F32 | PrimitiveType::F64)
    ) {
        return;
    }
    let Some(target) = primitive.filter(|target| integer_bit_width(*target).is_some()) else {
        return;
    };
    let owner = walk.owner;
    // There is NO MODULAR READING of a float, so `f as iN in Wrapping` is a
    // compile error (ch5; the ruling's precedent generalized to the float
    // domain list). Saturating (NaN -> 0, clamp to the target range) and
    // Trapping (trap on NaN/out-of-range) are the defined policies.
    if cast.domain == ArithmeticDomain::Wrapping {
        diagnostics.push(Diagnostic::error(format!(
            "float-to-int cast `in Wrapping` in {owner}: there is no modular reading \
             of a float (ch5 cast ruling). Use `in Saturating` (NaN -> 0, clamp to \
             the target range) or `in Trapping` (trap on NaN/out-of-range) instead.",
        )));
    }
    // Exact is unchanged denotation, not a guarantee about a truncated result.
    // Every value permitted by the source facts must be integral and fit.
    if cast.domain == ArithmeticDomain::Exact
        && !super::super::float_source_bounds(
            walk.program,
            walk.machine,
            walk.state,
            walk.environment,
            cast.value,
        )
        .is_some_and(|bounds| bounds.is_exact_integer(target))
    {
        diagnostics.push(Diagnostic::error(format!(
            "Exact float-to-int cast in {owner} is not provably integral and representable in `{}`; \
             prove unchanged denotation or use a named conversion to truncate.",
            primitive_name(target),
        )));
    }
}

/// Exact integer coercion preserves the mathematical value. Width narrowing and
/// signedness changes therefore need a proof that the source interval fits the
/// complete target range; a cast is not an opt-in truncation/reinterpretation
/// surface. Widening succeeds from the source carrier's ordinary full-range
/// fact.
fn require_exact_integer_fit(
    walk: &ExpressionWalk,
    cast: &TableCastExpression,
    source: &Analysis,
    primitive: Option<PrimitiveType>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if cast.domain == ArithmeticDomain::Exact
        && let Some(source_primitive) = source
            .primitive
            .filter(|source| primitive_range(*source).is_some())
        && let Some(target) = primitive.filter(|target| primitive_range(*target).is_some())
        && !integer_interval_fits_primitive(source.interval, source_primitive, target)
    {
        diagnostics.push(Diagnostic::error(format!(
            "Exact integer cast in {} from `{}` to `{}` is not provably \
             representable; constrain the source with a range or dominating guard, \
             or use a named Wrapping, Saturating, or Trapping conversion.",
            walk.owner,
            source.primitive.map(primitive_name).unwrap_or("integer"),
            primitive_name(target),
        )));
    }
}

/// A Trapping numeric conversion traps exactly when its Exact counterpart is
/// not provably representable: an integer (or address) operand outside the
/// target carrier, or a float operand that is NaN or truncates outside it.
/// That same judgment, over this conversion's own operand and carriers,
/// decides whether the machine's published Trap ceiling covers the site.
fn require_trapping_conversion_coverage(
    walk: &ExpressionWalk,
    expression: typed_trees::expression::ExpressionHandle,
    cast: &TableCastExpression,
    source: &Analysis,
    primitive: Option<PrimitiveType>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if cast.domain != ArithmeticDomain::Trapping {
        return;
    }
    let Some(target) = primitive.filter(|target| primitive_range(*target).is_some()) else {
        return;
    };
    let Some(source_primitive) = source.primitive else {
        return;
    };
    let float_source = matches!(source_primitive, PrimitiveType::F32 | PrimitiveType::F64);
    if !float_source && primitive_range(source_primitive).is_none() {
        return;
    }
    let forms = |walk: &ExpressionWalk, source: &Analysis| {
        if float_source {
            float_source_proves_integer_conversion(
                walk.program,
                walk.machine,
                walk.state,
                walk.environment,
                cast.value,
                target,
            )
        } else {
            integer_interval_fits_primitive(source.interval, source_primitive, target)
        }
    };
    walk.require_trap_coverage(
        expression,
        || forms(walk, source),
        |covering| {
            let source = source_value(
                covering,
                cast,
                primitive,
                &mut Vec::new(),
                &mut super::ArithmeticDecisions::default(),
            );
            source.primitive == Some(source_primitive) && forms(covering, &source)
        },
        || {
            format!(
                "Trapping conversion of `{}` from `{}` to `{target}` traps when that value {} \
                 `{target}`",
                walk.operand_label(cast.value),
                primitive_name(source_primitive),
                if float_source {
                    "is NaN or truncates outside"
                } else {
                    "lies outside"
                },
                target = primitive_name(target),
            )
        },
        diagnostics,
    );
}

/// Numeric conversion of a Boolean preserves its binary value. Keep that
/// stronger fact instead of widening the result to the target's complete
/// carrier: foreign bitmask construction such as `(flag as i32) << 10` is
/// therefore ordinary provable Exact arithmetic, not a reason to weaken the
/// operation to Wrapping.
///
/// Exact integer coercion likewise preserves the mathematical source value.
/// Retain a sound source interval after the fit check so a widening conversion
/// remains useful proof evidence for enclosing arithmetic. Wrapped expressions
/// can carry a computed interval outside their carrier; in that case use the
/// carrier's full range rather than manufacturing an impossible intersection.
/// Same-carrier policy qualification also preserves payload and source bounds.
/// Other non-Exact casts retain the conservative target-carrier approximation.
fn result_interval(
    cast: &TableCastExpression,
    source: &Analysis,
    primitive: Option<PrimitiveType>,
) -> Interval {
    let target_is_integer = primitive.is_some_and(|target| integer_bit_width(target).is_some());
    if source.primitive == Some(PrimitiveType::Bool) && target_is_integer {
        return BOOLEAN_INTERVAL;
    }
    if (cast.domain == ArithmeticDomain::Exact || source.primitive == primitive)
        && let Some(source_primitive) = source.primitive
        && integer_bit_width(source_primitive).is_some()
        && target_is_integer
        && let Some(source_range) = primitive_range(source_primitive)
    {
        return if source_range.contains(source.interval) {
            source.interval
        } else {
            source_range
        };
    }
    primitive
        .and_then(primitive_range)
        .unwrap_or(Interval::UNBOUNDED)
}
