use crate::native::control_flow::{OperationKind, StateFlow};
use crate::native::object_formats::can_emit_target_object;
use crate::native::plan::NativePlan;
use crate::native::target::ObjectFormat;
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionPlan {
    pub object_format: ObjectFormat,
    pub entry_symbol: String,
    pub sections: usize,
    pub symbols: usize,
    pub host_bindings: usize,
    pub host_calls: usize,
    pub data_bytes: usize,
    pub selected_instructions: usize,
    pub instruction_operands: usize,
    pub machine_code_bytes: usize,
    pub encoded_machine_bytes: usize,
    pub relocations: usize,
    pub blockers: Arena<EmissionBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmissionBlocker {
    pub stage: String,
    pub reason: String,
}

pub fn build_emission_plan(native_plan: &NativePlan) -> EmissionPlan {
    let mut blockers = Arena::new();

    if native_plan.machine_code.bytes.len() < native_plan.machine_code.byte_count {
        blockers.insert(blocker(
            "machine encoding",
            "not all selected native instructions are encoded into target bytes yet",
        ));
    }

    for (_, unsupported_call) in native_plan.host_calls.unsupported_calls.iter() {
        blockers.insert(blocker(
            "host lowering",
            &format!(
                "{}.{} statement {} platform call `{}`: {}",
                unsupported_call.machine,
                unsupported_call.state,
                unsupported_call.statement_index,
                unsupported_call.platform_call,
                unsupported_call.reason
            ),
        ));
    }

    for (_, host_call) in native_plan.host_calls.calls.iter() {
        if host_call.machine != native_plan.entry_machine
            || host_call.state != native_plan.entry_state
        {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "{}.{} statement {} platform call `{}` is outside compiled entry state {}.{}",
                    host_call.machine,
                    host_call.state,
                    host_call.statement_index,
                    host_call.platform_call,
                    native_plan.entry_machine,
                    native_plan.entry_state
                ),
            ));
        }
    }

    collect_entry_state_codegen_blockers(native_plan, &mut blockers);

    if !can_emit_real_object(native_plan) {
        blockers.insert_many([
            blocker(
                "relocation encoding",
                "planned relocation records are not serialized into this target object format yet",
            ),
            blocker(
                "object writer",
                "this target still falls back to the Omega native object container",
            ),
        ]);
    }

    EmissionPlan {
        object_format: native_plan.target.object_format,
        entry_symbol: native_plan.object.entry_symbol.clone(),
        sections: native_plan.object.sections.len(),
        symbols: native_plan.object.symbols.len(),
        host_bindings: native_plan.host_abi.bindings.len(),
        host_calls: native_plan.host_calls.calls.len(),
        data_bytes: native_plan.data.bytes.len(),
        selected_instructions: native_plan.instructions.instructions.len(),
        instruction_operands: native_plan.instructions.operands.len(),
        machine_code_bytes: native_plan.machine_code.byte_count,
        encoded_machine_bytes: native_plan.machine_code.bytes.len(),
        relocations: native_plan.relocations.records.len(),
        blockers,
    }
}

fn collect_entry_state_codegen_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let Some(entry_state) = entry_state_flow(native_plan) else {
        blockers.insert(blocker(
            "state codegen",
            &format!(
                "entry state {}.{} was not present in the control-flow plan",
                native_plan.entry_machine, native_plan.entry_state
            ),
        ));
        return;
    };

    if let Some(transitions) = native_plan
        .control_flow
        .transitions
        .span(entry_state.transitions)
    {
        if !transitions.is_empty() {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "{}.{} has {} transition(s); native emission currently supports straight-line entry states only",
                    native_plan.entry_machine,
                    native_plan.entry_state,
                    transitions.len()
                ),
            ));
        }
    }

    let Some(operations) = native_plan
        .control_flow
        .operations
        .span(entry_state.operations)
    else {
        blockers.insert(blocker(
            "state codegen",
            &format!(
                "{}.{} has an invalid operation span",
                native_plan.entry_machine, native_plan.entry_state
            ),
        ));
        return;
    };

    for operation in operations {
        match operation.kind {
            OperationKind::Call
                if entry_statement_has_host_call(native_plan, operation.statement_index) => {}
            OperationKind::Call => {
                blockers.insert(blocker(
                    "state codegen",
                    &format!(
                        "{}.{} statement {} is a call that is not lowered to a native host operation",
                        native_plan.entry_machine,
                        native_plan.entry_state,
                        operation.statement_index
                    ),
                ));
            }
            _ => {
                blockers.insert(blocker(
                    "state codegen",
                    &format!(
                        "{}.{} statement {} {:?} is not supported by native emission yet",
                        native_plan.entry_machine,
                        native_plan.entry_state,
                        operation.statement_index,
                        operation.kind
                    ),
                ));
            }
        };
    }
}

fn entry_state_flow(native_plan: &NativePlan) -> Option<&StateFlow> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == native_plan.entry_machine)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| {
            states
                .iter()
                .find(|state| state.name == native_plan.entry_state)
        })
}

fn entry_statement_has_host_call(native_plan: &NativePlan, statement_index: usize) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.machine == native_plan.entry_machine
            && host_call.state == native_plan.entry_state
            && host_call.statement_index == statement_index
    })
}

fn can_emit_real_object(native_plan: &NativePlan) -> bool {
    can_emit_target_object(native_plan.target)
        && native_plan.machine_code.bytes.len() == native_plan.machine_code.byte_count
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    EmissionBlocker {
        stage: stage.to_owned(),
        reason: reason.to_owned(),
    }
}
