use crate::abi::{HostAbiPlan, PlatformCallData, PlatformCallLowering};
use crate::target::NativeTarget;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::state::State;
use omega_typed_program::statement::{Call, Statement};
use std::sync::Arc;

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
    pub data: PlatformCallData,
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
            data: PlatformCallData::None,
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
            kind: HostCallArgumentKind::Expression(Expression::Integer(0)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCallArgumentKind {
    Text(String),
    Integer(i64),
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StaticValue {
    Integer(i64),
    Expression(Expression),
    Text(String),
}

pub fn build_host_call_plan(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
) -> Result<HostCallPlan, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_host_call_plan_with_workers(
        Arc::new(program.clone()),
        target,
        Arc::new(host_abi.clone()),
        workers.handle(),
    )
}

pub fn build_host_call_plan_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    host_abi: Arc<HostAbiPlan>,
    workers: WorkerPoolHandle,
) -> Result<HostCallPlan, Diagnostic> {
    if program.machines.is_empty() {
        return Ok(HostCallPlan::default());
    }

    let machine_count = program.machines.len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("host-call worker index should be in range");
        let mut machine_plan = HostCallPlan::default();

        collect_machine_host_calls(&program, target, &host_abi, machine, &mut machine_plan)
            .map(|_| machine_plan)
    });

    let mut plan = HostCallPlan::default();

    for machine_plan in machine_plans {
        merge_host_call_plan(&mut plan, machine_plan?);
    }

    Ok(plan)
}

fn merge_host_call_plan(target: &mut HostCallPlan, source: HostCallPlan) {
    for (_, unsupported_call) in source.unsupported_calls.iter() {
        target.unsupported_calls.insert(unsupported_call.clone());
    }

    for (_, call) in source.calls.iter() {
        let operations = target.operations.insert_many(
            source
                .operations
                .span_or_empty(call.operations)
                .iter()
                .cloned(),
        );
        let arguments = target.arguments.insert_many(
            source
                .arguments
                .span_or_empty(call.arguments)
                .iter()
                .cloned(),
        );

        target.calls.insert(HostCall {
            operations,
            arguments,
            ..call.clone()
        });
    }
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
    let mut static_values = initial_static_values(machine);

    for (statement_index, statement) in state.statements.iter().enumerate() {
        match statement {
            Statement::Assignment(assignment) => {
                apply_static_assignment(&mut static_values, &assignment.target, &assignment.value);
                continue;
            }
            Statement::Call(call) => {
                collect_call_host_lowering(
                    program,
                    target,
                    host_abi,
                    machine,
                    state,
                    statement_index,
                    call,
                    &static_values,
                    plan,
                )?;
                apply_call_static_effects(&mut static_values, call);
            }
            _ => {}
        }
    }

    Ok(())
}

fn collect_call_host_lowering(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    call: &Call,
    static_values: &[(String, StaticValue)],
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    let Some(platform_name) = platform_call_receiver_type(program, machine, call) else {
        return Ok(());
    };

    let Some(lowering) = find_platform_call_lowering(host_abi, &platform_name, call) else {
        let platform_call = platform_call_name(call);
        plan.unsupported_calls.insert(UnsupportedHostCall {
            machine: machine.name.clone(),
            state: state.name.clone(),
            statement_index,
            platform_call: platform_call.clone(),
            reason: format!("no native lowering for target {target:?}"),
        });
        return Ok(());
    };

    let operations = host_abi
        .host_operations
        .span(lowering.operations)
        .map(|operations| {
            plan.operations.insert_many(
                operations
                    .iter()
                    .map(|operation| host_operation(&operation.capability, &operation.operation)),
            )
        })
        .unwrap_or_else(HandleSpan::empty);
    let arguments = plan
        .arguments
        .insert_many(lower_host_call_arguments(call, static_values));
    plan.calls.insert(HostCall {
        machine: machine.name.clone(),
        state: state.name.clone(),
        statement_index,
        platform_call: platform_call_name(call),
        data: lowering.data,
        operations,
        arguments,
    });
    Ok(())
}

fn initial_static_values(machine: &Machine) -> Vec<(String, StaticValue)> {
    machine
        .owned_data
        .iter()
        .filter_map(|owned_data| {
            let value = match owned_data.initial_value.as_ref()? {
                Expression::Integer(value) => StaticValue::Integer(*value),
                Expression::String(value) => StaticValue::Text(value.clone()),
                Expression::Name(path) if is_static_symbol_path(path) => {
                    StaticValue::Expression(Expression::Name(path.clone()))
                }
                _ => return None,
            };

            Some((owned_data.name.clone(), value))
        })
        .collect()
}

fn apply_static_assignment(
    static_values: &mut Vec<(String, StaticValue)>,
    target: &Expression,
    value: &Expression,
) {
    let Some(target_name) = static_place_name(target) else {
        return;
    };

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            if let Some(field_value) = resolve_static_value(&field.value, static_values) {
                set_static_value(
                    static_values,
                    format!("{target_name}::{}", field.name),
                    field_value,
                );
            }
        }
        return;
    }

    if let Some(source_name) = static_place_name(value) {
        copy_static_prefix(static_values, &source_name, &target_name);
    }

    let Some(value) = resolve_static_value(value, static_values) else {
        return;
    };

    set_static_value(static_values, target_name, value);
}

fn static_place_name(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Name(path) if !path.is_empty() => Some(expression.display_name()),
        Expression::Indexed(_) => Some(expression.display_name()),
        _ => None,
    }
}

fn resolve_static_value(
    expression: &Expression,
    static_values: &[(String, StaticValue)],
) -> Option<StaticValue> {
    match expression {
        Expression::Integer(value) => Some(StaticValue::Integer(*value)),
        Expression::String(value) => Some(StaticValue::Text(value.clone())),
        Expression::Name(path) => {
            let name = expression.display_name();
            static_values
                .iter()
                .find(|(target, _)| target == &name)
                .map(|(_, value)| value.clone())
                .or_else(|| {
                    if is_static_symbol_path(path) {
                        Some(StaticValue::Expression(Expression::Name(path.clone())))
                    } else {
                        None
                    }
                })
        }
        _ => None,
    }
}

fn is_static_symbol_path(path: &[String]) -> bool {
    path.first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}

fn set_static_value(
    static_values: &mut Vec<(String, StaticValue)>,
    target_name: String,
    value: StaticValue,
) {
    if let Some((_, existing_value)) = static_values
        .iter_mut()
        .find(|(existing_name, _)| existing_name == &target_name)
    {
        *existing_value = value;
    } else {
        static_values.push((target_name, value));
    }
}

fn copy_static_prefix(
    static_values: &mut Vec<(String, StaticValue)>,
    source_name: &str,
    target_name: &str,
) {
    let source_prefix = format!("{source_name}::");
    let copied_values = static_values
        .iter()
        .filter_map(|(existing_name, value)| {
            existing_name
                .strip_prefix(&source_prefix)
                .map(|suffix| (format!("{target_name}::{suffix}"), value.clone()))
        })
        .collect::<Vec<_>>();

    for (copied_name, copied_value) in copied_values {
        set_static_value(static_values, copied_name, copied_value);
    }
}

fn apply_call_static_effects(static_values: &mut Vec<(String, StaticValue)>, call: &Call) {
    for argument in &call.arguments {
        let Expression::Mutable(target) = argument else {
            continue;
        };

        let Some(target_name) = static_place_name(target) else {
            continue;
        };

        invalidate_static_prefix(static_values, &target_name);
    }
}

fn invalidate_static_prefix(static_values: &mut Vec<(String, StaticValue)>, target_name: &str) {
    let target_prefix = format!("{target_name}::");
    static_values.retain(|(existing_name, _)| {
        existing_name != target_name && !existing_name.starts_with(&target_prefix)
    });
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

fn find_platform_call_lowering<'abi>(
    host_abi: &'abi HostAbiPlan,
    platform_name: &str,
    call: &Call,
) -> Option<&'abi PlatformCallLowering> {
    host_abi
        .platform_call_lowerings
        .iter()
        .find(|(_, lowering)| lowering_matches(lowering, platform_name, &call.target))
        .map(|(_, lowering)| lowering)
}

fn lowering_matches(
    lowering: &PlatformCallLowering,
    platform_name: &str,
    state_name: &str,
) -> bool {
    (lowering.platform == "*" || lowering.platform == platform_name) && lowering.state == state_name
}

fn host_operation(capability: &str, operation: &str) -> LoweredHostOperation {
    LoweredHostOperation {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
    }
}

fn lower_host_call_arguments(
    call: &Call,
    static_values: &[(String, StaticValue)],
) -> Vec<HostCallArgument> {
    call.arguments
        .iter()
        .map(|argument| HostCallArgument {
            kind: lower_host_call_argument(argument, static_values),
        })
        .collect()
}

fn lower_host_call_argument(
    argument: &Expression,
    static_values: &[(String, StaticValue)],
) -> HostCallArgumentKind {
    match argument {
        Expression::String(value) => HostCallArgumentKind::Text(value.clone()),
        Expression::Integer(value) => HostCallArgumentKind::Integer(*value),
        Expression::Name(_) => resolve_static_value(argument, static_values)
            .map(host_argument_from_static_value)
            .unwrap_or_else(|| HostCallArgumentKind::Expression(argument.clone())),
        _ => HostCallArgumentKind::Expression(argument.clone()),
    }
}

fn host_argument_from_static_value(value: StaticValue) -> HostCallArgumentKind {
    match value {
        StaticValue::Integer(value) => HostCallArgumentKind::Integer(value),
        StaticValue::Expression(value) => HostCallArgumentKind::Expression(value),
        StaticValue::Text(value) => HostCallArgumentKind::Text(value),
    }
}

fn platform_call_name(call: &Call) -> String {
    match call.receiver.as_deref() {
        Some(receiver) => format!("{receiver}.{}", call.target),
        None => call.target.clone(),
    }
}
