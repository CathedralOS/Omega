//! Finalizing checked authored selections under a policy.

use crate::authored_selections::call_targets::{
    checked_call_conformance_targets, checked_call_target, checked_name_path_segment_target,
    checked_operator_conformance_targets, declaration_target,
};
use crate::authored_selections::intrinsic_calls::{
    checked_call_intrinsic, checked_intrinsic_call_target, exact_build_prelude_data,
    type_reference_names_exact_prelude_data,
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
use crate::authored_selections::{CheckedResolution, CheckedResolutionTarget};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionFinalizationError,
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionOccurrenceId,
    AuthoredDeclarationSelectionTarget,
};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

pub(crate) fn finalize_checked_authored_selections_with_policy(
    program: &mut TypedTrees,
    facts: &CheckFacts,
    allow_unresolved_toolchain: bool,
) -> Result<(), Diagnostic> {
    let mut resolutions = Vec::new();
    let mut inferred_conformances = Vec::new();
    let expressions = &program.tables.expression_table;
    // Built at the first open-generic operator occurrence and shared by every
    // later one: this loop reads the expression table and the checked facts
    // and writes neither, so one index answers them all.
    let mut generic_operator_values: Option<GenericOperatorValueOrigins> = None;

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let typed_trees::statement::StatementNode::RootBinding(binding) = statement else {
                    continue;
                };
                let receiver_type = crate::flow::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    binding.receiver,
                );
                // Source/result loan checking establishes receiver authority,
                // including a returned temporary's lifetime across its operand.
                // Selection finalization checks the exact compiler-owned type;
                // evaluation separately checks this activation's original cell.
                let mut receiver = binding.receiver;
                while let ExpressionNode::Borrow(borrow) = expressions.expression(receiver) {
                    if borrow.access != language_semantics::ReferenceAccess::Mutable {
                        return Err(Diagnostic::error(
                            "root binding requires a compiler-issued &mut Build place",
                        )
                        .with_source_span(binding.source_span));
                    }
                    receiver = borrow.target;
                }
                if !receiver_type.is_some_and(|receiver_type| {
                    matches!(program.type_reference_table.type_reference(receiver_type),
                        typed_trees::types::TypeReferenceNode::Reference { access, referee, .. }
                            if *access == language_semantics::ReferenceAccess::Mutable && exact_build_prelude_data(program, program.type_reference_table.type_symbol(*referee), "Build"))
                })
                {
                    return Err(Diagnostic::error("root binding requires a compiler-issued &mut Build receiver")
                        .with_source_span(binding.source_span));
                }
                if binding.slot.is_empty()
                    || (binding.implementation.is_empty()
                        && !binding.implementation_operand.is_valid())
                {
                    return Err(Diagnostic::error("root-slot binding requires exactly one slot path and one implementation path or description expression")
                        .with_source_span(binding.source_span));
                }
                if binding.implementation_operand.is_valid() {
                    // Description expressions undergo the ordinary call,
                    // effect and ownership checks. Their declared type is not
                    // issuance authority: build evaluation still rejoins the
                    // resulting value with this activation's issued entries.
                    let operand_type = crate::flow::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        binding.implementation_operand,
                    );
                    if !operand_type.is_some_and(|operand_type| {
                        type_reference_names_exact_prelude_data(
                            program,
                            operand_type,
                            "ProductEntryRef",
                        )
                    }) {
                        return Err(Diagnostic::error("delegated root binding requires an operand of the compiler-owned ProductEntryRef description type")
                            .with_source_span(binding.source_span));
                    }
                }
            }
        }
    }

    for (expression, node) in expressions.iter_expressions() {
        let occurrences = expressions
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return Err(Diagnostic::error(format!(
                    "expression retains unknown authored declaration selection occurrence {}",
                    occurrence.ordinal()
                )));
            };

            if selection.kind() == AuthoredDeclarationSelectionKind::Call
                && let ExpressionNode::Call(call) = node
            {
                for selected_symbol in checked_call_conformance_targets(
                    program,
                    facts,
                    expression,
                    call.target_symbol,
                    selection.source_span(),
                ) {
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
            let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
                continue;
            };

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
                ) if call.target.as_str() == "select_provider" => {
                    let argument_index = late_binding_ordinal(program, &occurrences[..occurrence_offset], binding);
                    let argument = call.machine_arguments.get(argument_index);
                    if super::provider_selection::is_build_provider_selection(program, expression) {
                        if call.machine_arguments.len() != 2 {
                            return Err(Diagnostic::error("provider selection requires exactly two product declaration operands")
                                .with_source_span(selection.source_span()));
                        }
                        let selected = argument.and_then(|argument| {
                            super::provider_selection::resolve_product_operand(
                                program, argument, selection.source_span(), argument_index == 0,
                            )
                        });
                        let Some(selected) = selected else {
                            return Err(Diagnostic::error("provider selection operand does not resolve to one visible product declaration")
                                .with_source_span(selection.source_span()));
                        };
                        declaration_target(selected)
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
                    Some(CheckedResolutionTarget::Intrinsic(match request.kind {
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
                ) => checked_intrinsic_call_target(facts, expression)
                    .or_else(|| {
                        checked_call_intrinsic(
                            program,
                            call.target.as_str(),
                            call.target_symbol,
                            call.receiver,
                        )
                    })
                    .map(CheckedResolutionTarget::Intrinsic)
                    .or_else(|| {
                        declaration_target(checked_call_target(
                            program,
                            facts,
                            expression,
                            call.target_symbol,
                            selection.source_span(),
                        ))
                    }),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => checked_member_target(program, facts, expression, member),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStaticPathSegment,
                    ExpressionNode::Name(path),
                ) => declaration_target(checked_name_path_segment_target(
                    program,
                    expression,
                    path,
                    late_binding_ordinal(program, &occurrences[..occurrence_offset], binding),
                )),
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
                                crate::flow::resolve_member_symbol_from_type_symbol(
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
                    ))
                    .or_else(|| {
                        typed_operator_has_no_authored_selection(program, expression).then_some(
                            CheckedResolutionTarget::Intrinsic(
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
                ) => Some(CheckedResolutionTarget::Intrinsic(
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
                        .then_some(CheckedResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        ))
                        .or_else(|| {
                            declaration_target(
                                crate::operators::token_bound_machine_call_target(program, call)
                                    .unwrap_or_else(SymbolHandle::invalid),
                            )
                        })
                }
                _ => None,
            };
            if let Some(target) = target {
                push_consistent_resolution(
                    &mut resolutions,
                    CheckedResolution {
                        occurrence,
                        binding,
                        target,
                    },
                )?;
            }
        }
    }

    collect_checked_statement_selections(
        program,
        facts,
        &mut resolutions,
        &mut inferred_conformances,
    )?;
    collect_checked_proof_membership_selections(program, facts, &mut resolutions)?;
    let mut unoccurred_view_calls = Vec::new();
    collect_checked_proof_view_call_selections(
        program,
        &mut resolutions,
        &mut unoccurred_view_calls,
    )?;

    let mut selections = program.authored_declaration_selections().clone();
    for resolution in resolutions {
        let result = match resolution.target {
            CheckedResolutionTarget::Declaration(selected) => {
                selections.finalize_late_bound(resolution.occurrence, resolution.binding, selected)
            }
            CheckedResolutionTarget::Intrinsic(intrinsic) => {
                selections.finalize_intrinsic(resolution.occurrence, resolution.binding, intrinsic)
            }
        };
        result.map_err(|error| finalization_diagnostic(resolution, error))?;
    }
    // A contract clause may spell a bare uninterpreted view atom
    // (`Bag(items)`); the resolver deliberately records no Call occurrence
    // for it because it names no declaration. Once checking admits the atom
    // as a proof view, the checked ledger still owes the spelling explicit
    // compiler-owned custody: mint one finalized ProofView row per exact call
    // site and attach it as the expression's occurrence.
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
        program
            .expression_table
            .attach_authored_selection_occurrences(expression, [occurrence]);
    }
    for (source_span, exposure, selected_symbol) in inferred_conformances {
        let already_retained = selections.iter().any(|selection| {
            selection.source_span() == source_span
                && selection.exposure() == exposure
                && selection.kind()
                    == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
                && matches!(
                    selection.target(),
                    AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == selected_symbol
                )
        });
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
    if let Some(selection) = selections.iter().find(|selection| {
        if !matches!(
            selection.target(),
            AuthoredDeclarationSelectionTarget::LateBound(_)
        ) {
            return false;
        }
        !allow_unresolved_toolchain
            || program
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
        return Err(Diagnostic::error(format!(
            "authored {:?} declaration selection occurrence {} remained unresolved after successful checking ({binding:?})",
            selection.kind(),
            selection.occurrence_id().ordinal(),
        ))
        .with_source_span(selection.source_span()));
    }
    program.retain_authored_declaration_selections(selections);
    Ok(())
}

fn late_binding_ordinal(
    program: &TypedTrees,
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

pub(crate) fn push_consistent_resolution(
    resolutions: &mut Vec<CheckedResolution>,
    candidate: CheckedResolution,
) -> Result<(), Diagnostic> {
    if let Some(existing) = resolutions
        .iter()
        .find(|resolution| resolution.occurrence == candidate.occurrence)
    {
        if *existing != candidate {
            return Err(Diagnostic::error(format!(
                "authored declaration selection occurrence {} resolved inconsistently across compiler-derived copies: {:?} selected {:?} and {:?}",
                candidate.occurrence.ordinal(),
                candidate.binding,
                existing.target,
                candidate.target
            )));
        }
        return Ok(());
    }
    resolutions.push(candidate);
    Ok(())
}

pub(crate) fn finalization_diagnostic(
    resolution: CheckedResolution,
    error: AuthoredDeclarationSelectionFinalizationError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to finalize authored declaration selection occurrence {}: {error:?}",
        resolution.occurrence.ordinal()
    ))
}

/// Names the receiverless authored call target that stayed late-bound because
/// no declaration carries the name — the `Bag(items)` proof-view shape, where
/// every declared-target resolution correctly found no candidate. Receiver
/// calls, qualified paths, and already-bound targets keep the generic
/// unresolved-selection diagnostic so selection ambiguity retains its ledger
/// identity.
fn undeclared_checked_call_callee(
    program: &TypedTrees,
    selection: AuthoredDeclarationSelection,
) -> Option<String> {
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
                    selection.source_span(),
                )
                .is_none()
                .then(|| name.to_string())
        })
}
