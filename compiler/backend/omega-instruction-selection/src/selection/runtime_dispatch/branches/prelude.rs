use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, RuntimeAliasResolutionContext,
    resolve_branch_prelude_binding_expression,
};
use crate::selection::host_operations::select_host_call;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_checked_trees::name::ProgramName;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{
    RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion, RuntimeBranchPreludeOperationKind,
};
use omega_target_operations::InstructionOperand;

use super::super::super::lookups::host_call_for_statement;
use super::mutation::select_runtime_resolved_mutation_write;

pub(in crate::selection::runtime_dispatch) fn select_runtime_branch_preludes_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
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
        select_runtime_branch_prelude(input, expansion, operands, selected_instructions);
    }
}

fn select_runtime_branch_prelude(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeBranchPreludeExpansion,
    operands: &mut Arena<InstructionOperand>,
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

    for operation in operations {
        match &operation.kind {
            RuntimeBranchPreludeOperationKind::HostCall { .. } => {
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
                let target = input.runtime_branching_calls.expressions.to_tree(*target);
                let value = input.runtime_branching_calls.expressions.to_tree(*value);
                let resolved_target = resolve_branch_prelude_binding_expression(
                    &input.runtime_branching_calls.expressions,
                    &target,
                    bindings,
                );
                let resolved_value = resolve_branch_prelude_binding_expression(
                    &input.runtime_branching_calls.expressions,
                    &value,
                    bindings,
                );
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
