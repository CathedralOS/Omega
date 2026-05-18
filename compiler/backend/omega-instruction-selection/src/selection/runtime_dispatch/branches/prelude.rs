use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, RuntimeAliasResolutionContext,
    resolve_branch_prelude_binding_expression_handle,
};
use crate::selection::host_operations::select_host_call;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_checked_trees::expression::ExpressionTable;
use omega_checked_trees::name::ProgramName;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{
    RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion, RuntimeBranchPreludeOperationKind,
};
use omega_target_operations::{InstructionOperand, RuntimeValueOperand};

use super::super::super::lookups::host_call_for_statement;
use super::super::text_writes::runtime_text_builder_write_in_table_emit;
use super::mutation::{
    select_runtime_resolved_mutation_write,
    select_runtime_resolved_mutation_write_in_table_with_scratch,
};

#[derive(Default)]
struct BranchPreludeSelectionScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_branch_preludes_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut scratch = BranchPreludeSelectionScratch::default();

    for (_, expansion) in input
        .runtime_branching_calls
        .prelude_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_branch_prelude(
            input,
            expansion,
            &mut scratch,
            operands,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

fn select_runtime_branch_prelude(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeBranchPreludeExpansion,
    scratch: &mut BranchPreludeSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(operations) = input
        .runtime_branching_calls
        .prelude_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .prelude_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    let alias_bindings = prelude_alias_bindings(expansion.target_key, bindings);
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();

    for operation in operations {
        match &operation.kind {
            RuntimeBranchPreludeOperationKind::HostCall => {
                let Some(host_call) =
                    host_call_for_statement(input, operation.source_key, operation.statement_index)
                else {
                    continue;
                };
                select_host_call(
                    input,
                    host_call,
                    Some(expansion.dispatch_index),
                    Some(RuntimeAliasResolutionContext {
                        aliases: alias_bindings.bindings(),
                        alias_expressions: &input.runtime_branching_calls.expressions,
                    }),
                    operands,
                    selected_instructions,
                );
            }
            RuntimeBranchPreludeOperationKind::Mutation { target, value, .. } => {
                scratch.expressions.clear();
                let expressions = &mut scratch.expressions;
                let target =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *target);
                let value =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *value);
                let resolved_target = resolve_branch_prelude_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    target,
                    bindings,
                );
                let resolved_value = resolve_branch_prelude_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    value,
                    bindings,
                );
                if select_runtime_resolved_mutation_write_in_table_with_scratch(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.source_key,
                    operation.source_key,
                    operation.statement_index,
                    &expressions,
                    resolved_target,
                    resolved_value,
                    &mut scratch.mutable_expressions,
                    &mut scratch.resolved_segment_expressions,
                    runtime_value_operands,
                    selected_instructions,
                ) {
                    continue;
                }
                scratch.resolved_segment_expressions.clear();
                if runtime_text_builder_write_in_table_emit(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.source_key,
                    operation.statement_index,
                    &expressions,
                    resolved_target,
                    &mut scratch.resolved_segment_expressions,
                    &|expressions, expression| {
                        resolve_branch_prelude_binding_expression_handle(
                            &input.runtime_branching_calls.expressions,
                            expressions,
                            expression,
                            bindings,
                        )
                    },
                    &mut |kind| {
                        selected_instructions.push(omega_target_operations::SelectedInstruction {
                            kind,
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    },
                ) {
                    continue;
                }
                let resolved_target = expressions.to_tree(resolved_target);
                let resolved_value = expressions.to_tree(resolved_value);
                let (operation_machine, operation_state) = state_names(input, operation.source_key);
                select_runtime_resolved_mutation_write(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    &source_machine_name(input, expansion.source_key),
                    &operation_machine,
                    &operation_state,
                    operation.statement_index,
                    &resolved_target,
                    &resolved_value,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            RuntimeBranchPreludeOperationKind::StateCall { .. }
            | RuntimeBranchPreludeOperationKind::LocalData
            | RuntimeBranchPreludeOperationKind::Other => {}
        }
    }
}

fn prelude_alias_bindings(
    target_key: omega_control_flow::StateKey,
    bindings: &[RuntimeBranchPreludeBinding],
) -> RuntimeAliasBuffer {
    RuntimeAliasBuffer::from_iter(bindings.iter().map(|binding| RuntimeAliasBinding {
        source_key: target_key,
        parameter_symbol: binding.parameter_symbol,
        parameter_name: binding.parameter_name.clone(),
        expression_source_key: target_key,
        expression: binding.expression,
    }))
}

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    input.control_flow.state_names_by_key_cloned(key)
}

fn source_machine_name(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> ProgramName {
    input.control_flow.state_machine_name_by_key_cloned(key)
}
