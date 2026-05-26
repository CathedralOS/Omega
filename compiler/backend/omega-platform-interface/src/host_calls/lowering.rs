use crate::host_calls::static_values::{StaticValue, StaticValues, resolve_static_value_handle};
use crate::{HostCallArgument, HostCallArgumentKind, LoweredHostOperation};
use omega_calling_conventions::{
    HostAbiPlan, HostOperationKey, PlatformCallLowering, PlatformCallLoweringHandle,
    host_operation_fixed_leading_immediate,
};
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::TableCall;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

pub(crate) fn platform_call_receiver_type<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    call: &TableCall,
) -> Option<&'program str> {
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
        .or_else(|| data_field_type_symbol(program, call.receiver_symbol))
        .or_else(|| {
            let receiver_leaf_name = receiver_leaf_name(program, call);
            program
                .data_definitions()
                .iter()
                .find(|data_definition| {
                    Some(&data_definition.name) == machine.attached_data.as_ref()
                })
                .and_then(|data_definition| {
                    program
                        .data_members(data_definition)
                        .iter()
                        .find_map(|member| match member {
                            omega_checked_trees::data::DataMember::Field(field)
                                if field.symbol == call.receiver_symbol
                                    || receiver_leaf_name == Some(field.name.as_str()) =>
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
                        .traits()
                        .iter()
                        .filter(|trait_definition| trait_definition.is_boundary)
                        .find(|trait_definition| {
                            program
                                .trait_machine_signatures(trait_definition)
                                .iter()
                                .any(|machine| machine.symbol == call.target_symbol)
                        })
                        .map(|trait_definition| trait_definition.symbol)
                })
                .flatten()
        })?;

    receiver_surface_name(program, receiver_type_symbol)
}

fn receiver_leaf_name<'program>(
    program: &'program CheckedTrees,
    call: &TableCall,
) -> Option<&'program str> {
    program
        .statement_table
        .name_path_members(call.receiver)
        .last()
        .map(|name| name.as_str())
}

fn data_field_type_symbol(
    program: &CheckedTrees,
    field_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    program
        .data_definitions()
        .iter()
        .find_map(|data_definition| {
            program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    omega_checked_trees::data::DataMember::Field(field)
                        if field.symbol == field_symbol =>
                    {
                        let type_symbol = program.type_reference_symbol(field.type_reference);
                        type_symbol.is_valid().then_some(type_symbol)
                    }
                    _ => None,
                })
        })
}

fn receiver_surface_name(program: &CheckedTrees, symbol: SymbolHandle) -> Option<&str> {
    program
        .traits()
        .iter()
        .filter(|trait_definition| trait_definition.is_boundary)
        .find(|trait_definition| trait_definition.symbol == symbol)
        .map(|trait_definition| trait_definition.name.as_str())
        .or_else(|| {
            program
                .platforms()
                .iter()
                .find(|platform| platform.symbol == symbol)
                .map(|platform| platform.name.as_str())
        })
}

pub(crate) fn find_platform_call_lowering<'abi>(
    host_abi: &'abi HostAbiPlan,
    platform_name: &str,
    call: &TableCall,
) -> Option<(PlatformCallLoweringHandle, &'abi PlatformCallLowering)> {
    host_abi
        .platform_call_lowerings
        .iter()
        .find(|(_, lowering)| lowering_matches(lowering, platform_name, &call.target))
        .map(|(handle, lowering)| (handle, lowering))
}

pub(crate) fn host_operation(
    host_abi: &HostAbiPlan,
    operation_key: HostOperationKey,
) -> LoweredHostOperation {
    LoweredHostOperation {
        operation_key,
        fixed_leading_immediate: host_operation_fixed_leading_immediate(host_abi, operation_key),
    }
}

pub(crate) fn lower_host_call_arguments(
    program: &CheckedTrees,
    call: &TableCall,
    static_values: &StaticValues,
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

pub(crate) fn platform_call_name(program: &CheckedTrees, call: &TableCall) -> String {
    let receiver = program.statement_table.name_path_members(call.receiver);

    if receiver.is_empty() {
        return call.target.to_string();
    }

    let byte_count = receiver
        .iter()
        .map(|member| member.as_str().len())
        .sum::<usize>()
        + receiver.len()
        + call.target.len();
    let mut display = String::with_capacity(byte_count);

    for (index, member) in receiver.iter().enumerate() {
        if index > 0 {
            display.push('.');
        }
        display.push_str(member.as_str());
    }
    display.push('.');
    display.push_str(&call.target);

    display
}

fn lowering_matches(
    lowering: &PlatformCallLowering,
    platform_name: &str,
    state_name: &str,
) -> bool {
    (lowering.platform.as_ref() == "*" || lowering.platform.as_ref() == platform_name)
        && lowering.state.as_ref() == state_name
}

fn lower_host_call_argument(
    program: &CheckedTrees,
    argument: ExpressionHandle,
    static_values: &StaticValues,
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
