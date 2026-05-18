use crate::InstructionSelectionInput;
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

use self::bindings::RuntimeAliasBuffer;
use instruction_sink::SelectedInstructionSink;
use omega_target_operations::{
    FunctionInstructionPlan, InstructionOperand, InstructionPlan, RuntimeValueOperand,
    SelectedInstruction, SelectedInstructionKind,
};
use runtime_dispatch::select_runtime_dispatch_loop_instructions;
use state_bodies::{StateBodyVisitStack, runtime_reachable_states, select_state_body_instructions};

pub fn build_instruction_plan(input: &InstructionSelectionInput<'_>) -> InstructionPlan {
    let mut instruction_plan = InstructionPlan {
        target: input.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        operands: Arena::new(),
        runtime_value_operands: Arena::new(),
    };

    let instructions = select_entry_instructions(
        input,
        &mut instruction_plan.operands,
        &mut instruction_plan.runtime_value_operands,
        &mut instruction_plan.instructions,
    );

    instruction_plan.functions.insert(FunctionInstructionPlan {
        symbol: input.entry_symbol.clone(),
        source_key: input.entry_key,
        instructions,
    });

    instruction_plan
}

fn select_entry_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    instructions: &mut Arena<SelectedInstruction>,
) -> omega_core::arena::HandleSpan<SelectedInstruction> {
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
        &mut StateBodyVisitStack::new(),
    );

    selected_instructions.push(exit_instruction(input));
    selected_instructions.finish()
}

fn entry_instruction(input: &InstructionSelectionInput<'_>) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::EnterFunction,
        source_key: input.entry_key,
        source_statement: 0,
    }
}

fn exit_instruction(input: &InstructionSelectionInput<'_>) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::LeaveFunction,
        source_key: input.entry_key,
        source_statement: 0,
    }
}
