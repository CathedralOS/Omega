use crate::host_calls::static_values::{StaticValue, resolve_static_value_handle};
use crate::{HostCallArgument, HostCallArgumentKind, LoweredHostOperation, PlaceKey};
use omega_calling_conventions::{HostAbiPlan, HostOperationKey, PlatformCallLowering};
use omega_checked_trees::Program;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::TableCall;
use omega_core::arena::{Arena, HandleSpan};

pub(crate) fn platform_call_receiver_type(
    program: &Program,
    machine: &Machine,
    call: &TableCall,
) -> Option<String> {
    if call.receiver.count() == 0 {
        return None;
    }

    if !call.receiver_symbol.is_valid() {
        return None;
    }

    let receiver_type_symbol = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained_object| contained_object.symbol == call.receiver_symbol)
        .map(|contained_object| contained_object.type_symbol)
        .or_else(|| {
            program
                .data_definitions()
                .iter()
                .find(|data_definition| data_definition.name == machine.name)
                .and_then(|data_definition| {
                    program
                        .data_members(data_definition)
                        .iter()
                        .find_map(|member| match member {
                            omega_checked_trees::data::DataMember::Field(field)
                                if field.symbol == call.receiver_symbol =>
                            {
                                let type_symbol =
                                    program.type_reference_symbol(field.type_reference);
                                type_symbol.is_valid().then_some(type_symbol)
                            }
                            _ => None,
                        })
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find_map(|field| {
                    if field.symbol != call.receiver_symbol {
                        return None;
                    }

                    let type_symbol = program.type_reference_symbol(field.type_reference);
                    type_symbol.is_valid().then_some(type_symbol)
                })
        })
        .or_else(|| {
            call.target_symbol
                .is_valid()
                .then(|| {
                    program
                        .platforms()
                        .iter()
                        .find(|platform| {
                            program
                                .platform_state_signatures(platform)
                                .iter()
                                .any(|state| state.symbol == call.target_symbol)
                        })
                        .map(|platform| platform.symbol)
                })
                .flatten()
        })?;

    program
        .platforms()
        .iter()
        .find(|platform| platform.symbol == receiver_type_symbol)
        .map(|platform| platform.name.to_string())
}

pub(crate) fn find_platform_call_lowering<'abi>(
    host_abi: &'abi HostAbiPlan,
    platform_name: &str,
    call: &TableCall,
) -> Option<&'abi PlatformCallLowering> {
    host_abi
        .platform_call_lowerings
        .iter()
        .find(|(_, lowering)| lowering_matches(lowering, platform_name, &call.target))
        .map(|(_, lowering)| lowering)
}

pub(crate) fn host_operation(operation_key: HostOperationKey) -> LoweredHostOperation {
    LoweredHostOperation { operation_key }
}

pub(crate) fn lower_host_call_arguments(
    program: &Program,
    call: &TableCall,
    static_values: &[(PlaceKey, StaticValue)],
    expressions: &mut ExpressionTable,
    arguments: &mut Arena<HostCallArgument>,
) -> HandleSpan<HostCallArgument> {
    let mut argument_span = HandleSpan::empty();

    for argument in program.statement_table.expression_handles(call.arguments) {
        arguments.append_to_span(
            &mut argument_span,
            HostCallArgument {
                kind: lower_host_call_argument(program, *argument, static_values, expressions),
            },
        );
    }

    argument_span
}

pub(crate) fn platform_call_name(program: &Program, call: &TableCall) -> String {
    let receiver = program.statement_table.name_path_members(call.receiver);

    if receiver.is_empty() {
        return call.target.to_string();
    }

    format!("{}.{}", display_path(receiver), call.target)
}

fn display_path(path: &[omega_checked_trees::name::ProgramName]) -> String {
    let mut display = String::new();

    for member in path {
        if !display.is_empty() {
            display.push('.');
        }
        display.push_str(member.as_str());
    }

    display
}

fn lowering_matches(
    lowering: &PlatformCallLowering,
    platform_name: &str,
    state_name: &str,
) -> bool {
    (lowering.platform == "*" || lowering.platform == platform_name) && lowering.state == state_name
}

fn lower_host_call_argument(
    program: &Program,
    argument: ExpressionHandle,
    static_values: &[(PlaceKey, StaticValue)],
    expressions: &mut ExpressionTable,
) -> HostCallArgumentKind {
    match program.expression_table.expression(argument) {
        ExpressionNode::String(value) => HostCallArgumentKind::Text(value.clone()),
        ExpressionNode::Integer(value) => HostCallArgumentKind::Integer(*value),
        ExpressionNode::Name(_) => {
            resolve_static_value_handle(program, expressions, argument, static_values)
                .map(|value| host_argument_from_static_value(value))
                .unwrap_or_else(|| {
                    HostCallArgumentKind::Expression(
                        expressions.copy_from(&program.expression_table, argument),
                    )
                })
        }
        _ => HostCallArgumentKind::Expression(
            expressions.copy_from(&program.expression_table, argument),
        ),
    }
}

fn host_argument_from_static_value(value: StaticValue) -> HostCallArgumentKind {
    match value {
        StaticValue::Integer(value) => HostCallArgumentKind::Integer(value),
        StaticValue::Expression(value) => HostCallArgumentKind::Expression(value),
        StaticValue::Text(value) => HostCallArgumentKind::Text(value),
    }
}
