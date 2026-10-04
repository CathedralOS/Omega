//! Call results: the bound a call's value brings to enclosing arithmetic.
//!
//! Builtin and declared operators supply their own result semantics. Ordinary
//! calls intersect independent declared, selected-constant and ensured bounds;
//! one guarantee must not hide another. Body inference is optional when no
//! contract supplies a bound. Even an unknown value retains its declared
//! carrier and policy.
//! Unknown values are not untyped values, regardless of the call's receiver.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::float_arithmetic::resolve_named_float_arithmetic;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, primitive_range,
};
use crate::proof_contracts::arithmetic_domains::return_ranges::{
    ensured_call_result_interval, infer_return_interval, resolve_unique_self_call_state,
};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, Interval, TableCallExpression, TypedTrees,
    call_return_type, range_constraint_interval,
};
use symbols::BuiltinFunction;

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    // The shared expression walk has checked receiver and argument formation,
    // including calls whose result folds before reaching this result query.
    // Speculative interval queries below must not repeat those diagnostics.
    if let Some(result) = named_float_operation(walk, call, diagnostics) {
        return result;
    }
    let analysis = named_operator_result(walk.program, call)
        .or_else(|| bounding_builtin_result(walk, call))
        .unwrap_or_else(|| {
            match (
                selected_const_call_result(walk, expression),
                declared_return_range(walk, call),
            ) {
                (Some(selected), Some(declared)) => Analysis {
                    interval: selected.interval.intersect(declared.interval),
                    ..selected
                },
                (Some(analysis), None) | (None, Some(analysis)) => analysis,
                (None, None) => inferred_return_range(walk, call).unwrap_or(NEUTRAL),
            }
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

/// F7 named float requirements use the same operand-driven policy selection as
/// spellings. Preserve the selected domain for an enclosing expression and
/// reject two different explicit policies before checked evidence is built.
/// Classification calls return a non-float and therefore carry no float result
/// policy.
fn named_float_operation(
    walk: &ExpressionWalk,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Analysis> {
    let (_operator, return_primitive) = resolve_named_float_arithmetic(walk.program, call)?;
    let mut selected_domain: Option<ArithmeticDomain> = None;
    for argument in walk
        .program
        .expression_table
        .expression_handles(call.arguments)
    {
        let mut throwaway = Vec::new();
        let Some(domain) = walk.analyze(*argument, &mut throwaway).domain else {
            continue;
        };
        if let Some(selected) = selected_domain
            && selected != domain
        {
            diagnostics.push(Diagnostic::error(format!(
                "mixed arithmetic domains in {}: named float operation `{}` receives both \
                 `{}` and `{}` operands. Decision 17 forbids implicit domain mixing -- cross \
                 domains with an explicit `as` cast, or declare every operand in the same \
                 domain.",
                walk.owner,
                call.target,
                selected.name(),
                domain.name(),
            )));
        } else {
            selected_domain = Some(domain);
        }
    }
    Some(Analysis {
        domain: selected_domain,
        interval: Interval::UNBOUNDED,
        primitive: Some(return_primitive),
    })
}

/// A named operator's declared result supplies its carrier even when provider
/// selection has not supplied an executable body. Do not infer a body range or
/// repeatability from this signature.
fn named_operator_result(program: &TypedTrees, call: &TableCallExpression) -> Option<Analysis> {
    let operator = typed_trees::operator::resolve_named_expression_call(program, call)?;
    let primitive = program
        .primitive_type_reference(operator.return_type)
        .filter(|primitive| integer_bit_width(*primitive).is_some())?;
    let range = primitive_range(primitive)?;
    Some(Analysis {
        domain: Some(program.arithmetic_domain_for_type_reference(operator.return_type)),
        interval: range_constraint_interval(program, operator.return_type)
            .map(|declared| declared.intersect(range))
            .unwrap_or(range),
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
/// name + a free (receiverless) call + exactly two arguments. The caller checks
/// all arguments before result inference. This query independently requires
/// clean operand analyses before using their intervals, without duplicating
/// the diagnostics already emitted by that common argument walk.
fn bounding_builtin_result(walk: &ExpressionWalk, call: &TableCallExpression) -> Option<Analysis> {
    let builtin = BuiltinFunction::from_name(call.target.as_str());
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
    let left = walk.analyze(*left, &mut throwaway);
    let right = walk.analyze(*right, &mut throwaway);
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
fn exact_ordinary_actual_intervals(
    walk: &ExpressionWalk,
    call: &TableCallExpression,
) -> Option<Vec<(symbols::SymbolHandle, Interval)>> {
    if !call.selects_only_nominal_route() {
        return None;
    }
    let (callee, entry) =
        crate::machine_calls::calls::machine_state_by_symbol(walk.program, call.target_symbol)?;
    if walk.program.machine_states(callee).first()?.symbol != entry.symbol {
        return None;
    }
    let parameters = walk
        .program
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let arguments = walk
        .program
        .expression_table
        .expression_handles(call.arguments);
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
            let analysis = operand_walk.analyze(*argument, &mut diagnostics);
            diagnostics
                .is_empty()
                .then_some(Some((parameter.symbol, analysis.interval)))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(intervals.into_iter().flatten().collect())
}

/// ch15 stage 2 (modular return-range inference): with no DECLARED range, infer
/// the callee's return interval from its body (sound, permissive).
fn inferred_return_range(walk: &ExpressionWalk, call: &TableCallExpression) -> Option<Analysis> {
    let program = walk.program;
    let (callee, state) = resolve_unique_self_call_state(program, walk.machine, call)?;
    let primitive = program.primitive_type_reference(state.return_type);
    let interval = infer_return_interval(program, callee, state, primitive)?;
    Some(Analysis {
        domain: None,
        interval,
        primitive,
    })
}
