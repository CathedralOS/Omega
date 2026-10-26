//! Psi pre-resolution bridge for machine calls in const-generic arguments
//! (`Buffer<table_size()>`). Generic record instances must be synthesized
//! before the ordinary frontend runs, while the established build-time
//! evaluator needs typed trees. Prepare a retained declaration frontier,
//! complete only the probe's exact implementation dependencies, type it,
//! reuse the const-initializer admission gate and scalar evaluator, then
//! substitute the complete canonical argument before monomorphization.
//!
//! Calls with closed scalar arguments (`Buffer<sized(4)>`) are ordinary
//! applications of the same kind: the whole authored expression is evaluated
//! as a scalar probe on a checked program so each named call goes through
//! exact selected-entry resolution, concrete argument snapshots, and
//! premise/failure discharge — the same admission as const-initializer calls —
//! before its canonical result replaces the entire argument expression.
//!
//! Declared range endpoints inside those arguments are const positions of the
//! same kind: `RangeValue<u64[0..=limit()]>` can only synthesize its instance
//! once the canonical interval is known, so each authored endpoint call leaf is
//! folded through the shared typed endpoint gate on the probe program and its
//! evaluated literal is written back before resolution. Folding here does not
//! replace `range_endpoints`: the probe evaluation applies the same resolved
//! machine identity, closed-argument, result-carrier, and dependency checks,
//! while calls it cannot close keep their authored expression for ordinary
//! post-typing admission.

use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

mod dependencies;
mod extent_entries;

pub fn evaluate_const_generic_calls(
    mut syntax: SyntaxTrees,
    context: crate::BuildTimeSources<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    loop {
        // Readiness precedes admission: a provisional carrier can reveal exact
        // dependencies but must not supply an executable layout. Every layer
        // settles original argument handles, so at most N layers can progress.
        // Provider-waiting leaves (and their dependants) remain authored.
        let (evaluated, progress) = evaluate_application_group(syntax, context.clone())?;
        syntax = evaluated;
        if !progress {
            return Ok(syntax);
        }
    }
}

fn evaluate_application_group(
    mut syntax: SyntaxTrees,
    context: crate::BuildTimeSources<'_>,
) -> Result<(SyntaxTrees, bool), Vec<Diagnostic>> {
    let mut progress = false;
    let crate::BuildTimeSources {
        sources,
        source_scoped_top_level_bindings,
        selection_authority,
    } = context;
    let mut const_arguments =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_data_const_argument_expressions(
            &syntax, sources.clone(), source_scoped_top_level_bindings.to_vec(),
        )?;
    const_arguments.extend(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_machine_const_arguments(
            &syntax,
            sources.clone(),
            source_scoped_top_level_bindings.to_vec(),
        )?,
    );
    let extent_arguments =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_array_extent_expressions(
            &mut syntax,
        );
    let extent_uses = extent_arguments
        .iter()
        .map(|(use_handle, ..)| *use_handle)
        .collect::<Vec<_>>();
    const_arguments.extend(extent_arguments);
    // Work belongs to reachable closed uses. Open templates and detached
    // expression arena entries retain recipes without execution demand.
    let mut applications = Vec::new();
    for (type_reference, ..) in &const_arguments {
        if let TypeReferenceNode::ConstExpression(expression) =
            syntax.type_references.type_reference(*type_reference)
            && contains_call(&syntax, *expression)
            && !applications
                .iter()
                .any(|(existing, _)| existing == type_reference)
        {
            applications.push((*type_reference, *expression));
        }
    }
    // Range endpoints nested in generic arguments also feed synthesis. Only
    // their call leaves are collected here; the probe's shared endpoint fold
    // decides which of them can actually close.
    let mut endpoint_leaves = Vec::new();
    collect_generic_range_endpoint_call_leaves(&syntax, &mut endpoint_leaves);
    if endpoint_leaves.is_empty() && applications.is_empty() {
        return Ok((syntax, false));
    }

    // A standalone probe cannot supply the caller's lexical bindings. Resolve
    // those in their authored lexical owner, then rejoin exact
    // constant and call declarations after typed invocation preparation.
    let mut lexical_selections = Vec::new();
    if !applications.is_empty() {
        let resolved = syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_const_argument_selection(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: syntax.clone(),
                sources: sources.clone(),
                top_level_bindings: source_scoped_top_level_bindings.to_vec(),
            },
        )?;
        for (argument, expression) in &applications {
            let failure = |reason| {
                vec![
                    Diagnostic::error(reason)
                        .with_source_span(syntax.expressions.source_span(*expression)),
                ]
            };
            let origins = super::const_generic_expressions::lexical_selection::retain(
                &syntax,
                &resolved,
                *expression,
            )
            .map_err(&failure)?;
            let calls = super::const_generic_expressions::lexical_selection::retain_calls(
                &syntax,
                &resolved,
                *expression,
            )
            .map_err(failure)?;
            lexical_selections.push((*argument, origins, calls));
        }
    }

    // Whole-expression application probes need the declared const-argument
    // destination: the const parameter's own type reference supplies the
    // landing carrier. An application outside a declared const position has
    // nothing to land on and is rejected rather than deferred.
    let mut application_destinations = Vec::new();
    if !applications.is_empty() {
        for (type_reference, expression) in &applications {
            let Some((_, destination, public)) = const_arguments
                .iter()
                .find(|(argument, _, _)| argument == type_reference)
            else {
                return Err(vec![Diagnostic::error(
                    "a const-generic application call must occupy a declared const argument destination",
                )
                .with_source_span(syntax.expressions.source_span(*expression))]);
            };
            application_destinations.push((*type_reference, *expression, *destination, *public));
        }
    }

    // Demand only private probe declarations. Surrounding applications retain
    // their authored pending indices until their own evaluation settles; no
    // provisional index may discharge a structural equation or select layout.
    let mut probe = syntax.clone();
    crate::syntax_probes::defer_pending_const_qualifications(
        &mut probe,
        &application_destinations
            .iter()
            .map(|(argument, ..)| *argument)
            .collect::<Vec<_>>(),
    );
    for (type_reference, ..) in &application_destinations {
        probe
            .type_references
            .retain_pending_const_argument_uses(*type_reference);
    }
    let declaration_frontier = probe.root_item_handles().len();
    let owners = application_destinations
        .iter()
        .enumerate()
        .map(|(ordinal, (_, expression, destination, _))| {
            super::const_generic_expressions::append_probe(
                &mut probe,
                ordinal,
                *expression,
                *destination,
            )
        })
        .collect::<Vec<_>>();
    let declarations = probe.root_item_handles()[declaration_frontier..].to_vec();
    let probe = crate::syntax_probes::normalize_initializer_probe_data(
        probe,
        sources.clone(),
        source_scoped_top_level_bindings,
        None,
        &declarations,
    )?;
    let endpoint_references = endpoint_leaves
        .iter()
        .map(|(_, source)| *source)
        .collect::<Vec<_>>();
    let frontier =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_initializer_probe(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: probe,
                sources,
                top_level_bindings: source_scoped_top_level_bindings.to_vec(),
            },
        )?;
    let owner_symbols = owners
        .iter()
        .map(|owner| owner.resolve(frontier.trees()))
        .collect::<Result<Vec<_>, _>>()?;
    let preparation = crate::syntax_probes::prepare_owned_probe_evaluation(
        frontier,
        &owner_symbols,
        &endpoint_references,
    )?;
    // Every call-bearing extent belongs to the length continuation. Callee
    // types may themselves depend on other extents: scalar application checks
    // cannot replace the length owner's prerequisite and provider settlement.
    if extent_entries::retain(
        &mut syntax,
        &preparation.typed,
        &application_destinations,
        &owner_symbols,
        &extent_uses,
        &lexical_selections,
        selection_authority.as_deref(),
    )? {
        return Ok((syntax, true));
    }
    let layer = dependencies::ready_layer(
        &preparation,
        &application_destinations,
        &owner_symbols,
        &syntax,
    )?;
    // This pre-resolution probe has no Omega provider plan. Keep authored
    // operator uses intact: visible satisfiers cannot authorize execution.

    if !layer.arguments.is_empty() {
        // The appended probe machines are excluded from ordinary checking;
        // their expressions are evaluated by the exact scalar evaluator with
        // checked call admission, matching const-initializer invocations.
        let probe_symbols = layer
            .arguments
            .iter()
            .map(|ordinal| owner_symbols[*ordinal])
            .collect::<Vec<_>>();
        // Decide deferral before settled checking: a sibling fixed-array
        // length may need the same pending provider selection. No execution
        // is admitted here; independent calls still require checked bodies,
        // and deferred calls retain their ordinary typed continuation.
        let ready_domain = preparation.domain_for_roots(&probe_symbols);
        let prepared =
            crate::PreparedBuildMachineProgram::prepare_in(&preparation.typed, &ready_domain)?;
        let execution = prepared.typed();
        let admission =
            crate::BuildTimeAdmissionPlan::infer(execution, selection_authority.clone());
        let operators = validation::derive_pre_flow_operator_selections(execution);
        let independent = probe_symbols
            .iter()
            .copied()
            .filter(|symbol| {
                !admission.closure_needs_operator_selection(execution, *symbol, &operators)
            })
            .collect::<Vec<_>>();
        // Deferred provider uses are not part of this layer's checking domain.
        // A dependent application remains authored until its prerequisite is
        // settled under the later selected-provider continuation.
        let independent_domain = preparation.domain_for_roots(&independent);
        let independent_program = crate::PreparedBuildMachineProgram::prepare_in(
            &preparation.typed,
            &independent_domain,
        )?;
        let mut checked_program = None;
        for ordinal in &layer.arguments {
            let (argument, expression, _, public) = &application_destinations[*ordinal];
            let reference = syntax.expressions.source_span(*expression);
            let failure = |reason: String| {
                vec![
                    Diagnostic::error(format!(
                        "const-generic application evaluation failed: {reason}"
                    ))
                    .with_source_span(reference),
                ]
            };
            let machine = execution.machine(owner_symbols[*ordinal]).ok_or_else(|| {
                failure("application probe lost its exact private declaration".into())
            })?;
            // This pre-resolution probe has no Omega provider plan. Keep
            // authored applications whose reachable closure needs selected
            // execution intact: the typed continuation folds them under the
            // exact provider rows Omega settles, as deferred fixed-array
            // lengths do. This is explicit external readiness, not a failed
            // admission interpreted as a dependency signal.
            if admission.closure_needs_operator_selection(execution, machine.symbol, &operators) {
                continue;
            }
            let checked = match checked_program {
                Some(ref checked) => checked,
                None => checked_program.insert(
                    crate::const_evaluation::const_initializers::CheckedInitializers::prepare(
                        &independent_program,
                        selection_authority.clone(),
                        &independent,
                    )?,
                ),
            };
            let invocation = checked.calls_for_symbol(machine.symbol).map_err(&failure)?;
            let evaluated = invocation
                .evaluate_application_probe(
                    reference,
                    *public,
                    &syntax,
                    extent_uses.contains(argument),
                )
                .map_err(&failure)?;
            let (_, origins, calls) = lexical_selections
                .iter()
                .find(|(original, ..)| original == argument)
                .ok_or_else(|| {
                    failure("application lost its original lexical selections".into())
                })?;
            if calls.iter().any(|call| !evaluated.calls.contains(call))
                || origins
                    .iter()
                    .any(|origin| !evaluated.origins.contains(origin))
            {
                return Err(failure(
                    "application probe changed the original constant or call selection".to_owned(),
                ));
            }
            let folded = match evaluated.value.decode_encoding() {
                Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
                Some(DecodedCanonicalConstValue::Boolean(_)) => evaluated.value.atom(),
                _ => {
                    return Err(failure(
                        "application probe returned a non-scalar canonical value".to_owned(),
                    ));
                }
            };
            for warning in &evaluated.warnings {
                eprintln!("{warning}");
            }
            syntax.type_references.retain_const_argument_normalization(
                *argument,
                reference,
                evaluated.value.encoding.clone(),
                evaluated.origins,
                evaluated.operators,
            );
            syntax.type_references.replace_type_reference(
                *argument,
                TypeReferenceNode::Named(Identifier::generated(folded)),
            );
            progress = true;
        }
    }

    // Endpoints are input jobs too. Their private execution closure must not
    // consume any unsettled index; leave blocked leaves authored for the next
    // layer or the later selected-provider continuation.
    endpoint_leaves.retain(|(_, source)| {
        let domain = preparation.domain_for_inputs(&[], &[*source]);
        !preparation
            .pending_type_inputs
            .iter()
            .any(|(owner, _)| domain.contains(*owner))
    });
    let endpoint_sources = endpoint_leaves
        .iter()
        .map(|(_, source)| *source)
        .collect::<Vec<_>>();
    let evaluation_domain = preparation.domain_for_inputs(&[], &endpoint_sources);
    let mut typed = preparation.typed;
    if !endpoint_leaves.is_empty() {
        // The probe carries resolved endpoint calls. Fold them through the
        // shared typed evaluator so resolved machine identity, closed argument,
        // declared result-carrier, and signature-bound dependency checks match
        // post-typing admission exactly. Only leaves the gate actually closed
        // are written back; an endpoint it leaves authored keeps its original
        // expression and its ordinary post-typing route.
        crate::const_evaluation::range_endpoints::evaluate_selected_range_endpoints(
            &mut typed,
            selection_authority.clone(),
            crate::SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &[],
            },
            &evaluation_domain,
        )?;
        for (expression, source_span) in endpoint_leaves {
            let value = typed
                .expression_table
                .iter_expressions()
                .filter(|(handle, _)| typed.expression_table.source_span(*handle) == source_span)
                .find_map(|(_, node)| match node {
                    typed_trees::expression::ExpressionNode::Integer(literal) => {
                        literal.value_bignum()
                    }
                    _ => None,
                });
            let Some(value) = value else {
                continue;
            };
            let literal = IntegerLiteral::from_parts(
                value.is_negative(),
                IntegerRadix::Decimal,
                value.abs().to_string().as_str(),
            )
            .expect("an evaluated range endpoint is a valid integer literal");
            syntax
                .expressions
                .replace_expression(expression, ExpressionNode::Integer(literal));
            progress = true;
        }
    }
    Ok((syntax, progress))
}

/// Every authored call leaf inside declared range endpoints of generic
/// arguments. Synthesis needs those canonical intervals before resolution, so
/// each call's position is marked here; the typed probe fold supplies values.
fn collect_generic_range_endpoint_call_leaves(
    syntax: &SyntaxTrees,
    pending: &mut Vec<(ExpressionHandle, source::SourceSpan)>,
) {
    let mut visited = Vec::new();
    let mut worklist = syntax.type_references.generic_nodes();
    while let Some(reference) = worklist.pop() {
        if visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Generic { arguments, .. } => {
                worklist.extend(syntax.type_references.type_reference_handles(*arguments));
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                worklist.push(*base_type);
                for constraint in syntax.type_references.constraints(*constraints) {
                    let TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } = constraint
                    else {
                        continue;
                    };
                    collect_endpoint_call_leaves(syntax, *minimum, pending);
                    collect_endpoint_call_leaves(syntax, *maximum, pending);
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => worklist.push(*element_type),
            TypeReferenceNode::Reference { referee, .. } => worklist.push(*referee),
            _ => {}
        }
    }
}

/// Call leaves of one range endpoint, matching the shared typed fold's
/// traversal: binary operands and call arguments may each contain the next
/// authored invocation. Calls with runtime receivers or unresolved static
/// applications are still marked; the probe gate leaves them authored.
fn collect_endpoint_call_leaves(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    pending: &mut Vec<(ExpressionHandle, source::SourceSpan)>,
) {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            collect_endpoint_call_leaves(syntax, binary.left, pending);
            collect_endpoint_call_leaves(syntax, binary.right, pending);
        }
        ExpressionNode::Call(call) => {
            pending.push((expression, syntax.expressions.source_span(expression)));
            for argument in syntax.expressions.expression_handles(call.arguments) {
                collect_endpoint_call_leaves(syntax, *argument, pending);
            }
        }
        _ => {}
    }
}

/// Route the whole scalar expression through checked invocation admission.
/// Zero-argument calls are not eager leaves: an unselected branch must retain
/// its selection obligations without executing its helper.
pub(super) fn contains_call(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            contains_call(syntax, binary.left) || contains_call(syntax, binary.right)
        }
        ExpressionNode::Unary(unary) => contains_call(syntax, unary.operand),
        ExpressionNode::Cast(cast) => contains_call(syntax, cast.value),
        ExpressionNode::Match(dispatch) => {
            contains_call(syntax, dispatch.subject)
                || syntax
                    .expressions
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, syntax_trees::expression::MatchPattern::Value(pattern)
                        if contains_call(syntax, pattern))
                            || contains_call(syntax, arm.value)
                    })
        }
        ExpressionNode::Call(_) => true,
        _ => false,
    }
}
