use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::expression::Expression;
use crate::ir::machine::Machine;
use crate::ir::state::State;
use crate::ir::statement::{Call, Statement};
use crate::native::abi::{HostAbiPlan, PlatformCallLowering};
use crate::native::target::NativeTarget;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallPlan {
    pub calls: Arena<HostCall>,
    pub unsupported_calls: Arena<UnsupportedHostCall>,
    pub operations: Arena<LoweredHostOperation>,
    pub arguments: Arena<HostCallArgument>,
}

impl Default for HostCallPlan {
    fn default() -> Self {
        Self {
            calls: Arena::new(),
            unsupported_calls: Arena::new(),
            operations: Arena::new(),
            arguments: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsupportedHostCall {
    pub machine: String,
    pub state: String,
    pub statement_index: usize,
    pub platform_call: String,
    pub reason: String,
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

pub fn build_host_call_plan(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
) -> Result<HostCallPlan, Diagnostic> {
    let mut plan = HostCallPlan::default();

    for machine in &program.machines {
        collect_machine_host_calls(program, target, host_abi, machine, &mut plan)?;
    }

    Ok(plan)
}

fn collect_machine_host_calls(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    for state in &machine.states {
        collect_state_host_calls(program, target, host_abi, machine, state, plan)?;
    }

    Ok(())
}

fn collect_state_host_calls(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    for (statement_index, statement) in state.statements.iter().enumerate() {
        let Statement::Call(call) = statement else {
            continue;
        };

        let Some(platform_name) = platform_call_receiver_type(program, machine, call) else {
            continue;
        };

        let lowered_operations = lower_platform_call(host_abi, &platform_name, call);
        if lowered_operations.is_empty() {
            let platform_call = platform_call_name(call);
            plan.unsupported_calls.insert(UnsupportedHostCall {
                machine: machine.name.clone(),
                state: state.name.clone(),
                statement_index,
                platform_call: platform_call.clone(),
                reason: format!("no native lowering for target {target:?}"),
            });
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

    Ok(())
}

fn platform_call_receiver_type(
    program: &Program,
    machine: &Machine,
    call: &Call,
) -> Option<String> {
    let Some(receiver) = call.receiver.as_deref() else {
        return None;
    };

    let Some(receiver_type) = machine
        .contains
        .iter()
        .find(|contained_object| contained_object.name == receiver)
        .map(|contained_object| contained_object.type_name.as_str())
    else {
        return None;
    };

    if program
        .platforms
        .iter()
        .any(|platform| platform.name == receiver_type)
    {
        Some(receiver_type.to_owned())
    } else {
        None
    }
}

fn lower_platform_call(
    host_abi: &HostAbiPlan,
    platform_name: &str,
    call: &Call,
) -> Vec<LoweredHostOperation> {
    host_abi
        .platform_call_lowerings
        .iter()
        .find(|(_, lowering)| lowering_matches(lowering, platform_name, &call.target))
        .map(|(_, lowering)| {
            lowering
                .operations
                .iter()
                .map(|operation| host_operation(&operation.capability, &operation.operation))
                .collect()
        })
        .unwrap_or_default()
}

fn lowering_matches(
    lowering: &PlatformCallLowering,
    platform_name: &str,
    state_name: &str,
) -> bool {
    lowering.platform == platform_name && lowering.state == state_name
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
