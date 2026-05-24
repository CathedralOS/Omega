use super::*;
mod accesses;
mod calls;
mod roots;

use accesses::borrow_access_place;
use calls::collect_statement_borrow_calls;
use crate::lookup::machine_state_count;
use crate::semantic_calls::find_state;
use roots::{append_state_writable_roots, estimated_borrow_root_capacity, mutable_parameter_count};

pub(crate) fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        omega_core::arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut access_segments =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut loans =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = omega_core::arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            append_state_writable_roots(
                program,
                machine,
                state,
                &mut writable_roots,
                &mut writable_roots_span,
            );

            let mut calls_span = omega_core::arena::HandleSpan::empty();
            let mut loans_span = omega_core::arena::HandleSpan::empty();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                if let Some(place) = statement_borrow_loan_place(
                    program,
                    machine.symbol,
                    statement,
                )
                {
                    loans.append_to_span(&mut loans_span, omega_checked_trees::BorrowLoanFact {
                        statement_index,
                        owner_symbol: local_borrow_owner_symbol(statement).unwrap(),
                        root_symbol: place.root_symbol,
                        segments: access_segments.insert_many(place.segments),
                    });
                }
                let mut call_ordinal = 0usize;
                collect_statement_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &mut call_ordinal,
                    &mut access_segments,
                    &mut argument_accesses,
                    &mut calls,
                    &mut calls_span,
                );
            }

            states.append(StateBorrowFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: writable_roots_span,
                mutable_parameter_count: mutable_parameter_count(program, state),
                calls: calls_span,
                loans: loans_span,
            });
        }
    }

    BorrowFacts {
        writable_roots,
        access_segments,
        argument_accesses,
        calls,
        loans,
        states,
    }
}

fn statement_borrow_loan_place(
    program: &omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
) -> Option<accesses::BorrowAccessPlace> {
    let StatementNode::LocalData(local_data) = statement else {
        return None;
    };

    if !is_mutable_reference_type(program, local_data.type_reference) {
        return None;
    }

    match program.expression_table.expression(local_data.initial_value) {
        omega_checked_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            borrow_access_place(program, *inner_expression, machine_symbol)
        }
        omega_checked_trees::expression::ExpressionNode::Call(call) => {
            helper_call_borrow_loan_place(program, machine_symbol, call)
        }
        _ => None,
    }
}

fn local_borrow_owner_symbol(statement: &StatementNode) -> Option<SymbolHandle> {
    match statement {
        StatementNode::LocalData(local_data) => Some(local_data.symbol),
        _ => None,
    }
}

fn helper_call_borrow_loan_place(
    program: &omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    call: &omega_checked_trees::expression::TableCallExpression,
) -> Option<accesses::BorrowAccessPlace> {
    if !call.receiver.is_valid() {
        return None;
    }

    if call.target.as_str() == "as_mut_slice" {
        return borrow_access_place(program, call.receiver, machine_symbol);
    }

    let Some(target_state) = find_state(program, call.target_symbol) else {
        return None;
    };
    let receiver_is_mutable = program
        .state_parameters(target_state)
        .iter()
        .any(|parameter| parameter.is_self && parameter.is_mutable);

    if !receiver_is_mutable || !is_mutable_reference_type(program, target_state.return_type) {
        return None;
    }

    borrow_access_place(program, call.receiver, machine_symbol)
}

fn is_mutable_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { is_mutable, .. } => *is_mutable,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_mutable_reference_type(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}
