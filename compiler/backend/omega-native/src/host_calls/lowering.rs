use crate::abi::{HostAbiPlan, PlatformCallLowering};
use crate::host_calls::static_values::{StaticValue, resolve_static_value};
use crate::place_keys::PlaceKey;
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::Call;

use super::{HostCallArgument, HostCallArgumentKind, LoweredHostOperation};

pub(in crate::host_calls) fn platform_call_receiver_type(
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
        .find(|contained_object| {
            if call.receiver_symbol.is_valid() {
                contained_object.symbol == call.receiver_symbol
            } else {
                contained_object.name == receiver
            }
        })
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

pub(in crate::host_calls) fn find_platform_call_lowering<'abi>(
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

pub(in crate::host_calls) fn host_operation(
    capability: &str,
    operation: &str,
) -> LoweredHostOperation {
    LoweredHostOperation {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
    }
}

pub(in crate::host_calls) fn lower_host_call_arguments(
    call: &Call,
    static_values: &[(PlaceKey, StaticValue)],
) -> Vec<HostCallArgument> {
    call.arguments
        .iter()
        .map(|argument| HostCallArgument {
            kind: lower_host_call_argument(argument, static_values),
        })
        .collect()
}

pub(in crate::host_calls) fn platform_call_name(call: &Call) -> String {
    match call.receiver.as_deref() {
        Some(receiver) => format!("{receiver}.{}", call.target),
        None => call.target.to_string(),
    }
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
