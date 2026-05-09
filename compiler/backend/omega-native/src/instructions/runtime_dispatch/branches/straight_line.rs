use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation;
use crate::runtime_dispatch::branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};
use omega_typed_program::name::ProgramName;

use super::super::super::bindings::{
    resolve_leaf_binding_expression, resolve_straight_line_binding_expression,
};
use super::super::super::lookups::{
    state_call_for_statement, state_mutation_for_statement, state_operations, state_parameters,
};
use super::super::super::model::SelectedInstruction;
use super::mutation::select_runtime_resolved_mutation_write;

pub(in crate::instructions::runtime_dispatch) fn select_runtime_straight_line_branch_expansions_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_straight_line_branch_expansion(
            native_plan,
            expansion,
            selected_instructions,
        );
    }
}

fn select_runtime_straight_line_branch_expansion(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if expansion.resolved_guard != omega_typed_program::statement::TransitionGuard::Always {
        return;
    }

    select_runtime_straight_line_branch_writes(native_plan, expansion, selected_instructions);
}

fn select_runtime_straight_line_branch_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(operations) = native_plan
        .runtime_branching_calls
        .straight_line_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = native_plan
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                let resolved_target = resolve_straight_line_binding_expression(target, bindings);
                let resolved_value = resolve_straight_line_binding_expression(value, bindings);
                let (operation_machine, operation_state) =
                    state_names(native_plan, operation.source_key);
                select_runtime_resolved_mutation_write(
                    native_plan,
                    expansion.dispatch_index,
                    operation.source_key,
                    &source_machine_name(native_plan, expansion.source_key),
                    &operation_machine,
                    &operation_state,
                    operation.statement_index,
                    &resolved_target,
                    &resolved_value,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                target_key,
                lowering: crate::state_calls::StateCallLowering::InlineLeaf,
                ..
            } => select_runtime_straight_line_leaf_state_call_writes(
                native_plan,
                expansion,
                operation,
                bindings,
                *target_key,
                selected_instructions,
            ),
            _ => {}
        }
    }
}

fn state_names(native_plan: &NativePlan, key: StateKey) -> (ProgramName, ProgramName) {
    native_plan.control_flow.state_names_by_key_cloned(key)
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_leaf_state_call_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    target_key: StateKey,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(state_call) =
        state_call_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };
    let leaf_parameters = state_parameters(native_plan, target_key);
    let leaf_bindings = leaf_parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter_name)| {
            let argument = arguments.get(parameter_index)?;
            Some(RuntimeLeafBranchBinding {
                parameter_name: parameter_name.clone(),
                expression: resolve_straight_line_binding_expression(
                    &argument.expression,
                    straight_line_bindings,
                ),
                kind: RuntimeLeafBranchBindingKind::LeafParameter,
            })
        })
        .collect::<Vec<_>>();

    let Some(operations) = state_operations(native_plan, target_key) else {
        return;
    };
    let (target_machine, target_state) = state_names(native_plan, target_key);
    for leaf_operation in operations {
        let Some(mutation) =
            state_mutation_for_statement(native_plan, target_key, leaf_operation.statement_index)
        else {
            continue;
        };
        let resolved_target = resolve_leaf_binding_expression(&mutation.target, &leaf_bindings);
        let resolved_value = resolve_leaf_binding_expression(&mutation.value, &leaf_bindings);
        select_runtime_resolved_mutation_write(
            native_plan,
            expansion.dispatch_index,
            target_key,
            &source_machine_name(native_plan, expansion.source_key),
            &target_machine,
            &target_state,
            leaf_operation.statement_index,
            &resolved_target,
            &resolved_value,
            selected_instructions,
        );
    }
}

fn source_machine_name(native_plan: &NativePlan, key: StateKey) -> ProgramName {
    native_plan
        .control_flow
        .state_machine_name_by_key_cloned(key)
}
