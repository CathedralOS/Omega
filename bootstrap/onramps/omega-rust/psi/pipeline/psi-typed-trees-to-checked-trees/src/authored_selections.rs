use psi_checked_trees::{CheckFacts, CheckedOperatorResolutionStatus};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionFinalizationError, AuthoredDeclarationSelectionIntrinsic,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionOccurrenceId,
    AuthoredDeclarationSelectionTarget,
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
            let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
                continue;
            };

            let target = match (binding, node) {
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => declaration_target(checked_call_target(
                    program,
                    facts,
                    expression,
                    call.target_symbol,
                )),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => declaration_target(crate::flow::effective_member_symbol(
                    program,
                    member.receiver,
                    member,
                )),
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
    program.retain_authored_declaration_selections(selections);
    Ok(())
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
            let operand = match node {
                ExpressionNode::Binary(binary) => binary.left,
                ExpressionNode::Unary(unary) => unary.operand,
                _ => return None,
            };
            expression_is_intrinsic_primitive_without_origin(program, operand).then_some(
                CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ),
            )
        })
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
