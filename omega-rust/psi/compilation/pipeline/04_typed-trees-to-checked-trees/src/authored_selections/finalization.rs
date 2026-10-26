//! Finalizing checked authored selections.
//!
//! Every authored selection resolves to its checked target here, except a
//! late-bound selection in compiler toolchain source, a target sibling's body
//! or a build descriptor: those stay late-bound for the realization or build
//! evaluation that binds them. Both package checkpoints apply this one rule.

use crate::authored_selections::call_targets::{
    checked_call_target, checked_name_path_segment_target, checked_operator_conformance_targets,
    expression_receiver_symbol, implementation_call_selection,
};
use crate::authored_selections::member_targets::checked_member_target;
use crate::authored_selections::operator_targets::{
    GenericOperatorValueOrigins, checked_generic_operator_target,
    checked_operator_target_for_occurrence, checked_structural_equality_call,
    typed_operator_has_no_authored_selection,
};
use crate::authored_selections::selection_collection::{
    checked_struct_literal_type_symbol, collect_checked_proof_membership_selections,
    collect_checked_proof_view_call_selections, collect_checked_statement_selections,
};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionIntrinsic,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
    AuthoredDeclarationSelections, BuildOperation,
};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::ExpressionNode;
use validation::selection_resolution::{
    AuthoredSelectionResolution, AuthoredSelectionResolutionTarget, declaration_target,
    finalization_diagnostic, push_consistent_resolution,
};

pub(crate) fn finalize_checked_authored_selections(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<AuthoredDeclarationSelections, Diagnostic> {
    let mut resolutions = Vec::new();
    let mut inferred_conformances = Vec::new();
    let expressions = &program.tables.expression_table;
    // Built at the first open-generic operator occurrence and shared by every
    // later one: this loop reads the expression table and the checked facts
    // and writes neither, so one index answers them all.
    let mut generic_operator_values: Option<GenericOperatorValueOrigins> = None;
    // One exact-owner index answers every member expression's query in a
    // single pass instead of a whole-program rescan per occurrence.
    let owner_index = {
        let _s = phase_profile::guard("05.d.11_13.a.1.owner_index");
        validation::expression_owners::OwnerEnvironmentIndex::with_call_signatures(
            program,
            &facts.proof.contract_facts,
            &facts.proof.call_contract_signatures,
        )
    };

    {
        let _s = phase_profile::guard("05.d.11_13.a.2.root_bindings");
        super::build_selections::validate(program, &owner_index)?;
    }

    let product_operands =
        validation::execution_projection::product_request_operand_occurrences(program);
    {
        let _s = phase_profile::guard("05.d.11_13.a.3.expression_loop");
        for (expression, node) in expressions.iter_expressions() {
            let occurrences = expressions
                .authored_selection_occurrences(expression)
                .collect::<Vec<_>>();
            for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    return Err(Diagnostic::error(format!(
                        "expression retains unknown authored declaration selection occurrence {}",
                        occurrence.ordinal()
                    )));
                };

                let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target()
                else {
                    continue;
                };

                if product_operands.contains(&occurrence) {
                    continue;
                }
                if binding == AuthoredDeclarationSelectionLateBinding::CheckedOperator {
                    for selected_symbol in checked_operator_conformance_targets(facts, expression) {
                        let inferred = (
                            selection.source_span(),
                            selection.exposure(),
                            selected_symbol,
                        );
                        if !inferred_conformances.contains(&inferred) {
                            inferred_conformances.push(inferred);
                        }
                    }
                }
                let target = match (binding, node) {
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStaticArgument,
                    ExpressionNode::Call(call),
                ) if BuildOperation::from_call_target(call.target.as_str())
                    .is_some_and(BuildOperation::selects_product_declarations) =>
                {
                    let Some(argument_index) = selection.top_level_static_argument_position() else {
                        continue;
                    };
                    let argument = call.machine_arguments.get(argument_index);
                    if validation::compiler_operations::is_build_declaration_selection(program, expression) {
                        let selected = validation::compiler_operations::resolve_build_static_operand(
                            program, expression, argument_index, selection.source_span(),
                        ).map_err(|message| Diagnostic::error(message)
                            .with_source_span(selection.source_span()))?;
                        Some(selected)
                    } else {
                        declaration_target(argument.map(|argument| argument.symbol).unwrap_or_default())
                    }
                }
                // A sealed quotient request selects compiler vocabulary, not a
                // declaration: it has no target symbol and produces no checked
                // call fact (the checked value paths skip it), so it resolves
                // as a proof-only intrinsic. Its representative and theorem
                // operands are separate static-argument selections.
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) if call.quotient_operation.is_some() => {
                    let request = call
                        .quotient_operation
                        .as_ref()
                        .expect("guarded sealed quotient request");
                    if call.target_symbol.is_valid() {
                        return Err(Diagnostic::error(
                            "a sealed quotient request cannot also select an authored declaration",
                        )
                        .with_source_span(selection.source_span()));
                    }
                    Some(AuthoredSelectionResolutionTarget::Intrinsic(match request.kind {
                        typed_trees::expression::QuotientOperationKind::Define => {
                            AuthoredDeclarationSelectionIntrinsic::QuotientDefine
                        }
                        typed_trees::expression::QuotientOperationKind::Lift => {
                            AuthoredDeclarationSelectionIntrinsic::QuotientLift
                        }
                    }))
                }
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => {
                    let _s = phase_profile::guard("05.d.11_13.a.3.call");
                    program.expression_table.build_selection_request(call.build_selection)
                    .and_then(|request| match request {
                        language_semantics::declaration_selection::BuildSelectionRequest::Declaration(request) => Some(request.operation.intrinsic()),
                        _ => None,
                    })
                    .or_else(|| validation::compiler_operations::classify_expression_call_intrinsic(program, call))
                    .map(AuthoredSelectionResolutionTarget::Intrinsic)
                    .or_else(|| {
                        declaration_target(
                            implementation_call_selection(
                                program,
                                expression_receiver_symbol(program, call.receiver),
                            )
                            .unwrap_or_else(|| {
                                checked_call_target(
                                    program,
                                    facts,
                                    expression,
                                    call.target_symbol,
                                    selection.source_span(),
                                )
                            }),
                        )
                    })
                }
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => {
                    let _s = phase_profile::guard("05.d.11_13.a.3.member");
                    checked_member_target(program, facts, expression, member, Some(&owner_index))
                }
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStaticPathSegment,
                    ExpressionNode::Name(path),
                ) => {
                    let _s = phase_profile::guard("05.d.11_13.a.3.name_path");
                    declaration_target(checked_name_path_segment_target(
                        program,
                        expression,
                        path,
                        late_binding_ordinal(program, &occurrences[..occurrence_offset], binding),
                    ))
                }
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralType,
                    ExpressionNode::StructLiteral(literal),
                ) => declaration_target(checked_struct_literal_type_symbol(
                    program,
                    literal,
                    selection.source_span(),
                )),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralCase,
                    ExpressionNode::StructLiteral(literal),
                ) => declaration_target(literal.case_symbol.unwrap_or_else(SymbolHandle::invalid)),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralField,
                    ExpressionNode::StructLiteral(literal),
                ) => declaration_target(
                    expressions
                        .struct_fields(literal.fields)
                        .get(late_binding_ordinal(
                            program,
                            &occurrences[..occurrence_offset],
                            binding,
                        ))
                        .map(|field| {
                            if field.field_symbol.is_valid() {
                                field.field_symbol
                            } else {
                                validation::resolve_member_symbol_from_type_symbol(
                                    program,
                                    checked_struct_literal_type_symbol(
                                        program,
                                        literal,
                                        selection.source_span(),
                                    ),
                                    field.name.as_str(),
                                )
                                .unwrap_or_else(SymbolHandle::invalid)
                            }
                        })
                        .unwrap_or_else(SymbolHandle::invalid),
                ),
                (AuthoredDeclarationSelectionLateBinding::CheckedOperator, _)
                    if matches!(
                        node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Unary(_)
                    ) =>
                {
                    let _s = phase_profile::guard("05.d.11_13.a.3.operator");
                    checked_generic_operator_target(
                        program,
                        facts,
                        generic_operator_values.get_or_insert_with(|| {
                            GenericOperatorValueOrigins::index(program, facts)
                        }),
                        expression,
                    )?
                    .or_else(|| checked_operator_target_for_occurrence(
                        program,
                        facts,
                        expression,
                        node,
                        occurrence,
                        Some(&owner_index),
                    ))
                    .or_else(|| {
                        typed_operator_has_no_authored_selection(program, expression).then_some(
                            AuthoredSelectionResolutionTarget::Intrinsic(
                                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                            ),
                        )
                    })
                }
                // Primitive constant folding can replace a successfully checked
                // builtin operator with its literal result while retaining the
                // source operator's custody occurrence on that result.
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                    ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::Integer(_),
                ) => Some(AuthoredSelectionResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                )),
                // A spelled use rewritten to a compiler-synthesized call keeps
                // its authored operator occurrence: `==` behind a written
                // `equals` stays the builtin operator custody, and a use bound
                // to a token-bearing machine's own body settles to that exact
                // declaration.
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                    ExpressionNode::Call(call),
                ) if call.operational_acknowledgement.origin
                    == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized =>
                {
                    checked_structural_equality_call(program, facts, expression, call)
                        .then_some(AuthoredSelectionResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        ))
                        .or_else(|| {
                            declaration_target(
                                validation::token_bound_machine_call_target(program, call)
                                    .unwrap_or_else(SymbolHandle::invalid),
                            )
                        })
                }
                _ => None,
            };
                if let Some(target) = target {
                    push_consistent_resolution(
                        &mut resolutions,
                        AuthoredSelectionResolution {
                            occurrence,
                            binding,
                            target,
                        },
                    )?;
                }
            }
        }
    }

    {
        let _s = phase_profile::guard("05.d.11_13.a.4.statement_selections");
        collect_checked_statement_selections(program, facts, &mut resolutions)?;
    }
    {
        let _s = phase_profile::guard("05.d.11_13.a.5.membership_selections");
        collect_checked_proof_membership_selections(program, facts, &mut resolutions)?;
    }
    let mut unoccurred_view_calls = Vec::new();
    {
        let _s = phase_profile::guard("05.d.11_13.a.6.proof_view_calls");
        collect_checked_proof_view_call_selections(
            program,
            &mut resolutions,
            &mut unoccurred_view_calls,
        )?;
    }

    let _s_apply = phase_profile::guard("05.d.11_13.a.7.apply_resolutions");
    let mut selections = program.authored_declaration_selections().clone();
    for resolution in resolutions {
        let result =
            match resolution.target {
                AuthoredSelectionResolutionTarget::Declaration(selected) => selections
                    .finalize_late_bound(resolution.occurrence, resolution.binding, selected),
                AuthoredSelectionResolutionTarget::Intrinsic(intrinsic) => selections
                    .finalize_intrinsic(resolution.occurrence, resolution.binding, intrinsic),
            };
        result.map_err(|error| finalization_diagnostic(resolution, error))?;
    }
    // A contract clause may spell a bare uninterpreted view atom
    // (`Bag(items)`); the resolver deliberately records no Call occurrence
    // for it because it names no declaration. Once checking admits the atom
    // as a proof view, the checked ledger still owes the spelling explicit
    // compiler-owned custody: mint one finalized ProofView row per exact call
    // site and retain its expression association in checked facts. Typed
    // resolution data stays frozen, including its original occurrence roster.
    let mut finalized_selection_occurrences = arena::Arena::new();
    let mut minted_view_calls: Vec<(
        source::SourceSpan,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
        AuthoredDeclarationSelectionOccurrenceId,
    )> = Vec::new();
    for (source_span, exposure, expression) in unoccurred_view_calls {
        let occurrence = match minted_view_calls
            .iter()
            .find(|(seen_span, seen_exposure, _)| {
                *seen_span == source_span && *seen_exposure == exposure
            }) {
            Some((_, _, occurrence)) => *occurrence,
            None => {
                let occurrence = selections
                    .record_late_bound(
                        source_span,
                        exposure,
                        AuthoredDeclarationSelectionKind::Call,
                        AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "failed to retain proof-view call selection: {error:?}"
                        ))
                        .with_source_span(source_span)
                    })?;
                selections
                    .finalize_intrinsic(
                        occurrence,
                        AuthoredDeclarationSelectionLateBinding::CheckedCall,
                        AuthoredDeclarationSelectionIntrinsic::ProofView,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "failed to finalize proof-view call selection: {error:?}"
                        ))
                        .with_source_span(source_span)
                    })?;
                minted_view_calls.push((source_span, exposure, occurrence));
                occurrence
            }
        };
        if !finalized_selection_occurrences
            .iter()
            .any(|(_, retained)| *retained == (expression, Some(occurrence)))
        {
            finalized_selection_occurrences.insert((expression, Some(occurrence)));
        }
    }
    for (source_span, exposure, selected_symbol) in inferred_conformances {
        let already_retained = selections
            .resolved_occurrence(
                source_span,
                exposure,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance,
                selected_symbol,
            )
            .is_some();
        if !already_retained {
            selections
                .record_resolved(
                    source_span,
                    exposure,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance,
                    selected_symbol,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain checked authored conformance selection: {error:?}"
                    ))
                    .with_source_span(source_span)
            })?;
        }
    }
    // Psi checks every target-scoped body, so a target sibling resolves its
    // selections here like any other body. Only execution descriptors stay
    // late-bound for their projection.
    let descriptor_occurrences =
        validation::execution_projection::descriptor_selection_occurrences(program);
    if let Some(selection) = selections.iter().find(|selection| {
        if !matches!(
            selection.target(),
            AuthoredDeclarationSelectionTarget::LateBound(_)
        ) || descriptor_occurrences.contains(&selection.occurrence_id())
        {
            return false;
        }
        program
            .symbols
            .source_file(selection.source_span())
            .is_none_or(|source| source.origin != source::SourceOrigin::Toolchain)
    }) {
        let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
            unreachable!("guarded late-bound authored selection")
        };
        if selection.kind() == AuthoredDeclarationSelectionKind::Call
            && binding == AuthoredDeclarationSelectionLateBinding::CheckedCall
            && let Some(callee) = undeclared_checked_call_callee(program, *selection)
        {
            return Err(Diagnostic::error(format!(
                "call `{callee}` selects no declaration: `{callee}` is not a declared machine, data, or proof definition"
            ))
            .with_source_span(selection.source_span()));
        }
        let location = program
            .symbols
            .source_file(selection.source_span())
            .map(|source| source.path.display().to_string())
            .unwrap_or_else(|| "<no source file>".to_owned());
        return Err(Diagnostic::error(format!(
            "authored {:?} declaration selection occurrence {} (`{}` in {location}) remained unresolved after successful checking ({binding:?})",
            selection.kind(),
            selection.occurrence_id().ordinal(),
            program.symbols.source_text(selection.source_span()),
        ))
        .with_source_span(selection.source_span()));
    }
    facts.finalized_selection_occurrences = finalized_selection_occurrences;
    Ok(selections)
}

fn late_binding_ordinal(
    program: &dyn ::validation::CheckingInput,
    prior_occurrences: &[AuthoredDeclarationSelectionOccurrenceId],
    binding: AuthoredDeclarationSelectionLateBinding,
) -> usize {
    prior_occurrences
        .iter()
        .filter(|occurrence| {
            program
                .authored_declaration_selections()
                .get(**occurrence)
                .is_some_and(|selection| {
                    selection.target() == AuthoredDeclarationSelectionTarget::LateBound(binding)
                })
        })
        .count()
}

/// Names the receiverless authored call target that stayed late-bound because
/// no declaration carries the name — the `Bag(items)` proof-view shape, where
/// every declared-target resolution correctly found no candidate. Statement
/// calls resolve through the same rule with their own selection row: a call
/// whose callee was filtered out of the selected program (e.g. a
/// foreign-target method) records no call fact, so only the call row's own
/// selection occurrence still names it. Receiver calls, qualified paths, and
/// already-bound targets keep the generic unresolved-selection diagnostic so
/// selection ambiguity retains its ledger identity.
fn undeclared_checked_call_callee(
    program: &dyn ::validation::CheckingInput,
    selection: AuthoredDeclarationSelection,
) -> Option<String> {
    if let Some(name) = undeclared_statement_call_callee(program, selection) {
        return Some(name);
    }
    let expressions = &program.tables.expression_table;
    expressions
        .iter_expressions()
        .find_map(|(expression, node)| {
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            if !expressions
                .authored_selection_occurrences(expression)
                .any(|occurrence| occurrence == selection.occurrence_id())
            {
                return None;
            }
            if call.target_symbol.is_valid() || call.receiver.is_valid() {
                return None;
            }
            let name = call.target.as_str();
            if name.contains("::") {
                return None;
            }
            (!top_level_declaration_exists(program, name, selection.source_span()))
                .then(|| name.to_string())
        })
}

/// The undeclared-callee check's declaration roster: every top-level kind
/// that could still carry the call's target name in the selected program.
fn top_level_declaration_exists(
    program: &dyn ::validation::CheckingInput,
    name: &str,
    reference: source::SourceSpan,
) -> bool {
    program
        .symbols
        .find_top_level_declaration_or_operator_family_from_source(
            name,
            &[
                SymbolKind::Machine,
                SymbolKind::Data,
                SymbolKind::MathematicalDefinition,
                SymbolKind::Proposition,
                SymbolKind::BuiltinFunction,
                SymbolKind::Operator,
                SymbolKind::Measure,
                SymbolKind::Domain,
            ],
            reference,
        )
        .is_some()
}

/// The statement-call twin of `undeclared_checked_call_callee`. A statement
/// call (`legs.acquire();`) carries its authored selection on the call row
/// rather than the expression table, and when the callee's declaration was
/// filtered out of the selected program the checked call facts record no row
/// — the statement would otherwise compile as if absent. The callee name is
/// `target`, so receiver calls are named here where the expression path
/// skips them. The name counts as declared only when the selected program
/// carries it somewhere — a top-level declaration, a machine, or any
/// machine's state — so a surviving same-named method still suppresses the
/// diagnostic and selection ambiguity keeps the generic ledger identity.
fn undeclared_statement_call_callee(
    program: &dyn ::validation::CheckingInput,
    selection: AuthoredDeclarationSelection,
) -> Option<String> {
    let call = program.machines().iter().find_map(|machine| {
        program.machine_states(machine).iter().find_map(|state| {
            program
                .statement_table
                .iter_statements(state.statement_nodes)
                .find_map(|(_, statement)| match statement {
                    typed_trees::statement::StatementNode::Call(call)
                        if call.authored_call_selection == Some(selection.occurrence_id()) =>
                    {
                        Some(call)
                    }
                    _ => None,
                })
        })
    })?;
    if call.target_symbol.is_valid() {
        return None;
    }
    let name = call.target.as_str();
    if name.contains("::") {
        return None;
    }
    let declared = top_level_declaration_exists(program, name, selection.source_span())
        || program.machines().iter().any(|machine| {
            machine.name.as_str() == name
                || program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.name.as_str() == name)
        });
    (!declared).then(|| name.to_string())
}
