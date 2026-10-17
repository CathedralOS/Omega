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
    ArithmeticDomain, ExpressionHandle, Interval, TableCallExpression, call_return_type,
    range_constraint_interval,
};

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    call: &TableCallExpression,
) -> Analysis {
    // The shared expression walk has checked receiver and argument formation,
    // including calls whose result folds before reaching this result query.
    // Speculative interval queries below must not repeat those diagnostics.
    let analysis = named_operator_result(walk, call).unwrap_or_else(|| {
        [
            selected_const_call_result(walk, expression),
            declared_return_range(walk, call),
            inferred_return_range(walk, call),
            returned_referent_range(walk, expression, call),
        ]
        .into_iter()
        .flatten()
        .reduce(|accepted, additional| Analysis {
            recipe: accepted.recipe.or(additional.recipe),
            interval: accepted.interval.intersect(additional.interval),
            ..accepted
        })
        .unwrap_or(NEUTRAL)
    });
    let mut analysis = with_result_carrier(walk, expression, analysis);
    if call.layout_query.is_some() {
        analysis.recipe = Some(checked_trees::IntegerRangeRecipe::observation(
            expression,
            walk.machine.symbol,
        ));
    }
    analysis
}

/// Observing a readable scalar-reference result reads the returned storage.
/// The ordinary result-origin relation chooses its formal; the captured call
/// operands choose the caller storage. Neither the helper's name nor a
/// same-typed argument proves that relation. Preserve an incoming bound only
/// when the complete call frame cannot write the selected referent.
fn returned_referent_range(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    call: &TableCallExpression,
) -> Option<Analysis> {
    let program = walk.program;
    let result = crate::reference_shell(program, call_return_type(program, call)?)?;
    if !result.access.is_readable() {
        return None;
    }
    let primitive = program.primitive_type_reference(result.referee)?;
    let callee = crate::machine_calls::calls::callee_entry_contract(program, call.target_symbol)?;
    let frames = crate::CallFrameResolver::new(program)?;
    let (_, selected_state) =
        crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)?;
    let returned = frames.reference_result_origins(selected_state.symbol)?;
    let actuals = frames.expression_call_write_origins(walk.machine, expression)?;
    let writes = frames.expression_may_write_paths(walk.machine, expression)?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut interval = None;
    for source in returned {
        if !source.segments.is_empty()
            || source.collection_coarse
            || !source.recasts.is_empty()
            || source.representation.is_some()
        {
            return None;
        }
        let position = callee
            .parameters
            .iter()
            .position(|parameter| parameter.symbol == source.root)?;
        let position = position.checked_add(callee.leading_operands)?;
        let origins = actuals.arguments.get(position)?.as_ref()?;
        if origins.is_empty()
            || origins.iter().any(|origin| {
                origin.collection_coarse
                    || !origin.recasts.is_empty()
                    || origin.representation.is_some()
                    || writes
                        .iter()
                        .any(|written| crate::frame_paths_overlap(written, &origin.path))
            })
        {
            return None;
        }
        let mut actual = *arguments.get(position)?;
        if let Some(borrow) = program.expression_table.borrow_expression(actual) {
            if !borrow.access.is_readable() {
                return None;
            }
            actual = borrow.target;
        }
        let mut diagnostics = Vec::new();
        let analysis = walk
            .with_destination(None, ArithmeticDomain::Exact)
            .analyze(
                actual,
                &mut diagnostics,
                &mut super::ArithmeticDecisions::default(),
            );
        if !diagnostics.is_empty() || analysis.primitive != Some(primitive) {
            return None;
        }
        interval = Some(interval.map_or(analysis.interval, |previous: Interval| {
            previous.union(analysis.interval)
        }));
    }
    Some(Analysis {
        recipe: None,
        interval: interval?,
        primitive: Some(primitive),
        domain: Some(program.arithmetic_domain_for_type_reference(result.referee)),
    })
}

/// Decision 17 is operand-driven: a call whose callee declares a qualified
/// result (`-> u32 in Wrapping`) contributes that policy to enclosing
/// arithmetic, exactly as a field or local of that type does. The range
/// sources above only bound the value; without this a receiver method's
/// Wrapping result read as Exact whenever its body defeated inference, and the
/// overflow diagnostic then asked for the annotation already present.
/// Exact is a declared result policy too, not the absence of one. Bounds can
/// be unknown without making a typed call result anonymous: omitting Exact
/// here would let a store silently add Wrapping, Saturating or Trapping to
/// that result. Read the computed result through the shared type owner: a
/// prepared token call can retain a selected policy adapter beyond its raw
/// callee signature. Re-reading that signature here would silently erase the
/// adapter and disagree with ordinary value delivery. State and declaration
/// scopes supply their own places to that same result query; neither a
/// destination nor inferred bounds chooses the carrier or policy.
fn with_result_carrier(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    analysis: Analysis,
) -> Analysis {
    let program = walk.program;
    let result = match walk.state {
        Some(state) => {
            crate::expression_result_type_reference(program, walk.machine, state, expression)
        }
        None => {
            let parameters = program
                .machine_states(walk.machine)
                .first()
                .map_or(&[][..], |entry| program.state_parameters(entry));
            crate::parameter_expression_result_type_reference(
                program,
                walk.machine.symbol,
                parameters,
                expression,
            )
        }
    };
    let Some(return_type) = result else {
        return analysis;
    };
    let domain = program.arithmetic_domain_for_type_reference(return_type);
    let primitive = analysis
        .primitive
        .or_else(|| program.primitive_type_reference(return_type));
    Analysis {
        recipe: analysis.recipe,
        domain: Some(analysis.domain.unwrap_or(domain)),
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
        let source =
            super::super::float_source_bounds(program, walk.environment, *operand, &|value| {
                walk.state
                    .and_then(|state| {
                        crate::value_custody::expression_types::expression_result_type_reference(
                            program,
                            walk.machine,
                            state,
                            value,
                        )
                    })
                    .or_else(|| {
                        crate::value_custody::places::declared_place_type_raw(
                            program,
                            walk.machine,
                            walk.state,
                            value,
                        )
                    })
            })?;
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
        recipe: None,
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
        recipe: None,
        domain: Some(program.arithmetic_domain_for_type_reference(return_type)),
        interval: Interval {
            low: Some(minimum),
            high: Some(maximum),
        }
        .intersect(carrier),
        primitive: Some(primitive),
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
    let values: Vec<_> = actuals
        .iter()
        .map(|(symbol, interval, _)| (*symbol, *interval))
        .collect();
    let lengths = argument_collection_lengths(walk, call);
    let concrete = super::super::call_result_bounds::ArgumentBounds { values, lengths };
    let ensured = ensured_call_result_interval(program, call, &concrete).map(|ensured| {
        primitive
            .and_then(primitive_range)
            .map_or(ensured, |carrier| ensured.intersect(carrier))
    });
    let interval = match (declared, ensured) {
        (Some(declared), Some(ensured)) => Some(declared.intersect(ensured)),
        (declared, ensured) => declared.or(ensured),
    }?;
    Some(Analysis {
        recipe: None,
        domain: None,
        interval,
        primitive,
    })
}

/// A borrowed slice carries its extent by value, even when its elements are
/// writable. Only a single reference to the collection has that property:
/// a reference to a descriptor can replace it and must not import its entry
/// extent as a normal-return observation. The existing geometry owner rejects
/// unstable saved initializers and owns range formation and selected operators.
fn argument_collection_lengths(
    walk: &ExpressionWalk,
    call: &TableCallExpression,
) -> Vec<(symbols::SymbolHandle, Interval)> {
    use typed_trees::types::TypeReferenceNode;
    let Some(state) = walk.state else {
        return Vec::new();
    };
    let Some(callee) =
        crate::machine_calls::calls::callee_entry_contract(walk.program, call.target_symbol)
            .filter(|_| call.selects_only_nominal_route())
    else {
        return Vec::new();
    };
    let arguments = walk
        .program
        .expression_table
        .expression_handles(call.arguments);
    let Some(arguments) = arguments.get(callee.leading_operands..) else {
        return Vec::new();
    };
    if arguments.len() != callee.parameters.len() {
        return Vec::new();
    }
    callee
        .parameters
        .iter()
        .zip(arguments)
        .filter_map(|(parameter, argument)| {
            if parameter.is_const {
                return None;
            }
            let mut reference = parameter.type_reference;
            if let TypeReferenceNode::Reference { referee, .. } =
                walk.program.type_reference_table.type_reference(reference)
            {
                let (machine, entry) = crate::machine_calls::calls::machine_state_by_symbol(
                    walk.program,
                    call.target_symbol,
                )?;
                if !crate::state_reference_parameter_binding_is_stable(
                    walk.program,
                    machine,
                    entry,
                    parameter.symbol,
                ) {
                    return None;
                }
                reference = *referee;
            }
            if !matches!(
                walk.program.type_reference_table.type_reference(reference),
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
            ) {
                return None;
            }
            let closed =
                crate::closed_collection_length(walk.program, walk.machine, state, *argument)
                    .and_then(|length| i64::try_from(length).ok())
                    .map(Interval::constant);
            let carried = super::super::carried_slice_length(
                walk.program,
                walk.machine,
                Some(state),
                walk.environment,
                *argument,
            );
            let length = match (closed, carried) {
                (Some(closed), Some(carried)) => closed.intersect(carried),
                (Some(length), None) | (None, Some(length)) => length,
                (None, None) => return None,
            };
            Some((parameter.symbol, length))
        })
        .collect()
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
) -> Option<
    Vec<(
        symbols::SymbolHandle,
        Interval,
        Option<checked_trees::IntegerRangeRecipe>,
    )>,
> {
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
            diagnostics.is_empty().then_some(Some((
                parameter.symbol,
                analysis.interval,
                analysis.recipe,
            )))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(intervals.into_iter().flatten().collect())
}

/// Infer an additional call-specific bound from a completely captured body.
/// This bound intersects the declared result range at the common call join.
fn inferred_return_range(walk: &ExpressionWalk, call: &TableCallExpression) -> Option<Analysis> {
    let program = walk.program;
    if !walk.infer_call_bodies
        || !call.selects_only_nominal_route()
        || !call.machine_arguments.is_empty()
    {
        return None;
    }
    let (callee, state) =
        crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)?;
    if !program.machine_type_parameters(callee).is_empty() {
        return None;
    }
    let primitive = program.primitive_type_reference(state.return_type);
    let actuals = exact_ordinary_actual_intervals(walk, call).unwrap_or_default();
    let (interval, recipe) = infer_return_interval(program, callee, state, primitive, &actuals)?;
    Some(Analysis {
        recipe,
        domain: None,
        interval,
        primitive,
    })
}
