use crate::host_calls::static_values::{StaticValue, resolve_static_value};
use crate::{HostCallArgument, HostCallArgumentKind, LoweredHostOperation, PlaceKey};
use omega_calling_conventions::{HostAbiPlan, HostOperationKey, PlatformCallLowering};
use omega_checked_trees::Program;
use omega_checked_trees::expression::Expression;
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::Call;

pub(crate) fn platform_call_receiver_type(
    program: &Program,
    machine: &Machine,
    call: &Call,
) -> Option<String> {
    if call.receiver.is_empty() {
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
                                type_reference_symbol(&field.type_reference)
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
                    (field.symbol == call.receiver_symbol)
                        .then(|| type_reference_symbol(&field.type_reference))
                        .flatten()
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

fn type_reference_symbol(
    type_reference: &omega_checked_trees::types::TypeReference,
) -> Option<omega_core::symbols::SymbolHandle> {
    match type_reference {
        omega_checked_trees::types::TypeReference::Reference { referee, .. } => {
            type_reference_symbol(referee)
        }
        omega_checked_trees::types::TypeReference::Constrained { base_type, .. } => {
            type_reference_symbol(base_type)
        }
        omega_checked_trees::types::TypeReference::FixedArray { element_type, .. } => {
            type_reference_symbol(element_type)
        }
        omega_checked_trees::types::TypeReference::Slice { element_type } => {
            type_reference_symbol(element_type)
        }
        omega_checked_trees::types::TypeReference::Generic { base_symbol, .. } => {
            base_symbol.is_valid().then_some(*base_symbol)
        }
        omega_checked_trees::types::TypeReference::Named { symbol, .. } => {
            symbol.is_valid().then_some(*symbol)
        }
        omega_checked_trees::types::TypeReference::Unit => None,
    }
}

pub(crate) fn find_platform_call_lowering<'abi>(
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

pub(crate) fn host_operation(operation_key: HostOperationKey) -> LoweredHostOperation {
    LoweredHostOperation { operation_key }
}

pub(crate) fn lower_host_call_arguments(
    program: &Program,
    call: &Call,
    static_values: &[(PlaceKey, StaticValue)],
) -> Vec<HostCallArgument> {
    program
        .call_arguments(call)
        .iter()
        .map(|argument| HostCallArgument {
            kind: lower_host_call_argument(argument, static_values),
        })
        .collect()
}

pub(crate) fn platform_call_name(program: &Program, call: &Call) -> String {
    let receiver = program.statement_path_members(call.receiver);

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
    argument: &Expression,
    static_values: &[(PlaceKey, StaticValue)],
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
