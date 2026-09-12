//! Cast target predicates are obligations, never source interval premises.
//! Policy qualification cannot repair failed initial membership. Inspect every
//! authored range shell; unsupported bounds must not disappear through an
//! optional interval query that also represents absence of a range.

use super::{Interval, ValueEnv, known_u64_value, primitive_range};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeConstraintNode, TypeReferenceNode};

/// Record a successfully checked literal store at its statement point. Cast
/// predicates read this live fact; searching declarations later would ignore
/// intervening writes and permit forward-reference or cross-state assumptions.
pub(crate) fn record_float_literal_assignment(
    program: &TypedTrees,
    env: &mut ValueEnv,
    path: Option<String>,
    destination: Option<PrimitiveType>,
    value: ExpressionHandle,
) {
    let Some(path) = path else {
        return;
    };
    let Some(destination @ (PrimitiveType::F32 | PrimitiveType::F64)) = destination else {
        return;
    };
    let ExpressionNode::Float(literal) = program.expression_table.expression(value) else {
        return;
    };
    let format = match destination {
        PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
        _ => numerics::literals::FloatFormat::F64,
    };
    if literal.landing().is_some_and(|retained| retained != format) {
        return;
    }
    let value = literal.with_landing(format).landed_f64();
    if value.is_finite() {
        env.narrow_float(
            path.clone(),
            super::FloatInterval {
                low: Some(value),
                high: Some(value),
            },
        );
        env.mark_non_nan(path);
    }
}

/// The expression scanner owns selected reachability and calls this only for
/// an executing cast. Arithmetic analysis owns the proof from the current live
/// environment; a target annotation never substitutes for that analysis.
pub(crate) fn validate_range_cast_at_use(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    env: &ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return;
    };
    if cast.form.is_recast() {
        return;
    }
    let mut reference = cast.target_type;
    let mut visited = Vec::new();
    let mut has_range = false;
    while program
        .type_reference_table
        .contains_type_reference(reference)
        && !visited.contains(&reference)
    {
        visited.push(reference);
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            break;
        };
        has_range |= program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| matches!(constraint, TypeConstraintNode::Range { .. }));
        reference = *base_type;
    }
    if !has_range {
        return;
    }
    let owner = format!(
        "machine `{}` state `{}` cast expression",
        machine.name, state.name
    );
    let mut found = Vec::new();
    super::validate_arithmetic_domains(
        program,
        machine,
        Some(state),
        expression,
        env,
        None,
        numerics::arithmetic::ArithmeticDomain::Exact,
        &owner,
        &mut found,
    );
    for diagnostic in found {
        let duplicate = diagnostics.iter().any(|existing| {
            existing.source_span == diagnostic.source_span
                && (existing.message == diagnostic.message
                    // Root and nested scans describe different enclosing
                    // owners, but this is the same source membership failure.
                    || (existing.message.starts_with("cast target range in ")
                        && diagnostic.message.starts_with("cast target range in ")))
        });
        if !duplicate {
            diagnostics.push(diagnostic);
        }
    }
}

pub(super) fn validate_target_ranges(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    cast: &TableCastExpression,
    source_interval: Interval,
    source_primitive: Option<PrimitiveType>,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source_bounds = source_primitive.and_then(|primitive| {
        let carrier = primitive_range(primitive)?;
        if let Some(value) = known_u64_value(program, env, cast.value) {
            let value = i128::from(value);
            return Some((value, value));
        }
        Some((
            i128::from(source_interval.low.or(carrier.low)?),
            source_interval
                .high
                .or(carrier.high)
                .map(i128::from)
                .or_else(|| {
                    matches!(primitive, PrimitiveType::U64 | PrimitiveType::Addr)
                        .then_some(i128::from(u64::MAX))
                })?,
        ))
    });
    let mut reference = cast.target_type;
    let mut visited = Vec::new();
    while program
        .type_reference_table
        .contains_type_reference(reference)
        && !visited.contains(&reference)
    {
        visited.push(reference);
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            break;
        };
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range { minimum, maximum } = constraint else {
                continue;
            };
            let integer_proven = source_primitive.and_then(primitive_range).is_some()
                && program
                    .primitive_type_reference(cast.target_type)
                    .and_then(primitive_range)
                    .is_some()
                && integer_bound(program, *minimum)
                    .zip(integer_bound(program, *maximum))
                    .is_some_and(|(minimum, maximum)| {
                        minimum <= maximum
                            && source_bounds.is_some_and(|(low, high)| {
                                minimum <= low && low <= high && high <= maximum
                            })
                    });
            let float_proven =
                float_membership(program, machine, state, cast, env, *minimum, *maximum);
            if !integer_proven && !float_proven {
                let span = Some(program.expression_table.source_span(cast.value));
                if diagnostics.iter().any(|existing| {
                    existing.source_span == span
                        && existing.message.starts_with("cast target range in ")
                }) {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "cast target range in {owner} is not proven from the source value; all target predicates must hold before policy qualification (unknown numeric bounds or unsupported non-integer membership require an explicit proof)"
                )).with_source_span(program.expression_table.source_span(cast.value)));
            }
        }
        reference = *base_type;
    }
}

fn float_membership(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    cast: &TableCastExpression,
    env: &ValueEnv,
    minimum: ExpressionHandle,
    maximum: ExpressionHandle,
) -> bool {
    let Some(target @ (PrimitiveType::F32 | PrimitiveType::F64)) =
        program.primitive_type_reference(cast.target_type)
    else {
        return false;
    };
    let source = match program.expression_table.expression(cast.value) {
        ExpressionNode::Float(literal) => match literal.landing() {
            Some(numerics::literals::FloatFormat::F32) => Some(PrimitiveType::F32),
            Some(numerics::literals::FloatFormat::F64) | None => Some(PrimitiveType::F64),
        },
        _ => super::declared_place_type_raw(program, machine, state, cast.value)
            .and_then(|reference| program.primitive_type_reference(reference)),
    };
    if source != Some(target) {
        return false;
    }
    let bounds = |expression| {
        if let Some(value) = super::float_literal_value(program, expression) {
            return value.is_finite().then_some((value, value));
        }
        let path = super::place_path(program, expression)?;
        let (interval, non_nan) = env.float_fact(&path);
        let (low, high) = (interval.low?, interval.high?);
        (non_nan && low.is_finite() && high.is_finite() && low <= high).then_some((low, high))
    };
    match (bounds(cast.value), bounds(minimum), bounds(maximum)) {
        (Some((source_low, source_high)), Some((_, minimum_high)), Some((maximum_low, _))) => {
            minimum_high <= source_low && source_high <= maximum_low
        }
        _ => false,
    }
}

fn integer_bound(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<i128> {
    let value = crate::closed_integer_range_bound(program, expression)?;
    value
        .to_i64()
        .map(i128::from)
        .or_else(|| value.to_u64().map(i128::from))
}
