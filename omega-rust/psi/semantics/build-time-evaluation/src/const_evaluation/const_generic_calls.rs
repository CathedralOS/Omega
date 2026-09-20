//! Psi pre-resolution bridge for machine calls in const-generic arguments
//! (`Buffer<table_size()>`). Generic record instances must be synthesized
//! before the ordinary frontend runs, while the established build-time
//! evaluator needs typed trees. Build a sanitized probe program, type it,
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

use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::Item;
use syntax_trees::statement::StatementNode;
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub fn evaluate_const_generic_calls(
    mut syntax: SyntaxTrees,
    context: crate::BuildTimeSources<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    loop {
        // Usually all arguments share one frontend preparation. If a helper's
        // own index is still pending, its body cannot execute on provisional
        // types. Complete an independently admitted argument, then reprepare
        // the original body with that real value. Every successful iteration
        // removes an authored argument; no-progress returns the actual failure.
        // This reuses the admission gate rather than fabricating helper-local
        // defaults or maintaining a second syntax-level call dependency graph.
        let failures = match evaluate_application_group(
            syntax.clone(),
            context.clone(),
            TypeReferenceHandle::invalid(),
        ) {
            Ok(evaluated) => return Ok(evaluated),
            Err(failures) => failures,
        };
        let pending = syntax
            .type_references
            .const_expression_nodes()
            .into_iter()
            .filter(|(_, expression)| contains_call(&syntax, *expression))
            .map(|(argument, _)| argument)
            .collect::<Vec<_>>();
        if pending.len() < 2 {
            return Err(failures);
        }
        let mut progress = None;
        for argument in pending {
            if let Ok(evaluated) =
                evaluate_application_group(syntax.clone(), context.clone(), argument)
            {
                progress = Some(evaluated);
                break;
            }
        }
        let Some(evaluated) = progress else {
            return Err(failures);
        };
        syntax = evaluated;
    }
}

fn evaluate_application_group(
    mut syntax: SyntaxTrees,
    context: crate::BuildTimeSources<'_>,
    selected_argument: TypeReferenceHandle,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let crate::BuildTimeSources {
        sources,
        source_scoped_top_level_bindings,
        selection_authority,
    } = context;
    let mut applications = Vec::new();
    for (type_reference, expression) in syntax.type_references.const_expression_nodes() {
        if contains_call(&syntax, expression) {
            applications.push((type_reference, expression));
        }
    }
    // Range endpoints nested in generic arguments also feed synthesis. Only
    // their call leaves are collected here; the probe's shared endpoint fold
    // decides which of them can actually close.
    let mut endpoint_leaves = Vec::new();
    collect_generic_range_endpoint_call_leaves(&syntax, &mut endpoint_leaves);
    if endpoint_leaves.is_empty() && applications.is_empty() {
        return Ok(syntax);
    }

    // A standalone probe cannot supply the caller's lexical bindings. Resolve
    // those before provisional types remove its body, then rejoin exact
    // constant and call declarations after typed invocation preparation.
    let mut lexical_selections = Vec::new();
    if !applications.is_empty() {
        let resolved = syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_const_argument_selection(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
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
        let mut const_arguments =
            syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_data_const_argument_expressions(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax: &syntax,
                    sources: sources.clone(),
                    top_level_bindings: source_scoped_top_level_bindings.to_vec(),
                },
            )?;
        const_arguments.extend(
            syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_machine_const_arguments(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax: &syntax,
                    sources: sources.clone(),
                    top_level_bindings: source_scoped_top_level_bindings.to_vec(),
                },
            )?,
        );
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

    // The probe needs the same generic-template normalization as the real
    // program, but the call results are not known yet. A temporary zero leaf
    // for each owning const argument lets the frontend type/effect-check the
    // machine definitions themselves; no probe layout escapes this function.
    let mut probe = syntax.clone();
    crate::machine_execution::syntax_probes::defer_pending_const_qualifications(
        &mut probe,
        &application_destinations
            .iter()
            .map(|(argument, ..)| *argument)
            .collect::<Vec<_>>(),
    );
    // Application arguments get the same placeholder the scalar probes use:
    // `false` on a bool carrier, `0` on every integer carrier. The authored
    // expression survives whole inside the appended probe machine.
    for (type_reference, _, destination, _) in &application_destinations {
        let placeholder = match syntax.type_references.type_reference(*destination) {
            TypeReferenceNode::Named(name) if name.as_str() == "bool" => "false",
            _ => "0",
        };
        probe.type_references.replace_type_reference(
            *type_reference,
            TypeReferenceNode::Named(Identifier::generated(placeholder)),
        );
    }
    detach_probe_value_producers(&mut probe, &application_destinations);
    application_destinations
        .retain(|(argument, ..)| !selected_argument.is_valid() || selected_argument == *argument);
    for (ordinal, (_, expression, destination, _)) in application_destinations.iter().enumerate() {
        crate::const_evaluation::const_generic_expressions::append_probe(
            &mut probe,
            ordinal,
            *expression,
            *destination,
        );
    }
    let probe = crate::machine_execution::syntax_probes::normalize_generic_data(
        probe,
        sources.clone(),
        source_scoped_top_level_bindings,
        None,
    )?;
    let resolved = crate::machine_execution::syntax_probes::resolve(
        &probe,
        sources,
        source_scoped_top_level_bindings,
    )?;
    let mut typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    // This pre-resolution probe has no Omega provider plan. Keep authored
    // operator uses intact: visible satisfiers cannot authorize execution.

    if !application_destinations.is_empty() {
        // The appended probe machines are excluded from ordinary checking;
        // their expressions are evaluated by the exact scalar evaluator with
        // checked call admission, matching const-initializer invocations.
        let references = application_destinations
            .iter()
            .map(|(_, expression, ..)| syntax.expressions.source_span(*expression))
            .collect::<Vec<_>>();
        let probe_symbols = typed
            .machines()
            .iter()
            .filter(|machine| {
                typed
                    .symbols
                    .symbol_source_span(machine.symbol)
                    .is_some_and(|span| references.contains(&span))
            })
            .map(|machine| machine.symbol)
            .collect::<Vec<_>>();
        let checked = crate::const_evaluation::const_initializers::CheckedInitializers::prepare(
            &typed,
            selection_authority.clone(),
            &probe_symbols,
        )?;
        for (argument, expression, _, public) in &application_destinations {
            let reference = syntax.expressions.source_span(*expression);
            let failure = |reason: String| {
                vec![
                    Diagnostic::error(format!(
                        "const-generic application evaluation failed: {reason}"
                    ))
                    .with_source_span(reference),
                ]
            };
            let invocation = checked.calls_for_source(reference).map_err(&failure)?;
            let evaluated = invocation
                .evaluate_application_probe(reference, *public, &syntax)
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
        }
    }

    if !endpoint_leaves.is_empty() {
        // The probe carries resolved endpoint calls. Fold them through the
        // shared typed evaluator so resolved machine identity, closed argument,
        // declared result-carrier, and signature-bound dependency checks match
        // post-typing admission exactly. Only leaves the gate actually closed
        // are written back; an endpoint it leaves authored keeps its original
        // expression and its ordinary post-typing route.
        crate::evaluate_const_range_endpoints(&mut typed, selection_authority.clone())?;
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
        }
    }
    Ok(syntax)
}

/// Detach the probe's value producers that sit next to a placeholdered
/// declared type. `let folded: Buffer<size(4)> = Buffer { values: [..] }`
/// becomes `Buffer<0>` while its initializer keeps the authored element
/// count, so the probe's ordinary construction checks would compare shapes
/// the placeholder invented rather than the program's own. The probe exists
/// only to evaluate the appended call probes: a state containing a provisional
/// parameter, local or return type runs body-free with the return declaration dropped (a retained
/// `-> T` plus an empty body is itself a diagnostic). The authored syntax is
/// untouched; the real pipeline still checks every initializer and consumer
/// against the folded destination. Removing only the local initializer would
/// let provisional identity reach its later consumers during helper checking.
/// Such an owner cannot itself supply a scalar helper result in this probe;
/// its real body needs the completed index before invocation admission.
fn detach_probe_value_producers(
    probe: &mut SyntaxTrees,
    application_destinations: &[(
        TypeReferenceHandle,
        ExpressionHandle,
        TypeReferenceHandle,
        bool,
    )],
) {
    let arguments: Vec<TypeReferenceHandle> = application_destinations
        .iter()
        .map(|(argument, ..)| *argument)
        .collect();
    if arguments.is_empty() {
        return;
    }
    let states: Vec<_> = probe
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) => Some(machine.states),
            _ => None,
        })
        .flat_map(|states| probe.items.state_handles(states).to_vec())
        .collect();
    for state_handle in states {
        let (parameters, return_type, statements) = {
            let state = probe.items.state(state_handle);
            (state.parameters, state.return_type, state.statements)
        };
        let has_pending_parameter =
            probe
                .items
                .state_parameters(parameters)
                .iter()
                .any(|parameter| {
                    type_reference_contains_any(
                        probe,
                        probe.items.state_parameter(*parameter).type_reference,
                        &arguments,
                    )
                });
        let has_pending_local = probe.items.statements(statements).iter().any(|statement| {
            let StatementNode::LocalData(local) = probe.statements.statement(*statement) else {
                return false;
            };
            type_reference_contains_any(probe, local.type_reference, &arguments)
        });
        if has_pending_parameter
            || has_pending_local
            || type_reference_contains_any(probe, return_type, &arguments)
        {
            let state = probe.items.state_mut(state_handle);
            state.return_type = TypeReferenceHandle::invalid();
            state.statements = HandleSpan::empty();
        }
    }
}

/// Whether a type reference's subtree contains one of the given const
/// argument nodes, walking the same shapes position collection walks.
fn type_reference_contains_any(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    arguments: &[TypeReferenceHandle],
) -> bool {
    if arguments.contains(&type_reference) {
        return true;
    }
    match syntax.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_any(syntax, *referee, arguments)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            if type_reference_contains_any(syntax, *base_type, arguments) {
                return true;
            }
            syntax
                .type_references
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return false;
                    };
                    syntax
                        .type_references
                        .type_reference_handles(domain.arguments)
                        .iter()
                        .any(|argument| type_reference_contains_any(syntax, *argument, arguments))
                })
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_reference_contains_any(syntax, *element_type, arguments)
        }
        TypeReferenceNode::Generic {
            arguments: generic_arguments,
            ..
        } => syntax
            .type_references
            .type_reference_handles(*generic_arguments)
            .iter()
            .any(|argument| type_reference_contains_any(syntax, *argument, arguments)),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => false,
    }
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
