use crate::native::host_calls::HostCall;
use crate::native::plan::NativePlan;
use crate::native::target::NativeTarget;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<FunctionInstructionPlan>,
    pub instructions: Arena<SelectedInstruction>,
}

impl Default for InstructionPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionInstructionPlan {
    pub symbol: String,
    pub machine: String,
    pub state: String,
    pub instructions: HandleSpan<SelectedInstruction>,
}

impl Default for FunctionInstructionPlan {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            machine: String::new(),
            state: String::new(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub kind: SelectedInstructionKind,
    pub source_machine: String,
    pub source_state: String,
    pub source_statement: usize,
}

impl Default for SelectedInstruction {
    fn default() -> Self {
        Self {
            kind: SelectedInstructionKind::EnterFunction,
            source_machine: String::new(),
            source_state: String::new(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    EnterFunction,
    BeginPlatformCall {
        platform_call: String,
    },
    HostOperation {
        capability: String,
        operation: String,
    },
    LeaveFunction,
}

pub fn build_instruction_plan(native_plan: &NativePlan) -> InstructionPlan {
    let mut instruction_plan = InstructionPlan {
        target: native_plan.target,
        functions: Arena::new(),
        instructions: Arena::new(),
    };

    let entry_instructions = select_entry_instructions(native_plan);
    let instructions = instruction_plan
        .instructions
        .insert_many(entry_instructions);

    instruction_plan.functions.insert(FunctionInstructionPlan {
        symbol: native_plan.object.entry_symbol.clone(),
        machine: native_plan.entry_machine.clone(),
        state: native_plan.entry_state.clone(),
        instructions,
    });

    instruction_plan
}

fn select_entry_instructions(native_plan: &NativePlan) -> Vec<SelectedInstruction> {
    let mut selected_instructions = Vec::new();

    selected_instructions.push(entry_instruction(native_plan));

    for (_, host_call) in native_plan.host_calls.calls.iter() {
        if host_call.machine != native_plan.entry_machine
            || host_call.state != native_plan.entry_state
        {
            continue;
        }

        select_host_call(native_plan, host_call, &mut selected_instructions);
    }

    selected_instructions.push(exit_instruction(native_plan));
    selected_instructions
}

fn select_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::BeginPlatformCall {
            platform_call: host_call.platform_call.clone(),
        },
        source_machine: host_call.machine.clone(),
        source_state: host_call.state.clone(),
        source_statement: host_call.statement_index,
    });

    let Some(operations) = native_plan.host_calls.operations.span(host_call.operations) else {
        return;
    };

    for operation in operations {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                capability: operation.capability.clone(),
                operation: operation.operation.clone(),
            },
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
            source_statement: host_call.statement_index,
        });
    }
}

fn entry_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::EnterFunction,
        source_machine: native_plan.entry_machine.clone(),
        source_state: native_plan.entry_state.clone(),
        source_statement: 0,
    }
}

fn exit_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::LeaveFunction,
        source_machine: native_plan.entry_machine.clone(),
        source_state: native_plan.entry_state.clone(),
        source_statement: 0,
    }
}
