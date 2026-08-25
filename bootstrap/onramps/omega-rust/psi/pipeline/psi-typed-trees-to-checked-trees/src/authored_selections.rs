use psi_checked_trees::{CheckFacts, CheckedOperatorResolutionStatus};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionFinalizationError, AuthoredDeclarationSelectionIntrinsic,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::{TypedTrees, expression::ExpressionNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckedResolution {
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
    binding: AuthoredDeclarationSelectionLateBinding,
    target: CheckedResolutionTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckedResolutionTarget {
    Declaration(SymbolHandle),
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
}

pub(crate) fn finalize_checked_authored_selections(
    program: &mut TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Diagnostic> {
    let mut resolutions = Vec::new();
    let mut inferred_conformances = Vec::new();
    let expressions = &program.tables.expression_table;

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
                for selected_symbol in
                    checked_call_conformance_targets(program, facts, expression, call.target_symbol)
                {
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
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => checked_call_intrinsic(
                    program,
                    call.target.as_str(),
                    call.target_symbol,
                    call.receiver,
                )
                .map_or_else(
                    || {
                        declaration_target(checked_call_target(
                            program,
                            facts,
                            expression,
                            call.target_symbol,
                        ))
                    },
                    |intrinsic| Some(CheckedResolutionTarget::Intrinsic(intrinsic)),
                ),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => declaration_target(crate::flow::effective_member_symbol(
                    program,
                    member.receiver,
                    member,
                ))
                .or_else(|| contextual_domain_member_target(program, expression, member)),
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
                ) => declaration_target(literal.type_symbol),
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
                        .map(|field| field.field_symbol)
                        .unwrap_or_else(SymbolHandle::invalid),
                ),
                (AuthoredDeclarationSelectionLateBinding::CheckedOperator, _)
                    if matches!(node, ExpressionNode::Binary(_) | ExpressionNode::Unary(_)) =>
                {
                    checked_operator_target(program, facts, expression, node)
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
    for (source_span, exposure, selected_symbol) in inferred_conformances {
        let already_retained = selections.iter().any(|selection| {
            selection.source_span() == source_span
                && selection.exposure() == exposure
                && selection.kind()
                    == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
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
                    psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance,
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
    program.retain_authored_declaration_selections(selections);
    Ok(())
}

fn collect_checked_statement_selections(
    program: &TypedTrees,
    facts: &CheckFacts,
    resolutions: &mut Vec<CheckedResolution>,
    inferred_conformances: &mut Vec<(
        psi_source::SourceSpan,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
        SymbolHandle,
    )>,
) -> Result<(), Diagnostic> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let psi_typed_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                if call.operational_acknowledgement.origin
                    != psi_language_semantics::CallOperationalAcknowledgementOrigin::Source
                    || call.source_span.span.start >= call.source_span.span.end
                {
                    continue;
                }
                let target = checked_statement_call_target(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call.target_symbol,
                );
                if target.is_valid() {
                    for selected_symbol in checked_target_conformance_targets(program, target) {
                        let inferred = (
                            call.source_span,
                            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                            selected_symbol,
                        );
                        if !inferred_conformances.contains(&inferred) {
                            inferred_conformances.push(inferred);
                        }
                    }
                }

                let pending = program
                    .authored_declaration_selections()
                    .iter()
                    .filter(|selection| {
                        selection.source_span() == call.source_span
                            && selection.kind() == AuthoredDeclarationSelectionKind::Call
                            && selection.target()
                                == AuthoredDeclarationSelectionTarget::LateBound(
                                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                                )
                    })
                    .map(|selection| selection.occurrence_id())
                    .collect::<Vec<_>>();
                let resolution_target = checked_statement_call_intrinsic(program, state, call)
                    .map(CheckedResolutionTarget::Intrinsic)
                    .or_else(|| declaration_target(target));
                if let Some(target) = resolution_target {
                    for occurrence in pending {
                        push_consistent_resolution(
                            resolutions,
                            CheckedResolution {
                                occurrence,
                                binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
                                target,
                            },
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn checked_statement_call_intrinsic(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    if call.target_symbol.is_valid() {
        return None;
    }
    if exact_statement_build_output_receiver(program, state, call) {
        return Some(Intrinsic::BuildIncludedSourceHandoff);
    }
    checked_call_intrinsic(
        program,
        call.target.as_str(),
        call.target_symbol,
        psi_typed_trees::expression::ExpressionHandle::invalid(),
    )
}

fn checked_call_intrinsic(
    program: &TypedTrees,
    target: &str,
    target_symbol: SymbolHandle,
    receiver: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    // A resolved declaration always wins over compiler vocabulary with the
    // same spelling. Receiver calls likewise cannot select these free-call
    // intrinsics. This keeps intrinsic recognition from becoming a
    // source-text fallback for an ordinary package declaration.
    if target_symbol.is_valid() {
        None
    } else if receiver.is_valid() {
        exact_build_output_receiver(program, receiver, target)
            .then_some(Intrinsic::BuildIncludedSourceHandoff)
    } else if let Some(predicate) =
        psi_language_semantics::byte_predicates::ByteSequencePredicate::from_name(target)
    {
        Some(Intrinsic::ByteSequencePredicate(predicate))
    } else if target == "select_provider" {
        Some(Intrinsic::BuildProviderSelection)
    } else if target.starts_with("accept_boundary#") {
        Some(Intrinsic::BuildBoundaryAcceptance)
    } else if target.starts_with("wire_compatibility#") {
        Some(Intrinsic::BuildWireCompatibilityRequest)
    } else if target.starts_with("bind_root#") {
        Some(Intrinsic::BuildRootBinding)
    } else if target.starts_with("asm#") {
        Some(Intrinsic::InlineAssemblyOperation)
    } else {
        None
    }
}

fn exact_build_output_receiver(
    program: &TypedTrees,
    receiver: psi_typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "include_source" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_toolchain_data(program, type_symbol, "BuildOutput"))
}

fn exact_statement_build_output_receiver(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
) -> bool {
    if call.target.as_str() != "include_source" {
        return false;
    }
    let [root, members @ ..] = program.statement_table.name_path_members(call.receiver) else {
        return false;
    };
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == root.as_str())
    else {
        return false;
    };
    let mut type_symbol = program
        .type_reference_table
        .type_symbol(parameter.type_reference);
    if !exact_toolchain_data(program, type_symbol, "Build") {
        return false;
    }
    for member in members {
        let Some(selected) = crate::flow::resolve_member_symbol_from_type_symbol(
            program,
            type_symbol,
            member.as_str(),
        ) else {
            return false;
        };
        let Some(selected_type) = crate::flow::symbol_type_symbol(program, selected) else {
            return false;
        };
        type_symbol = selected_type;
    }
    exact_toolchain_data(program, type_symbol, "BuildOutput")
}

fn exact_toolchain_data(program: &TypedTrees, type_symbol: SymbolHandle, name: &str) -> bool {
    program.symbols.symbol_source_origin(type_symbol) == Some(psi_source::SourceOrigin::Toolchain)
        && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == type_symbol && data.name.as_str() == name)
}

fn checked_operator_conformance_targets(
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    let mut targets = Vec::new();
    for (_, operator_use) in facts.operators.uses.iter() {
        if operator_use.expression != expression
            || operator_use.status != CheckedOperatorResolutionStatus::Resolved
        {
            continue;
        }
        let Some(candidate) = facts.operators.selected_candidate(operator_use) else {
            continue;
        };
        if candidate.is_trait_backed() && !targets.contains(&candidate.conformance_symbol) {
            targets.push(candidate.conformance_symbol);
        }
    }
    targets
}

fn checked_call_conformance_targets(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    authored_target: SymbolHandle,
) -> Vec<SymbolHandle> {
    let target = checked_call_target(program, facts, expression, authored_target);
    checked_target_conformance_targets(program, target)
}

fn checked_target_conformance_targets(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Vec<SymbolHandle> {
    let Some(machine_symbol) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target)
            .then_some(machine.symbol)
    }) else {
        return Vec::new();
    };

    let mut targets = Vec::new();
    for specialization in &program.machine_specializations {
        if specialization.instance != machine_symbol {
            continue;
        }
        for selected in &specialization.inferred_conformance_arguments {
            if !targets.contains(selected) {
                targets.push(*selected);
            }
        }
    }
    targets
}

fn checked_statement_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    authored_target: SymbolHandle,
) -> SymbolHandle {
    if authored_target.is_valid() {
        return authored_target;
    }
    let Some(state) = facts.flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    }) else {
        return SymbolHandle::invalid();
    };
    for call in facts.flow.control.calls.span_or_empty(state.calls) {
        if call.statement_index != statement_index || !call.target_symbol.is_valid() {
            continue;
        }
        if matches!(
            crate::semantic_calls::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call.call_ordinal,
            ),
            Some(crate::semantic_calls::CallSite::Statement(_))
        ) {
            return call.target_symbol;
        }
    }
    SymbolHandle::invalid()
}

fn checked_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    authored_target: SymbolHandle,
) -> SymbolHandle {
    if authored_target.is_valid() {
        return authored_target;
    }
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            if let Some(crate::semantic_calls::CallSite::Expression {
                expression: candidate,
                ..
            }) = crate::semantic_calls::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) && candidate == expression
                && call.target_symbol.is_valid()
            {
                return call.target_symbol;
            }
        }
    }
    SymbolHandle::invalid()
}

fn checked_name_path_segment_target(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
    target_index: usize,
) -> SymbolHandle {
    let direct = crate::lookup::resolve_name_path_member_symbol(program, path, target_index);
    if direct.is_valid() {
        return direct;
    }

    let mut contextual_root = None;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&expression) {
                    continue;
                }
                let Some(crate::flow::CanonicalPlace {
                    root: psi_facts::PlaceRoot::Symbol(root),
                    ..
                }) = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    expression,
                )
                else {
                    continue;
                };
                let root = authored_contextual_root(program, machine, state, statement_index, root);
                if contextual_root.is_some_and(|candidate| candidate != root) {
                    return SymbolHandle::invalid();
                }
                contextual_root = Some(root);
            }
        }
    }

    let Some(mut selected) = contextual_root else {
        return SymbolHandle::invalid();
    };
    if target_index == 0 {
        return selected;
    }
    for member in program
        .expression_table
        .name_path_members(path.members)
        .iter()
        .skip(1)
        .take(target_index)
    {
        selected = crate::flow::symbol_type_symbol(program, selected)
            .and_then(|type_symbol| {
                crate::flow::resolve_member_symbol_from_type_symbol(
                    program,
                    type_symbol,
                    member.as_str(),
                )
            })
            .unwrap_or_else(SymbolHandle::invalid);
        if !selected.is_valid() {
            break;
        }
    }
    selected
}

fn authored_contextual_root(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    selected: SymbolHandle,
) -> SymbolHandle {
    let Some(template_symbol) = program
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.instance == machine.symbol
                && specialization.template != specialization.instance
        })
        .map(|specialization| specialization.template)
    else {
        return selected;
    };
    let Some(template) = crate::lookup::machine_by_symbol(program, template_symbol) else {
        return selected;
    };
    let Some(state_ordinal) = program
        .machine_states(machine)
        .iter()
        .position(|candidate| candidate.symbol == state.symbol)
    else {
        return selected;
    };
    let Some(template_state) = program.machine_states(template).get(state_ordinal) else {
        return selected;
    };

    if let Some(parameter_ordinal) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == selected)
    {
        return program
            .state_parameters(template_state)
            .get(parameter_ordinal)
            .map_or(selected, |parameter| parameter.symbol);
    }

    let statements = program.statement_table.statements(state.statement_nodes);
    let template_statements = program
        .statement_table
        .statements(template_state.statement_nodes);
    for (ordinal, statement) in statements.iter().take(statement_index).enumerate() {
        let psi_typed_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.symbol != selected {
            continue;
        }
        return template_statements
            .get(ordinal)
            .and_then(|statement| match statement {
                psi_typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            })
            .unwrap_or(selected);
    }
    selected
}

fn declaration_target(symbol: SymbolHandle) -> Option<CheckedResolutionTarget> {
    symbol
        .is_valid()
        .then_some(CheckedResolutionTarget::Declaration(symbol))
}

fn intrinsic_operator_operand_is_primitive(
    program: &TypedTrees,
    node: &ExpressionNode,
    origin: psi_checked_trees::CheckedValueOrigin,
) -> bool {
    let operand = match node {
        ExpressionNode::Binary(binary) => binary.left,
        ExpressionNode::Unary(unary) => unary.operand,
        _ => return false,
    };
    crate::operators::expression_type_reference_for_origin(program, operand, origin)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
        || expression_is_intrinsic_primitive_without_origin(program, operand)
}

fn checked_operator_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> Option<CheckedResolutionTarget> {
    // These operators have no authored declaration/spelling surface. Once
    // ordinary checking accepts their operand types, their exact meaning is
    // necessarily compiler intrinsic; nested-expression origins need not
    // recover a synthetic type reference merely to finalize custody.
    if matches!(
        node,
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                psi_typed_trees::expression::BinaryOperator::And
                    | psi_typed_trees::expression::BinaryOperator::BitwiseAnd
                    | psi_typed_trees::expression::BinaryOperator::BitwiseOr
                    | psi_typed_trees::expression::BinaryOperator::BitwiseXor
                    | psi_typed_trees::expression::BinaryOperator::Or
                    | psi_typed_trees::expression::BinaryOperator::ShiftLeft
                    | psi_typed_trees::expression::BinaryOperator::ShiftRight
            )
    ) {
        return Some(CheckedResolutionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        ));
    }

    let uses = facts
        .operators
        .uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.expression == expression).then_some(operator_use)
        })
        .collect::<Vec<_>>();

    uses.iter()
        .find_map(|operator_use| {
            (operator_use.status == CheckedOperatorResolutionStatus::Resolved)
                .then(|| declaration_target(operator_use.selected_operator_symbol))
                .flatten()
        })
        .or_else(|| {
            uses.iter()
                .any(|operator_use| {
                    operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
                        || (operator_use.status == CheckedOperatorResolutionStatus::Missing
                            && intrinsic_operator_operand_is_primitive(
                                program,
                                node,
                                operator_use.origin,
                            ))
                })
                .then_some(CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ))
        })
        .or_else(|| {
            resolve_authored_operator_without_use_fact(program, node)
                .and_then(|operator| declaration_target(operator.symbol))
        })
        .or_else(|| {
            let operand = match node {
                ExpressionNode::Binary(binary) => binary.left,
                ExpressionNode::Unary(unary) => unary.operand,
                _ => return None,
            };
            (expression_is_intrinsic_primitive_without_origin(program, operand)
                || expression_is_contextual_domain_primitive(program, expression, operand))
            .then_some(CheckedResolutionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
            ))
        })
        .or_else(|| {
            operator_has_no_authored_spelling_candidate(program, node).then_some(
                CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ),
            )
        })
}

/// Declaration contracts and other proof-static expressions are validated
/// without an executable value origin, so they do not always produce a
/// `CheckedOperatorUseFact`. Finalize their authored selection only when the
/// typed operands select one exact declared operator; partial or ambiguous
/// reconstruction remains unresolved and therefore rejects package custody.
fn resolve_authored_operator_without_use_fact<'program>(
    program: &'program TypedTrees,
    node: &ExpressionNode,
) -> Option<&'program psi_typed_trees::operator::OperatorDefinition> {
    use psi_language_core::OperatorSpelling;
    use psi_typed_trees::expression::BinaryOperator;

    let ExpressionNode::Binary(binary) = node else {
        return None;
    };
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return None,
    };
    let operand_types = [
        authored_operand_type(program, binary.left)?,
        authored_operand_type(program, binary.right)?,
    ];
    let candidates = psi_typed_trees::operator::resolve_spelling_for_operands(
        program,
        spelling,
        &operand_types.map(Some),
    );
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    Some(candidate.operator)
}

fn authored_operand_type(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => authored_operand_type(program, atomic.value),
        ExpressionNode::Borrow(borrow) => authored_operand_type(program, borrow.target),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Name(path) => program
            .state_parameters
            .iter()
            .find_map(|(_, parameter)| {
                (path.symbol.is_valid() && parameter.symbol == path.symbol)
                    .then_some(parameter.type_reference)
            })
            .or_else(|| operator_contract_parameter_type(program, expression, path)),
        _ => None,
    }
}

fn operator_contract_parameter_type(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    let mut types = program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
        .filter(|operator| {
            program.operator_contracts(operator).iter().any(|contract| {
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| match fact {
                        psi_typed_trees::domain::ProofFact::Expression(root) => {
                            expression_tree_contains(program, *root, expression)
                        }
                        psi_typed_trees::domain::ProofFact::Membership(membership) => {
                            expression_tree_contains(program, membership.value, expression)
                        }
                        psi_typed_trees::domain::ProofFact::Proposition(_) => false,
                    })
            })
        })
        .filter_map(|operator| {
            program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| parameter.name.as_str() == name)
                .map(|parameter| parameter.type_reference)
        });
    let first = types.next()?;
    types
        .all(|type_reference| type_reference == first)
        .then_some(first)
}

fn expression_tree_contains(
    program: &TypedTrees,
    root: psi_typed_trees::expression::ExpressionHandle,
    target: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    use psi_typed_trees::expression::ExpressionNode;

    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if expression == target {
            return true;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => pending.push(atomic.value),
            ExpressionNode::Binary(binary) => {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            _ => {}
        }
    }
    false
}

fn operator_has_no_authored_spelling_candidate(
    program: &TypedTrees,
    node: &ExpressionNode,
) -> bool {
    let ExpressionNode::Binary(binary) = node else {
        // Unary operators have no authored declaration dispatch surface.
        return matches!(node, ExpressionNode::Unary(_));
    };
    use psi_language_core::OperatorSpelling;
    use psi_typed_trees::expression::BinaryOperator;
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return true,
    };
    psi_typed_trees::operator::resolve_spelling(program, spelling, None).is_empty()
}

fn contextual_domain_member_target(
    program: &TypedTrees,
    containing_expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let target_type = contextual_domain_target_type(program, containing_expression)?;
    contextual_self_member_symbol(program, member, target_type).and_then(declaration_target)
}

fn expression_is_contextual_domain_primitive(
    program: &TypedTrees,
    containing_expression: psi_typed_trees::expression::ExpressionHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(target_type) = contextual_domain_target_type(program, containing_expression) else {
        return false;
    };
    contextual_expression_type_reference(program, expression, target_type)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
}

fn contextual_expression_type_reference(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    domain_target_type: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if contextual_self_path(program, path) => {
            Some(domain_target_type)
        }
        ExpressionNode::Member(member) => {
            contextual_self_member_symbol(program, member, domain_target_type)
                .and_then(|symbol| type_reference_for_symbol(program, symbol))
        }
        ExpressionNode::Borrow(inner) => {
            contextual_expression_type_reference(program, inner.target, domain_target_type)
        }
        _ => None,
    }
}

fn contextual_self_member_symbol(
    program: &TypedTrees,
    member: &psi_typed_trees::expression::TableMemberExpression,
    domain_target_type: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver) else {
        return None;
    };
    if !contextual_self_path(program, path) {
        return None;
    }
    crate::flow::resolve_member_symbol_from_type_symbol(
        program,
        program.type_reference_table.type_symbol(domain_target_type),
        member.member.as_str(),
    )
}

fn contextual_self_path(
    program: &TypedTrees,
    path: &psi_typed_trees::expression::TableNamePath,
) -> bool {
    let members = program.expression_table.name_path_members(path.members);
    !path.symbol.is_valid() && members.len() == 1 && members[0].as_str() == "self"
}

fn contextual_domain_target_type(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    for domain in program.domain_definitions() {
        for fact in program.proof_facts(domain) {
            let root = match fact {
                psi_typed_trees::domain::ProofFact::Expression(root) => *root,
                psi_typed_trees::domain::ProofFact::Membership(membership) => membership.value,
                psi_typed_trees::domain::ProofFact::Proposition(_) => continue,
            };
            if expression_contains(program, root, expression, &mut Vec::new()) {
                return Some(domain.target_type);
            }
        }
    }
    None
}

fn expression_contains(
    program: &TypedTrees,
    root: psi_typed_trees::expression::ExpressionHandle,
    target: psi_typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<psi_typed_trees::expression::ExpressionHandle>,
) -> bool {
    if !root.is_valid() || visited.contains(&root) {
        return false;
    }
    if root == target {
        return true;
    }
    visited.push(root);
    match program.expression_table.expression(root) {
        ExpressionNode::Atomic(atomic) => {
            expression_contains(program, atomic.value, target, visited)
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|child| expression_contains(program, *child, target, visited)),
        ExpressionNode::Binary(binary) => {
            expression_contains(program, binary.left, target, visited)
                || expression_contains(program, binary.right, target, visited)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_contains(program, call.receiver, target, visited))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|child| expression_contains(program, *child, target, visited))
        }
        ExpressionNode::Cast(cast) => expression_contains(program, cast.value, target, visited),
        ExpressionNode::Indexed(indexed) => {
            expression_contains(program, indexed.collection, target, visited)
                || expression_contains(program, indexed.index, target, visited)
        }
        ExpressionNode::Member(member) => {
            expression_contains(program, member.receiver, target, visited)
        }
        ExpressionNode::Borrow(inner) => {
            expression_contains(program, inner.target, target, visited)
        }
        ExpressionNode::Range(range) => {
            expression_contains(program, range.start, target, visited)
                || expression_contains(program, range.end, target, visited)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains(program, field.value, target, visited)),
        ExpressionNode::Unary(unary) => {
            expression_contains(program, unary.operand, target, visited)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn expression_is_intrinsic_primitive_without_origin(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let type_reference = match program.tables.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) | ExpressionNode::Float(_) | ExpressionNode::Integer(_) => {
            return true;
        }
        ExpressionNode::Name(path) => type_reference_for_symbol(program, path.symbol),
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find_map(|state| (state.symbol == call.target_symbol).then_some(state.return_type)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => type_reference_for_symbol(
            program,
            crate::flow::effective_member_symbol(program, member.receiver, member),
        ),
        ExpressionNode::Borrow(inner) => {
            return expression_is_intrinsic_primitive_without_origin(program, inner.target);
        }
        _ => None,
    };
    type_reference
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
}

pub(crate) fn typed_operator_is_definitely_intrinsic(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let operand = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => binary.left,
        ExpressionNode::Unary(unary) => unary.operand,
        _ => return false,
    };
    expression_is_intrinsic_primitive_without_origin(program, operand)
}

fn type_reference_for_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(field.type_reference);
                }
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(type_reference) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find_map(|field| (field.symbol == symbol).then_some(field.type_reference))
                    {
                        return Some(type_reference);
                    }
                }
                _ => {}
            }
        }
    }
    for machine in program.machines() {
        if let Some(type_reference) = program
            .machine_owned_data(machine)
            .iter()
            .find_map(|owned| (owned.symbol == symbol).then_some(owned.type_reference))
        {
            return Some(type_reference);
        }
        for state in program.machine_states(machine) {
            if let Some(type_reference) =
                program
                    .state_parameters(state)
                    .iter()
                    .find_map(|parameter| {
                        (parameter.symbol == symbol).then_some(parameter.type_reference)
                    })
            {
                return Some(type_reference);
            }
            if let Some(type_reference) = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    psi_typed_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == symbol =>
                    {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
            {
                return Some(type_reference);
            }
        }
    }
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            if let Some(type_reference) = program
                .state_signature_parameters(signature)
                .iter()
                .find_map(|parameter| {
                    (parameter.symbol == symbol).then_some(parameter.type_reference)
                })
            {
                return Some(type_reference);
            }
        }
    }
    for proposition in program.propositions() {
        if let Some(type_reference) = program
            .proposition_parameters(proposition)
            .iter()
            .find_map(|parameter| (parameter.symbol == symbol).then_some(parameter.type_reference))
        {
            return Some(type_reference);
        }
    }
    None
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

fn push_consistent_resolution(
    resolutions: &mut Vec<CheckedResolution>,
    candidate: CheckedResolution,
) -> Result<(), Diagnostic> {
    if let Some(existing) = resolutions
        .iter()
        .find(|resolution| resolution.occurrence == candidate.occurrence)
    {
        if *existing != candidate {
            return Err(Diagnostic::error(format!(
                "authored declaration selection occurrence {} resolved inconsistently across compiler-derived copies",
                candidate.occurrence.ordinal()
            )));
        }
        return Ok(());
    }
    resolutions.push(candidate);
    Ok(())
}

fn finalization_diagnostic(
    resolution: CheckedResolution,
    error: AuthoredDeclarationSelectionFinalizationError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to finalize authored declaration selection occurrence {}: {error:?}",
        resolution.occurrence.ordinal()
    ))
}
