//! Call results: the bound a call's value brings to enclosing arithmetic.
//!
//! Builtin and declared operators supply their own result semantics. Ordinary
//! calls intersect independent declared, selected-constant and ensured bounds;
//! one guarantee must not hide another. Body inference can tighten a declared
//! range using captured immutable actuals. Even an unknown value retains its declared
//! carrier and policy.
//! Unknown values are not untyped values, regardless of the call's receiver.
//! A named float call follows its selected signature too: operand policies do
//! not synthesize an adapter absent from that requirement. Binary-expression
//! policy selection belongs to the binary operation, not namespace spelling.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, primitive_range,
};
use crate::proof_contracts::arithmetic_domains::return_ranges::{
    ensured_call_result_interval, infer_return_interval,
};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, ExpressionHandle, Interval, TableCallExpression, TypedTrees,
    call_return_type, range_constraint_interval,
};
use symbols::BuiltinFunction;

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    call: &TableCallExpression,
) -> Analysis {
    // The shared expression walk has checked receiver and argument formation,
    // including calls whose result folds before reaching this result query.
    // Speculative interval queries below must not repeat those diagnostics.
    let analysis = named_operator_result(walk, call)
        .or_else(|| bounding_builtin_result(walk, call))
        .unwrap_or_else(|| {
            [
                selected_const_call_result(walk, expression),
                declared_return_range(walk, call),
                inferred_return_range(walk, call),
            ]
            .into_iter()
            .flatten()
            .reduce(|accepted, additional| Analysis {
                interval: accepted.interval.intersect(additional.interval),
                ..accepted
            })
            .unwrap_or(NEUTRAL)
        });
    with_declared_result_carrier(walk.program, call, analysis)
}

/// Decision 17 is operand-driven: a call whose callee declares a qualified
/// result (`-> u32 in Wrapping`) contributes that policy to enclosing
/// arithmetic, exactly as a field or local of that type does. The range
/// sources above only bound the value; without this a receiver method's
/// Wrapping result read as Exact whenever its body defeated inference, and the
/// overflow diagnostic then asked for the annotation already present. An
/// Exact result keeps the neutral domain it had, but not a missing carrier:
/// the declared type bounds an unknown value for ordinary and receiver calls
/// alike. Otherwise an unbounded call could evade its enclosing operation's
/// formation checks merely because it failed the static-call recognizer.
fn with_declared_result_carrier(
    program: &TypedTrees,
    call: &TableCallExpression,
    analysis: Analysis,
) -> Analysis {
    let Some(return_type) = call_return_type(program, call) else {
        return analysis;
    };
    let domain = program.arithmetic_domain_for_type_reference(return_type);
    let primitive = analysis
        .primitive
        .or_else(|| program.primitive_type_reference(return_type));
    Analysis {
        domain: analysis
            .domain
            .or((domain != ArithmeticDomain::Exact).then_some(domain)),
        primitive,
        interval: primitive
            .and_then(primitive_range)
            .map_or(analysis.interval, |carrier| {
                analysis.interval.intersect(carrier)
            }),
    }
}

/// A named operator's declared result supplies its carrier even when provider
/// selection has not supplied an executable body. Do not infer a body range or
/// repeatability from this signature.
fn named_operator_result(walk: &ExpressionWalk, call: &TableCallExpression) -> Option<Analysis> {
    let program = walk.program;
    let operator = typed_trees::operator::resolve_named_expression_call(program, call)?;
    let primitive = program
        .primitive_type_reference(operator.return_type)
        .filter(|primitive| integer_bit_width(*primitive).is_some())?;
    let range = primitive_range(primitive)?;
    let converted = (|| {
        let [operand] = program.expression_table.expression_handles(call.arguments) else {
            return None;
        };
        let source = super::super::float_source_bounds(
            program,
            walk.machine,
            walk.state,
            walk.environment,
            *operand,
        )?;
        let (minimum, maximum) = crate::float_integer_conversion_bounds(
            program,
            operator,
            source.minimum,
            source.maximum,
        )?;
        Some(Interval {
            low: minimum.to_i64(),
            high: maximum.to_i64(),
        })
    })();
    let declared = range_constraint_interval(program, operator.return_type).unwrap_or(range);
    Some(Analysis {
        domain: Some(program.arithmetic_domain_for_type_reference(operator.return_type)),
        interval: converted
            .map_or(declared, |converted| converted.intersect(declared))
            .intersect(range),
        primitive: Some(primitive),
    })
}

fn selected_const_call_result(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
) -> Option<Analysis> {
    let program = walk.program;
    let (return_type, minimum, maximum) =
        crate::proof_contracts::contract_entailment::selected_const_call_result_bounds(
            program,
            walk.machine,
            walk.state,
            expression,
        )?;
    let primitive = program.primitive_type_reference(return_type)?;
    let carrier = primitive_range(primitive)?;
    Some(Analysis {
        domain: Some(program.arithmetic_domain_for_type_reference(return_type)),
        interval: Interval {
            low: Some(minimum),
            high: Some(maximum),
        }
        .intersect(carrier),
        primitive: Some(primitive),
    })
}

/// S4: the `min`/`max` builtins bound their result by their operands' intervals
/// (`max(0, x)` is >= 0, `min(x, 100)` is <= 100), so a clamped value can feed
/// exact arithmetic instead of poisoning the enclosing op. Reserved-builtin
/// identity + a free (receiverless) call + exactly two arguments. A same-named
/// authored machine contributes its own checked result, not an operand bound.
/// The caller checks all arguments before result inference. This query requires
/// clean operand analyses before using their intervals, without duplicating
/// the diagnostics already emitted by that common argument walk.
fn bounding_builtin_result(walk: &ExpressionWalk, call: &TableCallExpression) -> Option<Analysis> {
    let builtin = walk
        .program
        .symbols
        .builtin_function_for_symbol(call.target_symbol);
    if call.receiver.is_valid()
        || !matches!(builtin, Some(BuiltinFunction::Min | BuiltinFunction::Max))
    {
        return None;
    }
    let [left, right] = walk
        .program
        .expression_table
        .expression_handles(call.arguments)
    else {
        return None;
    };
    let mut throwaway = Vec::new();
    let mut cast_queries = super::ArithmeticDecisions::default();
    let left = walk.analyze(*left, &mut throwaway, &mut cast_queries);
    let right = walk.analyze(*right, &mut throwaway, &mut cast_queries);
    if !throwaway.is_empty() {
        return None;
    }
    let interval = if builtin == Some(BuiltinFunction::Max) {
        left.interval.max_with(right.interval)
    } else {
        left.interval.min_with(right.interval)
    };
    let domain = match (left.domain, right.domain) {
        (Some(left_domain), Some(right_domain)) if left_domain == right_domain => Some(left_domain),
        (Some(domain), None) | (None, Some(domain)) => Some(domain),
        _ => None,
    };
    Some(Analysis {
        domain,
        interval,
        primitive: left.primitive.or(right.primitive),
    })
}

/// S4 return-range inference: a machine whose return type declares a literal
/// range constraint (`-> i32 [0..=10]`) is ENFORCED to return within that range
/// (validate_return_value_range), so a caller doing exact arithmetic on the
/// result can rely on the narrowed interval instead of being forced into a
/// domain. This owner supplies narrower declared/ensured bounds; the common
/// result-carrier step retains the full type range when no tighter fact exists.
/// A callee's `ensures result <= K` is the same enforced guarantee.
fn declared_return_range(walk: &ExpressionWalk, call: &TableCallExpression) -> Option<Analysis> {
    let program = walk.program;
    let return_type = call_return_type(program, call)?;
    let primitive = program.primitive_type_reference(return_type);
    let declared = range_constraint_interval(program, return_type);
    let actuals = exact_ordinary_actual_intervals(walk, call).unwrap_or_default();
    let ensured = ensured_call_result_interval(program, call, &actuals).map(|ensured| {
        primitive
            .and_then(primitive_range)
            .map_or(ensured, |carrier| ensured.intersect(carrier))
    });
    let interval = match (declared, ensured) {
        (Some(declared), Some(ensured)) => Some(declared.intersect(ensured)),
        (declared, ensured) => declared.or(ensured),
    }?;
    Some(Analysis {
        domain: None,
        interval,
        primitive,
    })
}

/// Pair immutable ordinary formals with the selected call's current actual
/// intervals before reading a relational result guarantee. This is a semantic
/// substitution, not initializer replay: expression analysis reads the
/// caller's already established environment at this call site. Receivers,
/// static dispatch and mutable parameters retain their dedicated owners.
/// A selector call's leading selector pairs with no formal of the contract.
fn exact_ordinary_actual_intervals(
    walk: &ExpressionWalk,
    call: &TableCallExpression,
) -> Option<Vec<(symbols::SymbolHandle, Interval)>> {
    if !call.selects_only_nominal_route() {
        return None;
    }
    let callee =
        crate::machine_calls::calls::callee_entry_contract(walk.program, call.target_symbol)?;
    let parameters = callee.parameters.iter().collect::<Vec<_>>();
    let arguments = walk
        .program
        .expression_table
        .expression_handles(call.arguments)
        .get(callee.leading_operands..)?;
    if parameters.len() != arguments.len() {
        return None;
    }
    let operand_walk = walk.with_destination(None, ArithmeticDomain::Exact);
    let intervals = parameters
        .into_iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            if parameter.is_mutable
                || parameter.is_const
                || !parameter.symbol.is_valid()
                || walk
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            {
                return Some(None);
            }
            let mut diagnostics = Vec::new();
            let analysis = operand_walk.analyze(
                *argument,
                &mut diagnostics,
                &mut super::ArithmeticDecisions::default(),
            );
            diagnostics
                .is_empty()
                .then_some(Some((parameter.symbol, analysis.interval)))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(intervals.into_iter().flatten().collect())
}

/// Infer an additional call-specific bound from a completely captured body.
/// This bound intersects the declared result range at the common call join.
fn inferred_return_range(walk: &ExpressionWalk, call: &TableCallExpression) -> Option<Analysis> {
    let program = walk.program;
    if !call.selects_only_nominal_route() || !call.machine_arguments.is_empty() {
        return None;
    }
    let (callee, state) =
        crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)?;
    if !program.machine_type_parameters(callee).is_empty() {
        return None;
    }
    let primitive = program.primitive_type_reference(state.return_type);
    let actuals = exact_ordinary_actual_intervals(walk, call).unwrap_or_default();
    let interval = infer_return_interval(program, callee, state, primitive, &actuals)?;
    Some(Analysis {
        domain: None,
        interval,
        primitive,
    })
}
