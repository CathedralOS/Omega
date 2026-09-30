//! Call results: the bound a call's value brings to enclosing arithmetic.
//!
//! Sources are tried from the most specific to the least: a named float
//! operation's policy, a named operator's declared carrier, a resolved static
//! machine's carrier, a selected constant call's result bounds, `min`/`max`
//! over their operands, a declared or ensured return range, and finally a
//! return interval inferred from the callee's body. A call none of them
//! describes stays neutral. Whichever source bounds the value, a callee's
//! declared result policy (`-> u32 in Wrapping`) is the call's domain.

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
    call_result_bounds, call_return_type, range_constraint_interval,
};
use symbols::BuiltinFunction;

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    // Call arguments are evaluated in the caller's environment at the call
    // site, so they owe their own decision-17 representability proofs exactly
    // like a statement-level expression. The result-bound passes below analyze
    // operands into throwaway buffers because they only need intervals; the
    // operands' own obligations are validated once here, into the caller's
    // diagnostics (a speculative caller still passes a throwaway buffer and
    // hears nothing). Previously these diagnostics were dropped entirely and
    // an unbounded operand surfaced at terminal proof reconstruction as an
    // opaque OperationProofUnavailable.
    //
    // The destination resets: an argument's type comes from the callee's
    // parameter, not the enclosing statement's target, so neither the target
    // primitive nor its declared domain may leak in.
    let operand_walk = walk.with_destination(None, ArithmeticDomain::Exact);
    if call.receiver.is_valid() {
        operand_walk.analyze(call.receiver, diagnostics);
    }
    for argument in walk
        .program
        .expression_table
        .expression_handles(call.arguments)
    {
        operand_walk.analyze(*argument, diagnostics);
    }
    if let Some(result) = named_float_operation(walk, call, diagnostics) {
        return result;
    }
    let analysis = named_operator_result(walk.program, call)
        .or_else(|| resolved_static_integer_call(walk.program, call))
        .or_else(|| selected_const_call_result(walk, expression))
        .or_else(|| bounding_builtin_result(walk, call))
        .or_else(|| declared_return_range(walk.program, call))
        .or_else(|| inferred_return_range(walk, call))
        .unwrap_or(NEUTRAL);
    with_declared_result_policy(walk.program, call, analysis)
}

/// Decision 17 is operand-driven: a call whose callee declares a qualified
/// result (`-> u32 in Wrapping`) contributes that policy to enclosing
/// arithmetic, exactly as a field or local of that type does. The range
/// sources above only bound the value; without this a receiver method's
/// Wrapping result read as Exact whenever its body defeated inference, and the
/// overflow diagnostic then asked for the annotation already present. An
/// Exact result keeps the neutral domain it had.
fn with_declared_result_policy(
    program: &TypedTrees,
    call: &TableCallExpression,
    analysis: Analysis,
) -> Analysis {
    if analysis.domain.is_some() {
        return analysis;
    }
    let Some(return_type) = call_return_type(program, call) else {
        return analysis;
    };
    let domain = program.arithmetic_domain_for_type_reference(return_type);
    if domain == ArithmeticDomain::Exact {
        return analysis;
    }
    Analysis {
        domain: Some(domain),
        primitive: analysis
            .primitive
            .or_else(|| program.primitive_type_reference(return_type)),
        ..analysis
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

/// A resolved monomorphic call without a self operand has its declared carrier
/// and policy even when no body or result-contract range is available. Unknown
/// values are not untyped values: their full carrier range still constrains
/// later operations. An attached static call retains its exact Data receiver
/// symbol, not a runtime place whose state or aliasing must be inferred.
fn resolved_static_integer_call(
    program: &TypedTrees,
    call: &TableCallExpression,
) -> Option<Analysis> {
    if !call.target_symbol.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    let mut targets = program.machines().iter().filter_map(|machine| {
        let state = program.machine_states(machine).first()?;
        (state.symbol == call.target_symbol).then_some((machine, state))
    });
    let (machine, state) = targets.next()?;
    if targets.next().is_some()
        || !program.machine_type_parameters(machine).is_empty()
        || !machine.lifetime_parameters.is_empty()
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self)
    {
        return None;
    }
    if call.receiver.is_valid()
        && !matches!(
            program.expression_table.expression(call.receiver),
            typed_trees::expression::ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && path.symbol == machine.attached_data_symbol
                    && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Data
        )
    {
        return None;
    }
    let primitive = program.primitive_type_reference(state.return_type)?;
    let carrier_range = primitive_range(primitive)?;
    let declared = range_constraint_interval(program, state.return_type)
        .map(|declared| declared.intersect(carrier_range))
        .unwrap_or(carrier_range);
    Some(Analysis {
        domain: Some(program.arithmetic_domain_for_type_reference(state.return_type)),
        interval: call_result_bounds::normal_return_interval(program, machine, state, declared),
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
/// name + a free (receiverless) call + exactly two arguments. Each operand is
/// analyzed into a THROWAWAY diagnostic buffer and its interval is trusted ONLY
/// if it proves clean -- the bound then rests on sound operand ranges, while an
/// unproven operand's diagnostics are dropped (call arguments are not otherwise
/// overflow-checked today, so this stays strictly permissive: it can only
/// tighten a previously-unbounded call result, never add a rejection).
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
/// domain. Narrow ONLY when the return type carries a range constraint -- a
/// plain `-> u32` stays NEUTRAL (opaque/unbounded, as before); attaching a bare
/// type's full range + primitive to an otherwise-unbounded call result would
/// turn a previously-unchecked expression into a spurious overflow. A callee's
/// `ensures result <= K` is the same enforced guarantee.
fn declared_return_range(program: &TypedTrees, call: &TableCallExpression) -> Option<Analysis> {
    let return_type = call_return_type(program, call)?;
    let primitive = program.primitive_type_reference(return_type);
    let declared = range_constraint_interval(program, return_type);
    let ensured = ensured_call_result_interval(program, call).map(|ensured| {
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
