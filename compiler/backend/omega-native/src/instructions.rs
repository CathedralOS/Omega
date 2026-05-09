use crate::plan::NativePlan;
use omega_core::arena::Arena;
use omega_state_schedule::{
    StateScheduleContext, build_entry_state_schedule, scheduled_state_flow,
};

mod bindings;
mod host_operations;
mod lookups;
mod runtime_dispatch;
mod state_bodies;
mod storage_places;

use omega_target_program::{
    FunctionInstructionPlan, InstructionOperand, InstructionPlan, SelectedInstruction,
    SelectedInstructionKind,
};
use runtime_dispatch::select_runtime_dispatch_loop_instructions;
use state_bodies::{
    runtime_reachable_states, select_state_body_instructions, select_state_host_calls,
};

pub fn build_instruction_plan(native_plan: &NativePlan) -> InstructionPlan {
    let mut instruction_plan = InstructionPlan {
        target: native_plan.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        operands: Arena::new(),
    };

    let entry_instructions = select_entry_instructions(native_plan, &mut instruction_plan.operands);
    let instructions = instruction_plan
        .instructions
        .insert_many(entry_instructions);

    instruction_plan.functions.insert(FunctionInstructionPlan {
        symbol: native_plan.object.entry_symbol.clone(),
        source_key: native_plan.entry_key,
        instructions,
    });

    instruction_plan
}

fn select_entry_instructions(
    native_plan: &NativePlan,
    operands: &mut Arena<InstructionOperand>,
) -> Vec<SelectedInstruction> {
    let mut selected_instructions = Vec::new();
    let schedule_context =
        StateScheduleContext::new(&native_plan.control_flow, &native_plan.host_calls);
    let state_schedule_result =
        build_entry_state_schedule(&schedule_context, native_plan.entry_key);
    let can_inline_state_calls = state_schedule_result.is_ok()
        && native_plan
            .state_calls
            .calls
            .iter()
            .any(|(_, call)| call.required);
    let state_schedule =
        state_schedule_result.unwrap_or_else(|_| runtime_reachable_states(native_plan));

    selected_instructions.push(entry_instruction(native_plan));

    if native_plan.runtime_dispatch_loop.needed {
        select_runtime_dispatch_loop_instructions(
            native_plan,
            operands,
            &mut selected_instructions,
        );
    } else if can_inline_state_calls {
        select_state_body_instructions(
            native_plan,
            native_plan.entry_key,
            operands,
            &mut selected_instructions,
            &mut Vec::new(),
        );
    } else {
        for scheduled_state in &state_schedule {
            if let Some(state_flow) = scheduled_state_flow(&schedule_context, scheduled_state) {
                select_state_host_calls(
                    native_plan,
                    state_flow.key,
                    operands,
                    &mut selected_instructions,
                );
            }
        }
    }

    selected_instructions.push(exit_instruction(native_plan));
    selected_instructions
}

fn entry_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::EnterFunction,
        source_key: native_plan.entry_key,
        source_statement: 0,
    }
}

fn exit_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::LeaveFunction,
        source_key: native_plan.entry_key,
        source_statement: 0,
    }
}
