//! `as` conversions: the source value, the cast-ruling obligations, and the
//! interval the converted value keeps. Arithmetic policy qualifies future
//! operations; it never changes the denotation or discharges carrier fit.
//! Publish the same occurrence proof for every accepted target qualification
//! so later lowering consumes the decision without inventing a lossy cast.

use super::{Analysis, BOOLEAN_INTERVAL, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, integer_interval_fits_primitive, literal_interval, primitive_name,
    primitive_range, validate_anonymous_integer_primitive_range,
};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionNode, Interval, PrimitiveType, cast_ranges,
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
            recipe: None,
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
    require_float_to_integer_denotation(walk, cast, &source, primitive, diagnostics);
    let exact_fit = require_exact_integer_fit(walk, expression, &source, primitive, diagnostics);
    if !diagnostics[first_diagnostic..]
        .iter()
        .any(Diagnostic::is_error)
        && let (Some(source_type), Some(target_type)) = (source.primitive, primitive)
        && source_type != target_type
        && primitive_range(source_type).is_some()
        && primitive_range(target_type).is_some()
        && exact_fit
        && let Some((minimum, maximum)) =
            checked_cast_bounds(source_interval, source_type, target_type)
    {
        decisions
            .exact_integer_casts
            .push(crate::ExactIntegerCastFact {
                expression,
                source_type,
                target_type,
                minimum,
                maximum,
            });
    }
    Analysis {
        domain: Some(cast.domain),
        interval: result_interval(&source, primitive),
        recipe: source
            .primitive
            .filter(|source| source.is_fixed_integer())
            .zip(primitive.filter(|target| target.is_fixed_integer()))
            .and(source.recipe),
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
        Some(literal) => {
            let interval = literal_interval(&literal);
            // Anonymous arithmetic lands as one closed value without the
            // typed operation walk. Publish that same checked operand value;
            // qualification must not need another evaluator or target premise.
            decisions.record_interval(cast.value, interval);
            Analysis {
                recipe: None,
                domain: None,
                interval,
                primitive: Some(primitive),
            }
        }
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

/// Arithmetic policy qualifies future operations; every float-to-integer
/// coercion must preserve the source value, independently of that policy.
fn require_float_to_integer_denotation(
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
    // Unchanged denotation is not a guarantee about a truncated result.
    // Every value permitted by the source facts must be integral and fit.
    if !super::super::float_source_bounds(walk.program, walk.environment, cast.value, &|value| {
        walk.state
            .and_then(|state| {
                crate::value_custody::expression_types::expression_result_type_reference(
                    walk.program,
                    walk.machine,
                    state,
                    value,
                )
            })
            .or_else(|| {
                crate::value_custody::places::declared_place_type_raw(
                    walk.program,
                    walk.machine,
                    walk.state,
                    value,
                )
            })
    })
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
/// fact. Target policy governs later operations, so it never waives this proof.
fn require_exact_integer_fit(
    walk: &ExpressionWalk,
    expression: typed_trees::expression::ExpressionHandle,
    source: &Analysis,
    primitive: Option<PrimitiveType>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(source_primitive) = source
        .primitive
        .filter(|source| primitive_range(*source).is_some())
    else {
        return false;
    };
    let Some(target) = primitive.filter(|target| primitive_range(*target).is_some()) else {
        return false;
    };
    let fits = integer_interval_fits_primitive(source.interval, source_primitive, target)
        || (walk.checks_occurrence_fit
            && (source_primitive == PrimitiveType::Addr || target == PrimitiveType::Addr)
            && walk.state.is_some_and(|state| {
                crate::proof_contracts::contract_entailment::exact_cast_fits_at_occurrence(
                    walk.program,
                    walk.machine,
                    state,
                    expression,
                )
            }));
    if !fits && walk.checks_occurrence_fit {
        let source_span = walk.program.expression_table.source_span(expression);
        // The executing scanner and enclosing arithmetic walk can both reach
        // this same authored coercion. Its occurrence owns one obligation;
        // changing the outer walk's owner must not multiply the diagnostic.
        if source_span.span.start < source_span.span.end
            && diagnostics.iter().any(|existing| {
                existing.source_span == Some(source_span)
                    && existing.message.starts_with("Exact integer cast in ")
            })
        {
            return false;
        }
        diagnostics.push(
            Diagnostic::error(format!(
                "Exact integer cast in machine `{}`{} from `{}` to `{}` is not provably \
                 representable; {}",
                walk.machine.name,
                walk.state
                    .map(|state| format!(" state `{}`", state.name))
                    .unwrap_or_default(),
                primitive_name(source_primitive),
                primitive_name(target),
                if target == PrimitiveType::Addr {
                    "prove 0 <= embed(source) < addr::Bound at this occurrence; count and address widths are independent."
                } else if source_primitive == PrimitiveType::Addr {
                    "prove the embedded address fits the destination integer range at this occurrence; count and address widths are independent."
                } else {
                    "constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion."
                },
            ))
            .with_source_span(source_span),
        );
    }
    fits
}

/// Retain payload bounds only after fit has been established. A symbolic
/// address ceiling cannot be squeezed into an i64 interval; fixed carriers
/// supply their exact endpoints, and an address-to-fixed proof supplies the
/// destination endpoint. Unknown never means infinity or successful fit.
fn checked_cast_bounds(
    interval: Interval,
    source: PrimitiveType,
    target: PrimitiveType,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use numerics::bignum::BigInt;
    let source_bounds = (source != PrimitiveType::Addr)
        .then(|| source.integer_bounds())
        .flatten();
    let target_bounds = (target != PrimitiveType::Addr)
        .then(|| target.integer_bounds())
        .flatten();
    let minimum = interval
        .low
        .map(i128::from)
        .or_else(|| source_bounds.map(|bounds| bounds.0))?
        .max(target_bounds.map_or(0, |bounds| bounds.0));
    let maximum = interval
        .high
        .map(i128::from)
        .or_else(|| source_bounds.map(|bounds| bounds.1))
        .or_else(|| target_bounds.map(|bounds| bounds.1))?;
    let maximum = target_bounds.map_or(maximum, |bounds| maximum.min(bounds.1));
    (minimum <= maximum).then(|| (BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
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
/// Target policy qualifies later operations and preserves these payload bounds.
fn result_interval(source: &Analysis, primitive: Option<PrimitiveType>) -> Interval {
    let target_is_integer = primitive.is_some_and(|target| integer_bit_width(target).is_some());
    if source.primitive == Some(PrimitiveType::Bool) && target_is_integer {
        return BOOLEAN_INTERVAL;
    }
    if let Some(source_primitive) = source.primitive
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
