use crate::ir::Program;
use crate::ir::expression::Expression;
use crate::ir::machine::Machine;
use crate::ir::state::State;
use crate::ir::statement::{Call, Statement};
use crate::native::target::{NativeTarget, ObjectFormat};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallPlan {
    pub calls: Arena<HostCall>,
    pub operations: Arena<LoweredHostOperation>,
    pub arguments: Arena<HostCallArgument>,
}

impl Default for HostCallPlan {
    fn default() -> Self {
        Self {
            calls: Arena::new(),
            operations: Arena::new(),
            arguments: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCall {
    pub machine: String,
    pub state: String,
    pub statement_index: usize,
    pub platform_call: String,
    pub operations: HandleSpan<LoweredHostOperation>,
    pub arguments: HandleSpan<HostCallArgument>,
}

impl Default for HostCall {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            statement_index: 0,
            platform_call: String::new(),
            operations: HandleSpan::empty(),
            arguments: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredHostOperation {
    pub capability: String,
    pub operation: String,
}

impl Default for LoweredHostOperation {
    fn default() -> Self {
        Self {
            capability: String::new(),
            operation: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallArgument {
    pub kind: HostCallArgumentKind,
}

impl Default for HostCallArgument {
    fn default() -> Self {
        Self {
            kind: HostCallArgumentKind::Expression(String::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCallArgumentKind {
    Text(String),
    Integer(i64),
    Expression(String),
}

pub fn build_host_call_plan(program: &Program, target: NativeTarget) -> HostCallPlan {
    let mut plan = HostCallPlan::default();

    for machine in &program.machines {
        collect_machine_host_calls(program, target, machine, &mut plan);
    }

    plan
}

fn collect_machine_host_calls(
    program: &Program,
    target: NativeTarget,
    machine: &Machine,
    plan: &mut HostCallPlan,
) {
    for state in &machine.states {
        collect_state_host_calls(program, target, machine, state, plan);
    }
}

fn collect_state_host_calls(
    program: &Program,
    target: NativeTarget,
    machine: &Machine,
    state: &State,
    plan: &mut HostCallPlan,
) {
    for (statement_index, statement) in state.statements.iter().enumerate() {
        let Statement::Call(call) = statement else {
            continue;
        };

        if !is_platform_call(program, machine, call) {
            continue;
        }

        let lowered_operations = lower_platform_call(target, call);
        if lowered_operations.is_empty() {
            continue;
        }

        let operations = plan.operations.insert_many(lowered_operations);
        let arguments = plan.arguments.insert_many(lower_host_call_arguments(call));
        plan.calls.insert(HostCall {
            machine: machine.name.clone(),
            state: state.name.clone(),
            statement_index,
            platform_call: platform_call_name(call),
            operations,
            arguments,
        });
    }
}

fn is_platform_call(program: &Program, machine: &Machine, call: &Call) -> bool {
    let Some(receiver) = call.receiver.as_deref() else {
        return false;
    };

    let Some(receiver_type) = machine
        .contains
        .iter()
        .find(|contained_object| contained_object.name == receiver)
        .map(|contained_object| contained_object.type_name.as_str())
    else {
        return false;
    };

    program
        .platforms
        .iter()
        .any(|platform| platform.name == receiver_type)
}

fn lower_platform_call(target: NativeTarget, call: &Call) -> Vec<LoweredHostOperation> {
    match (target.object_format, call.target.as_str()) {
        (ObjectFormat::Coff, "write_line") => vec![
            host_operation("Stdout", "get_std_handle"),
            host_operation("Stdout", "write_file"),
        ],
        (ObjectFormat::Coff, "exit_process") => vec![host_operation("Process", "exit_process")],
        (ObjectFormat::Elf, "write_line") => vec![host_operation("Stdout", "write")],
        (ObjectFormat::Elf, "exit_process") => vec![host_operation("Process", "exit_group")],
        (ObjectFormat::MachO, "write_line") => vec![host_operation("Stdout", "write")],
        (ObjectFormat::MachO, "exit_process") => vec![host_operation("Process", "exit")],
        _ => Vec::new(),
    }
}

fn host_operation(capability: &str, operation: &str) -> LoweredHostOperation {
    LoweredHostOperation {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
    }
}

fn lower_host_call_arguments(call: &Call) -> Vec<HostCallArgument> {
    call.arguments
        .iter()
        .map(|argument| HostCallArgument {
            kind: lower_host_call_argument(argument),
        })
        .collect()
}

fn lower_host_call_argument(argument: &Expression) -> HostCallArgumentKind {
    match argument {
        Expression::String(value) => HostCallArgumentKind::Text(value.clone()),
        Expression::Integer(value) => HostCallArgumentKind::Integer(*value),
        _ => HostCallArgumentKind::Expression(argument.display_name()),
    }
}

fn platform_call_name(call: &Call) -> String {
    match call.receiver.as_deref() {
        Some(receiver) => format!("{receiver}.{}", call.target),
        None => call.target.clone(),
    }
}
