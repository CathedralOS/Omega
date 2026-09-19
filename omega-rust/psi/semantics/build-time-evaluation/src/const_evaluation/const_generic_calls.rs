//! Psi pre-resolution bridge for machine calls in const-generic arguments
//! (`Buffer<table_size()>`). Generic record instances must be synthesized
//! before the ordinary frontend runs, while the established build-time
//! evaluator needs typed trees. Build a sanitized probe program, type it,
//! reuse the same normalized build-time gate and interpreter entry as
//! fixed-array lengths, then substitute canonical decimal leaves into the
//! authoritative syntax tree before monomorphization.
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
use syntax_trees::statement::{StatementNode, TableLocalData};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub fn evaluate_const_generic_calls(
    mut syntax: SyntaxTrees,
    context: crate::BuildTimeSources<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let crate::BuildTimeSources {
        sources,
        source_scoped_top_level_bindings,
        selection_authority,
    } = context;
    let mut pending = Vec::new();
    let mut pending_type_references = Vec::new();
    let mut applications = Vec::new();
    for (type_reference, expression) in syntax.type_references.const_expression_nodes() {
        let before = pending.len();
        if contains_application_call(&syntax, expression)? {
            applications.push((type_reference, expression));
            continue;
        }
        collect_call_leaves(&syntax, expression, &mut pending)?;
        if pending.len() > before {
            pending_type_references.push(type_reference);
        }
    }
    // Range endpoints nested in generic arguments also feed synthesis. Only
    // their call leaves are collected here; the probe's shared endpoint fold
    // decides which of them can actually close.
    let mut endpoint_leaves = Vec::new();
    collect_generic_range_endpoint_call_leaves(&syntax, &mut endpoint_leaves);
    if pending.is_empty() && endpoint_leaves.is_empty() && applications.is_empty() {
        return Ok(syntax);
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
        &pending_type_references
            .iter()
            .copied()
            .chain(
                application_destinations
                    .iter()
                    .map(|(argument, ..)| *argument),
            )
            .collect::<Vec<_>>(),
    );
    for type_reference in &pending_type_references {
        probe.type_references.replace_type_reference(
            *type_reference,
            TypeReferenceNode::Named(Identifier::generated("0")),
        );
    }
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
    let admission = crate::BuildTimeAdmissionPlan::infer(&typed, selection_authority.clone());

    for (expression, machine_name, source_span) in pending {
        let value = crate::evaluate_zero_argument_machine(
            &typed,
            &admission,
            &machine_name,
            "generic argument",
            Some(crate::BuildTimeInvocationCustody::Source(source_span)),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "const-generic evaluation of `{machine_name}()` failed: {reason}"
            ))]
        })?;
        let value = value.to_u64().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "const-generic evaluation of `{machine_name}()` returned {value}, but const data arguments must be non-negative"
            ))]
        })?;
        let literal =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, value.to_string().as_str())
                .expect("a decimal u64 const-machine result is a valid integer literal");
        syntax
            .expressions
            .replace_expression(expression, ExpressionNode::Integer(literal));
    }

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
            // Every authored call in the const expression must appear in the
            // probe's checked call closure; a dropped or substituted call
            // leaves the argument without selection custody.
            let mut authored = Vec::new();
            collect_authored_call_sites(&syntax, *expression, &mut authored);
            if authored.iter().any(|site| {
                !evaluated
                    .calls
                    .iter()
                    .any(|(call_site, _)| call_site == site)
            }) {
                return Err(failure(
                    "application probe dropped an authored call's selection custody".to_owned(),
                ));
            }
            let folded = match evaluated.value.decode_encoding() {
                Some(DecodedCanonicalConstValue::Integer { value, .. }) => ExpressionNode::Integer(
                    IntegerLiteral::from_parts(
                        value < 0,
                        IntegerRadix::Decimal,
                        value.unsigned_abs().to_string().as_str(),
                    )
                    .expect("a canonical integer const result is a valid integer literal"),
                ),
                Some(DecodedCanonicalConstValue::Boolean(value)) => ExpressionNode::Boolean(value),
                _ => {
                    return Err(failure(
                        "application probe returned a non-scalar canonical value".to_owned(),
                    ));
                }
            };
            for warning in &evaluated.warnings {
                eprintln!("{warning}");
            }
            syntax.expressions.replace_expression(*expression, folded);
            syntax.type_references.retain_const_argument_normalization(
                *argument,
                reference,
                evaluated.value.encoding.clone(),
                evaluated.origins,
                evaluated.operators,
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
/// only to evaluate the appended call probes: a `let` keeps its binding
/// without an initial value, and a state whose return position was
/// placeholdered runs body-free with the declaration dropped (a retained
/// `-> T` plus an empty body is itself a diagnostic). The authored syntax is
/// untouched; the real pipeline still checks every initializer against the
/// folded destination.
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
        let (return_type, statements) = {
            let state = probe.items.state(state_handle);
            (state.return_type, state.statements)
        };
        if type_reference_contains_any(probe, return_type, &arguments) {
            let state = probe.items.state_mut(state_handle);
            state.return_type = TypeReferenceHandle::invalid();
            state.statements = HandleSpan::empty();
            continue;
        }
        for statement_handle in probe.items.statements(statements).to_vec() {
            let StatementNode::LocalData(local) =
                probe.statements.statement(statement_handle).clone()
            else {
                continue;
            };
            if !type_reference_contains_any(probe, local.type_reference, &arguments) {
                continue;
            }
            probe.statements.replace_statement(
                statement_handle,
                StatementNode::LocalData(TableLocalData {
                    initial_value: ExpressionHandle::invalid(),
                    ..local
                }),
            );
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

/// Whether a const expression contains a value-argument call; such an
/// expression is evaluated whole by the application probe instead of by
/// leaf substitution. Calls carrying machine or evidence arguments need the
/// complete specialized application context and remain rejected here rather
/// than admitted early.
fn contains_application_call(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<bool, Vec<Diagnostic>> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let left = contains_application_call(syntax, binary.left)?;
            let right = contains_application_call(syntax, binary.right)?;
            Ok(left || right)
        }
        ExpressionNode::Call(call) => {
            if !call.machine_arguments.is_empty() || !call.evidence_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "const-generic call `{}` must take no machine arguments",
                    call.target.as_str()
                ))]);
            }
            Ok(!call.arguments.is_empty())
        }
        _ => Ok(false),
    }
}

/// The authored call sites inside one const expression, walking the same
/// shapes the application probe admits so custody comparison sees the same
/// call set (a dropped or substituted call leaves the argument without
/// selection custody).
fn collect_authored_call_sites(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    sites: &mut Vec<source::SourceSpan>,
) {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            collect_authored_call_sites(syntax, binary.left, sites);
            collect_authored_call_sites(syntax, binary.right, sites);
        }
        ExpressionNode::Call(call) => {
            sites.push(syntax.expressions.source_span(expression));
            for argument in syntax.expressions.expression_handles(call.arguments) {
                collect_authored_call_sites(syntax, *argument, sites);
            }
        }
        _ => {}
    }
}

fn collect_call_leaves(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    pending: &mut Vec<(ExpressionHandle, String, source::SourceSpan)>,
) -> Result<(), Vec<Diagnostic>> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            collect_call_leaves(syntax, binary.left, pending)?;
            collect_call_leaves(syntax, binary.right, pending)
        }
        ExpressionNode::Call(call) => {
            if !call.arguments.is_empty() || !call.machine_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "const-generic call `{}` must take no value or machine arguments",
                    call.target.as_str()
                ))]);
            }
            let machine_name = call_machine_name(syntax, call)?;
            pending.push((
                expression,
                machine_name,
                syntax.expressions.source_span(expression),
            ));
            Ok(())
        }
        _ => Ok(()),
    }
}

fn call_machine_name(
    syntax: &SyntaxTrees,
    call: &syntax_trees::expression::TableCallExpression,
) -> Result<String, Vec<Diagnostic>> {
    if !call.receiver.is_valid() {
        return Ok(call.target.as_str().to_string());
    }
    let ExpressionNode::Name(path) = syntax.expressions.expression(call.receiver) else {
        return Err(vec![Diagnostic::error(
            "a const-generic machine call must use a free or type-scoped machine path",
        )]);
    };
    let mut name = syntax
        .expressions
        .identifier_path_members(*path)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    if !name.is_empty() {
        name.push_str("::");
    }
    name.push_str(call.target.as_str());
    Ok(name)
}
