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
            // This pre-resolution probe has no Omega provider plan. Keep
            // authored applications whose reachable closure needs selected
            // execution intact: the typed continuation folds them under the
            // exact provider rows Omega settles, as deferred fixed-array
            // lengths do. The per-argument retry mode must report no
            // progress for such an application or the driver loop never
            // terminates.
            if invocation.needs_operator_selection() {
                if selected_argument.is_valid() {
                    return Err(failure(
                        "const-generic application awaits provider selection".to_owned(),
                    ));
                }
                continue;
            }
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

    // Pending arguments also hide behind member access: a field declared with
    // a pending argument makes its whole data carrier pending, and a field
    // typed by a pending carrier reaches the same provisional leaf through
    // `outer.inner`. Fold carrier names transitively so a `self.value` read
    // detaches the same way a pending parameter, local, or return does.
    let mut pending_data: Vec<String> = Vec::new();
    loop {
        let mut discovered = false;
        for item in probe.root_items() {
            let Item::Data(definition) = item else {
                continue;
            };
            if pending_data
                .iter()
                .any(|name| name == definition.name.as_str())
            {
                continue;
            }
            let is_pending = probe
                .items
                .data_members(definition.members)
                .iter()
                .any(|member| {
                    let syntax_trees::item::DataMember::Field(field) = member else {
                        return false;
                    };
                    type_reference_is_pending(
                        probe,
                        field.type_reference,
                        &arguments,
                        &pending_data,
                    )
                });
            if is_pending {
                pending_data.push(definition.name.as_str().to_owned());
                discovered = true;
            }
        }
        if !discovered {
            break;
        }
    }

    // A state can reach a pending-typed value without declaring one in its own
    // signature: through a field of the data its machine attaches to, or
    // through a call to a machine whose signature still carries a pending
    // argument. A detached machine keeps its name but loses its body, so
    // callers observing its result see probe artifacts rather than authored
    // semantics; detach those callers too and iterate to a fixpoint.
    let mut pending_machines: Vec<String> = Vec::new();
    let mut detached_states: Vec<syntax_trees::item::StateHandle> = Vec::new();
    loop {
        let mut changed = false;
        let machines: Vec<(
            Identifier,
            Option<Identifier>,
            Vec<syntax_trees::item::StateHandle>,
        )> = probe
            .root_items()
            .filter_map(|item| match item {
                Item::Machine(machine) => Some((
                    machine.name.clone(),
                    machine.attached_data.clone(),
                    probe.items.state_handles(machine.states).to_vec(),
                )),
                _ => None,
            })
            .collect();
        for (name, attached_data, state_handles) in machines {
            if pending_machines
                .iter()
                .any(|pending| pending == name.as_str())
            {
                continue;
            }
            let owner_pending = attached_data
                .map(|owner| pending_data.iter().any(|pending| pending == owner.as_str()))
                .unwrap_or(false);
            for state_handle in state_handles.iter().copied() {
                if detached_states.contains(&state_handle) {
                    continue;
                }
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
                            type_reference_is_pending(
                                probe,
                                probe.items.state_parameter(*parameter).type_reference,
                                &arguments,
                                &pending_data,
                            )
                        });
                let has_pending_local =
                    probe.items.statements(statements).iter().any(|statement| {
                        let StatementNode::LocalData(local) =
                            probe.statements.statement(*statement)
                        else {
                            return false;
                        };
                        type_reference_is_pending(
                            probe,
                            local.type_reference,
                            &arguments,
                            &pending_data,
                        )
                    });
                let reaches_pending_machine =
                    probe.items.statements(statements).iter().any(|statement| {
                        statement_reaches_pending_machine(
                            probe,
                            *statement,
                            &arguments,
                            &pending_data,
                            &pending_machines,
                        )
                    });
                if owner_pending
                    || has_pending_parameter
                    || has_pending_local
                    || type_reference_is_pending(probe, return_type, &arguments, &pending_data)
                    || reaches_pending_machine
                {
                    let state = probe.items.state_mut(state_handle);
                    state.return_type = TypeReferenceHandle::invalid();
                    state.statements = HandleSpan::empty();
                    detached_states.push(state_handle);
                    changed = true;
                }
            }
            if !state_handles.is_empty()
                && state_handles
                    .iter()
                    .all(|handle| detached_states.contains(handle))
            {
                pending_machines.push(name.as_str().to_owned());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Whether a type reference's subtree contains one of the given const
/// argument nodes or names a data definition carrying one, walking the same
/// shapes position collection walks.
fn type_reference_is_pending(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    arguments: &[TypeReferenceHandle],
    pending_data: &[String],
) -> bool {
    if arguments.contains(&type_reference) {
        return true;
    }
    match syntax.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => {
            pending_data.iter().any(|pending| pending == name.as_str())
        }
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_is_pending(syntax, *referee, arguments, pending_data)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            if type_reference_is_pending(syntax, *base_type, arguments, pending_data) {
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
                        .any(|argument| {
                            type_reference_is_pending(syntax, *argument, arguments, pending_data)
                        })
                })
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_reference_is_pending(syntax, *element_type, arguments, pending_data)
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments: generic_arguments,
            ..
        } => {
            pending_data
                .iter()
                .any(|pending| pending == base_name.as_str())
                || syntax
                    .type_references
                    .type_reference_handles(*generic_arguments)
                    .iter()
                    .any(|argument| {
                        type_reference_is_pending(syntax, *argument, arguments, pending_data)
                    })
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => false,
    }
}

/// Whether a statement observes a pending-typed value: a call into a machine
/// whose body the probe removed, a pending-typed static operand, or any nested
/// expression doing either.
fn statement_reaches_pending_machine(
    syntax: &SyntaxTrees,
    statement: syntax_trees::statement::StatementHandle,
    arguments: &[TypeReferenceHandle],
    pending_data: &[String],
    pending_machines: &[String],
) -> bool {
    let expression_reaches = |expression: ExpressionHandle| {
        expression_reaches_pending_machine(
            syntax,
            expression,
            arguments,
            pending_data,
            pending_machines,
        )
    };
    match syntax.statements.statement(statement) {
        StatementNode::Assignment(assignment) => {
            expression_reaches(assignment.target) || expression_reaches(assignment.value)
        }
        StatementNode::Call(call) => {
            pending_machines
                .iter()
                .any(|pending| pending == call.target.as_str())
                || call.machine_arguments.iter().any(|argument| {
                    static_machine_argument_is_pending(syntax, argument, arguments, pending_data)
                })
                || syntax
                    .expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_reaches(*argument))
        }
        StatementNode::ProofOutputBindingStatement(proof_output) => {
            expression_reaches(proof_output.call)
        }
        StatementNode::AssemblyFact(fact) => expression_reaches(fact.expression),
        StatementNode::Expression(expression) => expression_reaches(*expression),
        StatementNode::LocalData(local) => expression_reaches(local.initial_value),
        StatementNode::RootBinding(binding) => {
            expression_reaches(binding.receiver)
                || (binding.implementation_operand.is_valid()
                    && expression_reaches(binding.implementation_operand))
        }
        StatementNode::Transition(_) => false,
    }
}

/// Whether an expression subtree invokes a detached machine or names a
/// pending-typed static operand, walking every value-bearing variant.
fn expression_reaches_pending_machine(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    arguments: &[TypeReferenceHandle],
    pending_data: &[String],
    pending_machines: &[String],
) -> bool {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_reaches_pending_machine(
                syntax,
                atomic.value,
                arguments,
                pending_data,
                pending_machines,
            ) || (atomic.result.is_valid()
                && expression_reaches_pending_machine(
                    syntax,
                    atomic.result,
                    arguments,
                    pending_data,
                    pending_machines,
                ))
        }
        ExpressionNode::ArrayLiteral(elements) => syntax
            .expressions
            .expression_handles(*elements)
            .iter()
            .any(|element| {
                expression_reaches_pending_machine(
                    syntax,
                    *element,
                    arguments,
                    pending_data,
                    pending_machines,
                )
            }),
        ExpressionNode::Binary(binary) => {
            expression_reaches_pending_machine(
                syntax,
                binary.left,
                arguments,
                pending_data,
                pending_machines,
            ) || expression_reaches_pending_machine(
                syntax,
                binary.right,
                arguments,
                pending_data,
                pending_machines,
            )
        }
        ExpressionNode::Borrow(borrow) => expression_reaches_pending_machine(
            syntax,
            borrow.target,
            arguments,
            pending_data,
            pending_machines,
        ),
        ExpressionNode::Call(call) => {
            pending_machines
                .iter()
                .any(|pending| pending == call.target.as_str())
                || call.machine_arguments.iter().any(|argument| {
                    static_machine_argument_is_pending(syntax, argument, arguments, pending_data)
                })
                || (call.receiver.is_valid()
                    && expression_reaches_pending_machine(
                        syntax,
                        call.receiver,
                        arguments,
                        pending_data,
                        pending_machines,
                    ))
                || syntax
                    .expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        expression_reaches_pending_machine(
                            syntax,
                            *argument,
                            arguments,
                            pending_data,
                            pending_machines,
                        )
                    })
        }
        ExpressionNode::Cast(cast) => {
            type_reference_is_pending(syntax, cast.target_type, arguments, pending_data)
                || expression_reaches_pending_machine(
                    syntax,
                    cast.value,
                    arguments,
                    pending_data,
                    pending_machines,
                )
        }
        ExpressionNode::Indexed(indexed) => {
            expression_reaches_pending_machine(
                syntax,
                indexed.collection,
                arguments,
                pending_data,
                pending_machines,
            ) || expression_reaches_pending_machine(
                syntax,
                indexed.index,
                arguments,
                pending_data,
                pending_machines,
            )
        }
        ExpressionNode::Match(dispatch) => {
            expression_reaches_pending_machine(
                syntax,
                dispatch.subject,
                arguments,
                pending_data,
                pending_machines,
            ) || syntax
                .expressions
                .match_arms(dispatch.arms)
                .iter()
                .any(|arm| {
                    (matches!(arm.pattern, syntax_trees::expression::MatchPattern::Value(pattern)
                    if expression_reaches_pending_machine(
                        syntax,
                        pattern,
                        arguments,
                        pending_data,
                        pending_machines,
                    ))) || expression_reaches_pending_machine(
                        syntax,
                        arm.value,
                        arguments,
                        pending_data,
                        pending_machines,
                    )
                })
        }
        ExpressionNode::Member(member) => expression_reaches_pending_machine(
            syntax,
            member.receiver,
            arguments,
            pending_data,
            pending_machines,
        ),
        ExpressionNode::Membership(membership) => expression_reaches_pending_machine(
            syntax,
            membership.value,
            arguments,
            pending_data,
            pending_machines,
        ),
        ExpressionNode::Range(range) => {
            expression_reaches_pending_machine(
                syntax,
                range.start,
                arguments,
                pending_data,
                pending_machines,
            ) || expression_reaches_pending_machine(
                syntax,
                range.end,
                arguments,
                pending_data,
                pending_machines,
            )
        }
        ExpressionNode::StructLiteral(literal) => {
            pending_data
                .iter()
                .any(|pending| pending == literal.constructor_name.as_str())
                || syntax
                    .expressions
                    .struct_fields(literal.fields)
                    .iter()
                    .any(|field| {
                        expression_reaches_pending_machine(
                            syntax,
                            field.value,
                            arguments,
                            pending_data,
                            pending_machines,
                        )
                    })
        }
        ExpressionNode::TypeExpression(type_reference)
        | ExpressionNode::ZeroValue(type_reference) => {
            type_reference_is_pending(syntax, *type_reference, arguments, pending_data)
        }
        ExpressionNode::Unary(unary) => expression_reaches_pending_machine(
            syntax,
            unary.operand,
            arguments,
            pending_data,
            pending_machines,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::SelfValue
        | ExpressionNode::String(_) => false,
    }
}

/// Whether a static call argument still carries a pending const argument or a
/// pending data name, including nested symbol applications.
fn static_machine_argument_is_pending(
    syntax: &SyntaxTrees,
    argument: &syntax_trees::expression::StaticMachineArgument,
    arguments: &[TypeReferenceHandle],
    pending_data: &[String],
) -> bool {
    if argument.type_reference.is_valid()
        && type_reference_is_pending(syntax, argument.type_reference, arguments, pending_data)
    {
        return true;
    }
    argument
        .application
        .as_ref()
        .map(|application| {
            application.arguments.iter().any(|nested| {
                static_machine_argument_is_pending(syntax, nested, arguments, pending_data)
            })
        })
        .unwrap_or(false)
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
