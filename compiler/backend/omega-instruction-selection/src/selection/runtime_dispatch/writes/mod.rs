mod mutation;
mod static_values;
mod storage_copy;

use super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
};
use super::super::lookups::state_mutation_for_statement;
use super::text_writes::runtime_text_builder_write_in_table_emit;
use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{RuntimeValueOperand, SelectedInstruction};
use omega_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableMemberExpression,
};
use omega_checked_trees::name::Identifier;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
pub(crate) use static_values::RuntimeStaticValues;

pub(in crate::selection) use mutation::{
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
};
pub(super) use storage_copy::{
    runtime_storage_copy, runtime_storage_copy_in_table, runtime_storage_indirect_copy_in_table,
};

#[derive(Default)]
pub(crate) struct RuntimeStorageWriteScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
}

impl RuntimeStorageWriteScratch {
    pub(crate) fn clear(&mut self) {
        self.expressions.clear();
        self.mutable_expressions.clear();
        self.resolved_segment_expressions.clear();
    }
}

pub(super) fn select_runtime_storage_write_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    scratch: &mut RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::Mutation { .. } => {}
        RuntimeDispatchBodyOperationKind::StateCallResult {
            role,
            call_ordinal,
            target_key,
            value,
            ..
        } => {
            mutation::select_runtime_state_call_result_write(
                input,
                dispatch_index,
                operation.source_key,
                operation.statement_index,
                *role,
                *call_ordinal,
                *target_key,
                *value,
                aliases,
                alias_expressions,
                static_values,
                scratch,
                runtime_value_operands,
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

    if aliases.is_empty() {
        if select_runtime_storage_mutation_write_in_table_with_scratch(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.statement_index,
            mutation.target,
            mutation.value,
            static_values,
            scratch,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
    } else {
        scratch.expressions.clear();
        let expressions = &mut scratch.expressions;
        let copied_aliases =
            RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases, expressions);
        let target = expressions.copy_from(&input.state_storage.expressions, mutation.target);
        let value = expressions.copy_from(&input.state_storage.expressions, mutation.value);
        let resolved_target = resolve_runtime_alias_binding_handle(
            target,
            mutation.source_key,
            copied_aliases.bindings(),
            expressions,
        );
        let resolved_value = resolve_runtime_alias_binding_handle(
            value,
            mutation.source_key,
            copied_aliases.bindings(),
            expressions,
        );
        if select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
            input,
            dispatch_index,
            mutation.source_key,
            resolved_target.source_key,
            resolved_value.source_key,
            mutation.statement_index,
            &expressions,
            resolved_target.expression,
            resolved_value.expression,
            copied_aliases.bindings(),
            static_values,
            &mut scratch.mutable_expressions,
            &mut scratch.resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
    }

    let (source_machine, source_state) = state_names(input, mutation.source_key);
    let target = input.state_storage.expressions.to_tree(mutation.target);
    let value = input.state_storage.expressions.to_tree(mutation.value);
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
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn select_runtime_storage_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
    scratch: &mut RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        source_key,
        source_key,
        statement_index,
        &input.state_storage.expressions,
        target,
        value,
        &[],
        static_values,
        &mut scratch.mutable_expressions,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut RuntimeStaticValues,
    mutable_expressions: &mut ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if matches!(
        expressions.expression(value),
        ExpressionNode::StructLiteral(_)
    ) {
        let source_expressions = expressions;
        mutable_expressions.clear();
        let expressions = mutable_expressions;
        let copied_aliases =
            RuntimeAliasBuffer::copy_from_bindings(source_expressions, aliases, expressions);
        let target = expressions.copy_from(source_expressions, target);
        let value = expressions.copy_from(source_expressions, value);
        return select_runtime_storage_resolved_mutation_write_in_mutable_table(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            target,
            value,
            copied_aliases.bindings(),
            resolved_segment_expressions,
            static_values,
            runtime_value_operands,
            selected_instructions,
        );
    }

    select_runtime_storage_resolved_scalar_mutation_write_in_table(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        aliases,
        resolved_segment_expressions,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_resolved_mutation_write_in_mutable_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &mut ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if let ExpressionNode::StructLiteral(struct_literal) = expressions.expression(value).clone() {
        for offset in 0..struct_literal.fields.count() {
            let field = expressions
                .struct_field_at_offset(struct_literal.fields, offset)
                .clone();
            let field_target = expressions.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: target,
                member_symbol: SymbolHandle::invalid(),
                member: field.name,
            }));
            select_runtime_storage_resolved_mutation_write_in_mutable_table(
                input,
                dispatch_index,
                operation_source_key,
                target_source_key,
                value_source_key,
                statement_index,
                expressions,
                field_target,
                field.value,
                aliases,
                resolved_segment_expressions,
                static_values,
                runtime_value_operands,
                selected_instructions,
            );
        }
        return true;
    }

    select_runtime_storage_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        aliases,
        resolved_segment_expressions,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_resolved_scalar_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    select_runtime_storage_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        aliases,
        resolved_segment_expressions,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_resolved_scalar_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if let Some(kind) = mutation::select_runtime_static_mutation_write_in_table(
        input,
        dispatch_index,
        target_source_key,
        statement_index,
        expressions,
        target,
        value,
        static_values,
    )
    .or_else(|| {
        storage_copy::runtime_storage_indexed_source_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        storage_copy::runtime_storage_fixed_indexed_source_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        storage_copy::runtime_storage_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        storage_copy::runtime_storage_indirect_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        mutation::select_runtime_string_mutation_write_in_table(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            statement_index,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        mutation::select_runtime_binary_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            target,
            value,
            static_values,
            runtime_value_operands,
        )
    }) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    resolved_segment_expressions.clear();
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(expressions, aliases, resolved_segment_expressions);
    if runtime_text_builder_write_in_table_emit(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        statement_index,
        expressions,
        target,
        resolved_segment_expressions,
        &|expressions, expression| {
            resolve_runtime_alias_binding_handle(
                expression,
                operation_source_key,
                copied_aliases.bindings(),
                expressions,
            )
            .expression
        },
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        },
    ) {
        return true;
    }

    false
}

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (Identifier, Identifier) {
    input.control_flow.state_names_by_key_cloned(key)
}
