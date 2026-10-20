//! Checked call, conformance and name-path targets.

use crate::authored_selections::contract_resolution;
use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use validation::selection_resolution::exact_named_operator_call;

pub(crate) fn checked_operator_conformance_targets(
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    let mut targets = Vec::new();
    facts
        .operators
        .for_each_expression_use(expression, |_, operator_use| {
            if operator_use.status != CheckedOperatorResolutionStatus::Resolved {
                return;
            }
            let Some(candidate) = facts.operators.selected_candidate(operator_use) else {
                return;
            };
            if candidate.is_trait_backed() && !targets.contains(&candidate.conformance_symbol) {
                targets.push(candidate.conformance_symbol);
            }
        });
    targets
}

pub(crate) fn checked_statement_call_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    authored_target: SymbolHandle,
) -> SymbolHandle {
    if authored_target.is_valid() {
        return authored_target;
    }
    let Some(state) = facts.flow.control.state(machine_symbol, state_symbol) else {
        return SymbolHandle::invalid();
    };
    for call in facts.flow.control.calls.span_or_empty(state.calls) {
        if call.statement_index != statement_index || !call.target_symbol.is_valid() {
            continue;
        }
        if matches!(
            validation::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call.call_ordinal,
            ),
            Some(validation::CallSite::Statement(_))
        ) {
            return call.target_symbol;
        }
    }
    SymbolHandle::invalid()
}

pub(crate) fn checked_call_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    authored_target: SymbolHandle,
    authored_source_span: source::SourceSpan,
) -> SymbolHandle {
    if authored_target.is_valid() {
        return authored_target;
    }
    let mut checked_named_target = None;
    for use_fact in facts
        .operators
        .named_uses()
        .filter(|use_fact| use_fact.expression == expression)
    {
        if !use_fact.selected_operator_symbol.is_valid()
            || checked_named_target
                .is_some_and(|target| target != use_fact.selected_operator_symbol)
        {
            return SymbolHandle::invalid();
        }
        checked_named_target = Some(use_fact.selected_operator_symbol);
    }
    if let Some(target) = checked_named_target {
        return target;
    }
    if let Some(call) = program.expression_table.call_node(expression)
        && let Some(operator) = exact_named_operator_call(program, call)
    {
        return operator.symbol;
    }
    if let Some(call) = program.expression_table.call_node(expression)
        && let Some(operator) = contract_resolution::checked_named_operator_call(
            program,
            facts,
            expression,
            call,
            authored_source_span,
            None,
        )
    {
        return operator.symbol;
    }
    if let Some(call) = program.expression_table.call_node(expression)
        && let Some(target) =
            validation::expression_owners::checked_machine_call_target_from_exact_owner(
                program,
                &facts.proof.contract_facts,
                expression,
                call,
                None,
            )
    {
        return target;
    }
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            if let Some(validation::CallSite::Expression {
                expression: candidate,
                ..
            }) = validation::find_call_site(
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
    let mut checked_source_target = None;
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            if !call.authored_source_custody_valid
                || call.authored_source_span != Some(authored_source_span)
                || !call.target_symbol.is_valid()
            {
                continue;
            }
            if checked_source_target.is_some_and(|target| target != call.target_symbol) {
                return SymbolHandle::invalid();
            }
            checked_source_target = Some(call.target_symbol);
        }
    }
    if let Some(target) = checked_source_target {
        return target;
    }
    let mut checked_fact_target = None;
    for projection in &facts.fact_call_projections {
        if projection.call_expression != expression || !projection.target_state.is_valid() {
            continue;
        }
        if checked_fact_target.is_some_and(|target| target != projection.target_state) {
            return SymbolHandle::invalid();
        }
        checked_fact_target = Some(projection.target_state);
    }
    if let Some(target) = checked_fact_target {
        return target;
    }
    SymbolHandle::invalid()
}

pub(crate) fn checked_name_path_segment_target(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
    target_index: usize,
) -> SymbolHandle {
    let direct = validation::resolve_name_path_member_symbol(program, path, target_index);
    if direct.is_valid() && target_index != 0 {
        return direct;
    }

    let mut contextual_root = None;
    let mut conflicting = false;
    super::member_targets::for_each_statement_containing(
        program,
        expression,
        |machine, state, statement_index, _| {
            let root = if direct.is_valid() {
                direct
            } else {
                let Some(validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    ..
                }) = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    expression,
                )
                else {
                    return Some(());
                };
                root
            };
            let root = authored_contextual_root(program, machine, state, statement_index, root);
            if contextual_root.is_some_and(|candidate| candidate != root) {
                conflicting = true;
                return None;
            }
            contextual_root = Some(root);
            Some(())
        },
    );
    if conflicting {
        return SymbolHandle::invalid();
    }

    let Some(mut selected) = contextual_root else {
        return direct;
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
        selected = validation::symbol_type_symbol(program, selected)
            .and_then(|type_symbol| {
                validation::resolve_member_symbol_from_type_symbol(
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
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
    let Some(template) = program.machine_by_symbol(template_symbol) else {
        return selected;
    };
    let Some(state_ordinal) =
        typed_trees::state::state_position(program.machine_states(machine), state.symbol)
    else {
        return selected;
    };
    let Some(template_state) = program.machine_states(template).get(state_ordinal) else {
        return selected;
    };

    if let Some(parameter_ordinal) =
        typed_trees::signature::state_parameter_position(program.state_parameters(state), selected)
    {
        return program
            .state_parameters(template_state)
            .get(parameter_ordinal)
            .map_or(selected, |parameter| parameter.symbol);
    }

    let template_statements = program
        .statement_table
        .statements(template_state.statement_nodes);
    if let Some((ordinal, _)) = program
        .state_local_declaration(state, selected)
        .filter(|&(ordinal, _)| ordinal < statement_index)
    {
        return template_statements
            .get(ordinal)
            .and_then(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            })
            .unwrap_or(selected);
    }
    selected
}

/// A call through a named implementation (`Ascending::before(left, right)`)
/// selects the implementation: its visibility and package authority govern
/// the call, and the row it reaches may be a synthesized default machine that
/// has no separate authority of its own.
pub(crate) fn implementation_call_selection(
    program: &dyn ::validation::CheckingInput,
    receiver: SymbolHandle,
) -> Option<SymbolHandle> {
    (receiver.is_valid() && program.symbols.get(receiver).kind == symbols::SymbolKind::Conformance)
        .then_some(receiver)
}

pub(crate) fn expression_receiver_symbol(
    program: &dyn ::validation::CheckingInput,
    receiver: typed_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    if !receiver.is_valid() {
        return SymbolHandle::invalid();
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(path) => path.symbol,
        _ => SymbolHandle::invalid(),
    }
}
