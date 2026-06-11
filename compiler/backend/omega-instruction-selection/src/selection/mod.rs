use crate::InstructionSelectionInput;
use omega_abstract_operations::AbstractOperationPlan;
use omega_checked_trees::expression::ExpressionTable;
use omega_core::arena::Arena;
use omega_state_schedule::{StateScheduleContext, build_entry_state_schedule};

mod bindings;
mod host_operations;
mod instruction_sink;
mod lookups;
mod runtime_dispatch;
mod state_bodies;
mod storage_places;
mod wire_encode;

use self::bindings::RuntimeAliasBuffer;
use instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, AbstractValueOperand,
    InstructionOperand,
};
use runtime_dispatch::select_runtime_dispatch_loop_instructions;
use state_bodies::{StateBodyVisitStack, runtime_reachable_states, select_state_body_instructions};

pub fn build_instruction_plan(input: &InstructionSelectionInput<'_>) -> AbstractOperationPlan {
    let mut instruction_plan = estimated_instruction_plan(input);

    let instructions = select_entry_instructions(
        input,
        &mut instruction_plan.code.operands,
        &mut instruction_plan.code.runtime_value_operands,
        &mut instruction_plan.code.instructions,
    );

    instruction_plan
        .code
        .functions
        .insert(AbstractFunctionPlan {
            symbol: input.entry_symbol.clone(),
            source_key: input.entry_key,
            instructions,
        });

    instruction_plan
}

fn estimated_instruction_plan(input: &InstructionSelectionInput<'_>) -> AbstractOperationPlan {
    let runtime_dispatch_edges = input.runtime_dispatch_loop.edges.len();
    let runtime_body_operations = input.runtime_bodies.operations.len();
    let host_operations = input.host_calls.operations.len();
    let storage_writes = input.runtime_storage.writes.len();
    let text_writes = input.runtime_text.writes.len();
    let state_guards = input.state_guards.guards.len();

    let instruction_capacity = 2usize
        .saturating_add(runtime_body_operations.saturating_mul(4))
        .saturating_add(runtime_dispatch_edges.saturating_mul(8))
        .saturating_add(host_operations.saturating_mul(2))
        .saturating_add(storage_writes.saturating_mul(4))
        .saturating_add(text_writes.saturating_mul(4))
        .saturating_add(state_guards.saturating_mul(2));
    let operand_capacity = input
        .host_calls
        .arguments
        .len()
        .saturating_add(host_operations.saturating_mul(2));
    let runtime_value_operand_capacity = input
        .runtime_storage
        .frame_slots
        .len()
        .saturating_add(input.runtime_storage.writes.len())
        .saturating_add(input.state_guards.operands.len())
        .saturating_add(runtime_dispatch_edges.saturating_mul(2));

    AbstractOperationPlan::with_capacity(
        1,
        instruction_capacity,
        operand_capacity,
        runtime_value_operand_capacity,
    )
}

fn select_entry_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<AbstractValueOperand>,
    instructions: &mut Arena<AbstractOperation>,
) -> omega_core::arena::HandleSpan<AbstractOperation> {
    let mut selected_instructions = SelectedInstructionSink::new(instructions);

    selected_instructions.push(entry_instruction(input));

    if input.runtime_dispatch_loop.needed {
        select_runtime_dispatch_loop_instructions(
            input,
            operands,
            runtime_value_operands,
            &mut selected_instructions,
        );
        selected_instructions.push(exit_instruction(input));
        return selected_instructions.finish();
    }

    let schedule_context =
        StateScheduleContext::new(&input.control_flow, &input.host_calls, &input.state_calls);
    let _state_schedule = build_entry_state_schedule(&schedule_context, input.entry_key)
        .unwrap_or_else(|_| runtime_reachable_states(input));

    let empty_aliases = RuntimeAliasBuffer::default();
    select_state_body_instructions(
        input,
        input.entry_key,
        input
            .runtime_bodies
            .bodies
            .iter()
            .find(|(_, body)| body.key == input.entry_key)
            .map(|(_, body)| body.dispatch_index),
        &empty_aliases,
        &ExpressionTable::new(),
        operands,
        runtime_value_operands,
        &mut selected_instructions,
        &mut StateBodyVisitStack::with_capacity(input.control_flow.states.len()),
    );

    selected_instructions.push(exit_instruction(input));
    selected_instructions.finish()
}

fn entry_instruction(input: &InstructionSelectionInput<'_>) -> AbstractOperation {
    AbstractOperation {
        kind: AbstractOperationKind::EnterFunction,
        source_key: input.entry_key,
        source_statement: 0,
    }
}

fn exit_instruction(input: &InstructionSelectionInput<'_>) -> AbstractOperation {
    AbstractOperation {
        kind: AbstractOperationKind::LeaveFunction,
        source_key: input.entry_key,
        source_statement: 0,
    }
}
