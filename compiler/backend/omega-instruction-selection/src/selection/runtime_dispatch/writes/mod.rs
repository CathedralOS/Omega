mod mutation;
mod static_values;
mod storage_copy;

use super::super::bindings::RuntimeAliasBinding;
use super::super::lookups::state_mutation_for_statement;
use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_target_operations::SelectedInstruction;
use omega_checked_trees::expression::ExpressionTable;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::expression::Expression;
use static_values::RuntimeStaticValues;

pub(super) use storage_copy::runtime_storage_copy;

pub(super) fn select_runtime_storage_write_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    selected_instructions: &mut SelectedInstructionSink,
) {
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::Mutation { .. } => {}
        RuntimeDispatchBodyOperationKind::StateCallResult { target_key, value, .. } => {
            mutation::select_runtime_state_call_result_write(
                input,
                dispatch_index,
                operation.source_key,
                operation.statement_index,
                *target_key,
                *value,
                aliases,
                alias_expressions,
                static_values,
                selected_instructions,
            );
            return;
        }
        _ => return,
    }
    let Some(mutation) =
        state_mutation_for_statement(input, operation.source_key, operation.statement_index)
    else {
        return;
    };

    let (source_machine, source_state) = state_names(input, mutation.source_key);
    if aliases.is_empty()
        && let Some(copy) = storage_copy::runtime_storage_copy_in_table(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.source_key,
            &input.state_storage.expressions,
            mutation.target,
            mutation.value,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_key: mutation.source_key,
            source_statement: mutation.statement_index,
        });
        return;
    }

    let target = input.state_storage.expressions.to_tree(mutation.target);
    let value = input.state_storage.expressions.to_tree(mutation.value);
    let target = expand_state_local_initializers(input, mutation.source_key, &target);
    let value = expand_state_local_initializers(input, mutation.source_key, &value);
    mutation::select_runtime_mutation_writes(
        input,
        dispatch_index,
        mutation.source_key,
        &source_machine,
        &source_state,
        mutation.statement_index,
        &target,
        &value,
        aliases,
        alias_expressions,
        static_values,
        selected_instructions,
    );
}

fn expand_state_local_initializers(
    input: &InstructionSelectionInput<'_>,
    source_key: omega_control_flow::StateKey,
    expression: &Expression,
) -> Expression {
    match expression {
        Expression::ArrayLiteral(values) => Expression::ArrayLiteral(
            values
                .iter()
                .map(|value| expand_state_local_initializers(input, source_key, value))
                .collect(),
        ),
        Expression::Binary(binary) => {
            let mut binary = (**binary).clone();
            binary.left = expand_state_local_initializers(input, source_key, &binary.left);
            binary.right = expand_state_local_initializers(input, source_key, &binary.right);
            Expression::Binary(Box::new(binary))
        }
        Expression::Cast(cast) => {
            let mut cast = (**cast).clone();
            cast.value = expand_state_local_initializers(input, source_key, &cast.value);
            Expression::Cast(Box::new(cast))
        }
        Expression::Call(call) => {
            let mut call = (**call).clone();
            call.receiver = call.receiver.as_ref().map(|receiver| {
                Box::new(expand_state_local_initializers(input, source_key, receiver))
            });
            call.arguments = call
                .arguments
                .iter()
                .map(|argument| expand_state_local_initializers(input, source_key, argument))
                .collect();
            Expression::Call(Box::new(call))
        }
        Expression::Indexed(indexed) => {
            let mut indexed = (**indexed).clone();
            indexed.collection =
                expand_state_local_initializers(input, source_key, &indexed.collection);
            indexed.index = expand_state_local_initializers(input, source_key, &indexed.index);
            Expression::Indexed(Box::new(indexed))
        }
        Expression::Member(member) => {
            let mut member = (**member).clone();
            member.receiver =
                expand_state_local_initializers(input, source_key, &member.receiver);
            Expression::Member(Box::new(member))
        }
        Expression::Mutable(inner) => Expression::Mutable(Box::new(
            expand_state_local_initializers(input, source_key, inner),
        )),
        Expression::Name(path) if !path.is_empty() => {
            let Some(initializer) = state_local_initializer(input, source_key, path) else {
                return expression.clone();
            };
            let initializer = expand_state_local_initializers(input, source_key, &initializer);
            super::super::bindings::append_place_suffix(&initializer, &path[1..])
        }
        Expression::StructLiteral(struct_literal) => {
            let mut struct_literal = struct_literal.clone();
            struct_literal.fields = struct_literal
                .fields
                .iter()
                .map(|field| omega_checked_trees::expression::StructLiteralField {
                    name: field.name.clone(),
                    value: expand_state_local_initializers(input, source_key, &field.value),
                })
                .collect();
            Expression::StructLiteral(struct_literal)
        }
        _ => expression.clone(),
    }
}

fn state_local_initializer(
    input: &InstructionSelectionInput<'_>,
    source_key: omega_control_flow::StateKey,
    path: &omega_checked_trees::expression::NamePath,
) -> Option<Expression> {
    let machine = input
        .program
        .machines
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = machine
        .states
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    let statements = input.program.statement_table.statements(state.statement_nodes);
    statements.iter().find_map(|statement| {
        let omega_checked_trees::statement::StatementNode::LocalData(local_data) = statement else {
            return None;
        };
        let matches_symbol =
            path.head_symbol().is_valid() && local_data.symbol == path.head_symbol();
        let matches_name = path
            .first()
            .is_some_and(|name| local_data.name.as_str() == name.as_str());
        (local_data.initial_value.is_valid() && (matches_symbol || matches_name))
            .then(|| input.program.expression_table.to_tree(local_data.initial_value))
    })
}

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    input.control_flow.state_names_by_key_cloned(key)
}
