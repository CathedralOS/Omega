mod diagnostics;
mod expressions;
mod facts;
mod guards;
mod proofs;
mod state_arguments;

use diagnostics::{
    known_length_range_bound_failure, known_length_range_value_failure,
    unknown_length_range_failure,
};
use expressions::{
    expression_indexable_length, expression_integer_value, expression_is_slice, expression_name,
    provable_range_bounds,
};
use facts::{RangeFacts, fixed_array_field_lengths, fixed_array_type_length};
use guards::seed_guard_facts;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use proofs::{unknown_length_index_is_proven, unknown_length_range_is_proven};
use state_arguments::{collect_state_argument_facts, seed_state_argument_facts};

pub(crate) fn check_indexed_accesses(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let field_lengths = fixed_array_field_lengths(program);
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        let state_argument_facts = collect_state_argument_facts(program, &field_lengths, machine);
        for state in program.machine_states(machine) {
            let mut facts = RangeFacts::new(&field_lengths);
            seed_machine_requires(program, &mut facts, machine);
            seed_state_argument_facts(&mut facts, state, &state_argument_facts);
            for statement in program.statement_table.statements(state.statement_nodes) {
                check_statement(
                    program,
                    machine,
                    state,
                    &mut facts,
                    statement,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_statement(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            check_expression(
                program,
                machine,
                state,
                facts,
                assignment.target,
                diagnostics,
            );
            check_expression(
                program,
                machine,
                state,
                facts,
                assignment.value,
                diagnostics,
            );
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                let next_length = expression_indexable_length(program, facts, assignment.value);
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_local(symbol, name, next_length, next_integer);
            }
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                check_expression(program, machine, state, facts, *argument, diagnostics);
            }
        }
        StatementNode::Expression(expression) => {
            check_expression(program, machine, state, facts, *expression, diagnostics);
        }
        StatementNode::LocalData(local) => {
            check_expression(
                program,
                machine,
                state,
                facts,
                local.initial_value,
                diagnostics,
            );
            let length = expression_indexable_length(program, facts, local.initial_value)
                .or_else(|| fixed_array_type_length(program, local.type_reference));
            let integer = expression_integer_value(program, facts, local.initial_value);
            facts.define_local(local.symbol, local.name.to_string(), length, integer);
        }
        StatementNode::Transition(transition) => {
            let target_facts = match transition.guard {
                TransitionGuardNode::When(guard) => {
                    check_expression(program, machine, state, facts, guard, diagnostics);
                    let mut guarded_facts = facts.clone();
                    seed_guard_facts(program, &mut guarded_facts, guard);
                    guarded_facts
                }
                TransitionGuardNode::Always => facts.clone(),
            };
            check_transition_target(
                program,
                machine,
                state,
                &target_facts,
                transition.target,
                diagnostics,
            );
            check_transition_target(
                program,
                machine,
                state,
                &target_facts,
                transition.continuation,
                diagnostics,
            );
        }
    }
}

fn check_transition_target(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                check_expression(program, machine, state, facts, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(value) => {
            check_expression(program, machine, state, facts, *value, diagnostics)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn check_expression(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                check_expression(program, machine, state, facts, *value, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            check_expression(program, machine, state, facts, binary.left, diagnostics);
            check_expression(program, machine, state, facts, binary.right, diagnostics);
        }
        ExpressionNode::Call(call) => {
            check_expression(program, machine, state, facts, call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                check_expression(program, machine, state, facts, *argument, diagnostics);
            }
        }
        ExpressionNode::Cast(cast) => {
            check_expression(program, machine, state, facts, cast.value, diagnostics)
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(length) = expression_indexable_length(program, facts, indexed.collection) {
                check_index(
                    program,
                    facts,
                    indexed.collection,
                    indexed.index,
                    length,
                    diagnostics,
                );
            } else if expression_is_slice(program, machine, state, indexed.collection) {
                check_unknown_length_slice_index(
                    program,
                    facts,
                    indexed.collection,
                    indexed.index,
                    diagnostics,
                );
            }
            check_expression(
                program,
                machine,
                state,
                facts,
                indexed.collection,
                diagnostics,
            );
            check_expression(program, machine, state, facts, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => {
            check_expression(program, machine, state, facts, member.receiver, diagnostics);
        }
        ExpressionNode::Mutable(inner) => {
            check_expression(program, machine, state, facts, *inner, diagnostics)
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                check_expression(program, machine, state, facts, range.start, diagnostics);
            }
            if range.end.is_valid() {
                check_expression(program, machine, state, facts, range.end, diagnostics);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                check_expression(program, machine, state, facts, field.value, diagnostics);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn check_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    length: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => {
            check_range_index(program, facts, index, range, length, diagnostics)
        }
        _ => {
            let Some(index_value) = expression_integer_value(program, facts, index) else {
                let collection_label = program.expression_table.display_name(collection);
                let index_label = program.expression_table.display_name(index);
                if facts.index_is_proven(&collection_label, &index_label) {
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove index `{}` is within length {}",
                    index_label, length
                )));
                return;
            };
            let valid =
                index_value >= 0 && usize::try_from(index_value).is_ok_and(|index| index < length);
            if !valid {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove index `{}` is within length {}",
                    program.expression_table.display_name(index),
                    length
                )));
            }
        }
    }
}

fn check_unknown_length_slice_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => {
            if unknown_length_range_is_proven(program, facts, collection, range) {
                return;
            }
            let failure = unknown_length_range_failure(program, facts, collection, range);
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove subslice range {} `{}` is within unknown slice length",
                failure.label(),
                program.expression_table.display_name(index)
            )));
        }
        _ => {
            let index_label = program.expression_table.display_name(index);
            if unknown_length_index_is_proven(program, facts, collection, index) {
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove index `{}` is within unknown slice length",
                index_label
            )));
        }
    }
}

fn seed_machine_requires(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    machine: &omega_typed_trees::machine::Machine,
) {
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                omega_typed_trees::domain::ProofFact::Expression(expression) => {
                    seed_guard_facts(program, facts, *expression);
                    seed_index_proofs_from_expression(program, facts, *expression);
                }
                omega_typed_trees::domain::ProofFact::Membership(membership) => {
                    seed_index_proofs_from_expression(program, facts, membership.value);
                }
            }
        }
    }
}

fn seed_index_proofs_from_expression(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                seed_index_proofs_from_expression(program, facts, *value);
            }
        }
        ExpressionNode::Binary(binary) => {
            seed_index_proofs_from_expression(program, facts, binary.left);
            seed_index_proofs_from_expression(program, facts, binary.right);
        }
        ExpressionNode::Call(call) => {
            seed_index_proofs_from_expression(program, facts, call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                seed_index_proofs_from_expression(program, facts, *argument);
            }
        }
        ExpressionNode::Cast(cast) => seed_index_proofs_from_expression(program, facts, cast.value),
        ExpressionNode::Indexed(indexed) => {
            facts.prove_index(
                program.expression_table.display_name(indexed.collection),
                program.expression_table.display_name(indexed.index),
            );
            seed_index_proofs_from_expression(program, facts, indexed.collection);
            seed_index_proofs_from_expression(program, facts, indexed.index);
        }
        ExpressionNode::Member(member) => {
            seed_index_proofs_from_expression(program, facts, member.receiver);
        }
        ExpressionNode::Mutable(inner) => seed_index_proofs_from_expression(program, facts, *inner),
        ExpressionNode::Range(range) => {
            seed_index_proofs_from_expression(program, facts, range.start);
            seed_index_proofs_from_expression(program, facts, range.end);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                seed_index_proofs_from_expression(program, facts, field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn check_range_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
    range: &TableRangeExpression,
    length: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((start, end)) = provable_range_bounds(program, facts, range) else {
        let failure = known_length_range_value_failure(program, facts, range);
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove subslice range {} `{}` is within slice length {}",
            failure.label(),
            program.expression_table.display_name(index),
            length
        )));
        return;
    };

    if let Some(failure) = known_length_range_bound_failure(start, end, length) {
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove subslice range {} `{}` is within slice length {}",
            failure.label(),
            program.expression_table.display_name(index),
            length
        )));
    }
}
