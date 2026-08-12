use crate::host_calls::static_values::{StaticValue, StaticValues, resolve_static_value_handle};
use crate::{HostCallArgument, HostCallArgumentKind, LoweredHostOperation};
use omega_calling_conventions::{
    HostAbiPlan, HostOperationKey, PlatformCallLowering, PlatformCallLoweringHandle,
    host_operation_fixed_leading_immediate,
};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::machine::Machine;
use psi_checked_trees::statement::TableCall;
use psi_symbols::SymbolHandle;

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

    let receiver_type_symbol = data_field_type_symbol(program, call.receiver_symbol)
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
                            psi_checked_trees::data::DataMember::Field(field)
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
                    psi_checked_trees::data::DataMember::Field(field)
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
}

/// The boundary type name behind an EXPRESSION call -- the
/// assignment-RHS host-call shape `self.t = self.clock.tick_count()`.
/// Resolved through the boundary-trait fallback: the call's resolved target
/// symbol is a signature of exactly one boundary trait.
pub(crate) fn expression_platform_receiver_type<'program>(
    program: &'program CheckedTrees,
    target_symbol: psi_symbols::SymbolHandle,
) -> Option<&'program str> {
    if !target_symbol.is_valid() {
        return None;
    }
    if let Some(trait_symbol) = program
        .traits()
        .iter()
        .filter(|trait_definition| trait_definition.is_boundary)
        .find(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .any(|machine| machine.symbol == target_symbol)
        })
        .map(|trait_definition| trait_definition.symbol)
    {
        return receiver_surface_name(program, trait_symbol);
    }

    None
}

/// Like `find_platform_call_lowering`, keyed directly on the callee target
/// name (for expression calls, which have no statement `TableCall`).
pub(crate) fn find_platform_call_lowering_by_target<'abi>(
    program: &CheckedTrees,
    host_abi: &'abi HostAbiPlan,
    platform_name: &str,
    target: &psi_checked_trees::name::Identifier,
    target_symbol: SymbolHandle,
) -> Option<(PlatformCallLoweringHandle, &'abi PlatformCallLowering)> {
    find_lowering_prefer_exact(
        host_abi,
        platform_name,
        target,
        requirement_identity(program, target_symbol).as_deref(),
    )
}

pub(crate) fn find_platform_call_lowering<'abi>(
    program: &CheckedTrees,
    host_abi: &'abi HostAbiPlan,
    platform_name: &str,
    call: &TableCall,
) -> Option<(PlatformCallLoweringHandle, &'abi PlatformCallLowering)> {
    find_lowering_prefer_exact(
        host_abi,
        platform_name,
        &call.target,
        requirement_identity(program, call.target_symbol).as_deref(),
    )
}

fn requirement_identity(program: &CheckedTrees, target_symbol: SymbolHandle) -> Option<String> {
    program.traits().iter().find_map(|definition| {
        let signature = program
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)?;
        Some(
            program
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity(),
        )
    })
}

/// Resolve a lowering, PREFERRING an exact platform (boundary-trait) match over
/// the `"*"` wildcard. This lets a per-trait lowering (e.g. `FilesystemHost`'s
/// `write`) win over a wildcard one (Console's `*::write`), so both surfaces can
/// use the same human method name without colliding.
fn find_lowering_prefer_exact<'abi>(
    host_abi: &'abi HostAbiPlan,
    platform_name: &str,
    state_name: &str,
    requirement_identity: Option<&str>,
) -> Option<(PlatformCallLoweringHandle, &'abi PlatformCallLowering)> {
    let exact_identity = requirement_identity.and_then(|identity| {
        host_abi
            .platform_call_lowerings
            .iter()
            .find(|(_, lowering)| {
                lowering.platform.as_ref() == platform_name && lowering.state.as_ref() == identity
            })
    });
    let wildcard_identity = requirement_identity.and_then(|identity| {
        host_abi
            .platform_call_lowerings
            .iter()
            .find(|(_, lowering)| {
                lowering.platform.as_ref() == "*" && lowering.state.as_ref() == identity
            })
    });
    exact_identity
        .or(wildcard_identity)
        .or_else(|| {
            host_abi
                .platform_call_lowerings
                .iter()
                .find(|(_, lowering)| {
                    lowering.platform.as_ref() == platform_name
                        && lowering.state.as_ref() == state_name
                })
                .or_else(|| {
                    host_abi
                        .platform_call_lowerings
                        .iter()
                        .find(|(_, lowering)| {
                            lowering.platform.as_ref() == "*"
                                && lowering.state.as_ref() == state_name
                        })
                })
        })
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

    for (index, argument) in program
        .statement_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        arguments.append_to_span(
            &mut argument_span,
            HostCallArgument {
                kind: lower_host_call_argument(program, *argument, static_values, expressions),
                is_borrowed: matches!(
                    program.expression_table.expression(*argument),
                    ExpressionNode::Mutable(_)
                ),
                expects_reference: call_parameter_expects_reference(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    index,
                ),
                expects_address: call_parameter_expects_address(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    index,
                ),
            },
        );
    }

    argument_span
}

pub(crate) fn call_parameter_expects_reference(
    program: &CheckedTrees,
    target_symbol: SymbolHandle,
    target_name: &str,
    index: usize,
) -> bool {
    let signature = program.traits().iter().find_map(|definition| {
        program
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| {
                (target_symbol.is_valid() && signature.symbol == target_symbol)
                    || (!target_symbol.is_valid() && signature.name.as_str() == target_name)
            })
    });
    let Some(parameter) = signature.and_then(|signature| {
        program
            .state_signature_parameters(signature)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .nth(index)
    }) else {
        return false;
    };
    type_reference_is_reference(program, parameter.type_reference)
}

pub(crate) fn call_parameter_expects_address(
    program: &CheckedTrees,
    target_symbol: SymbolHandle,
    target_name: &str,
    index: usize,
) -> bool {
    let signature = program.traits().iter().find_map(|definition| {
        program
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| {
                (target_symbol.is_valid() && signature.symbol == target_symbol)
                    || (!target_symbol.is_valid() && signature.name.as_str() == target_name)
            })
    });
    let Some(parameter) = signature.and_then(|signature| {
        program
            .state_signature_parameters(signature)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .nth(index)
    }) else {
        return false;
    };
    type_reference_is_address(program, parameter.type_reference)
}

fn type_reference_is_reference(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        psi_checked_trees::types::TypeReferenceNode::Reference { .. } => true,
        psi_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_reference(program, *base_type)
        }
        _ => false,
    }
}

fn type_reference_is_address(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        psi_checked_trees::types::TypeReferenceNode::Named { name, .. } => name.as_str() == "addr",
        psi_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_address(program, *base_type)
        }
        _ => false,
    }
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

pub(crate) fn lower_host_call_argument(
    program: &CheckedTrees,
    argument: ExpressionHandle,
    static_values: &StaticValues,
    expressions: &mut ExpressionTable,
) -> HostCallArgumentKind {
    match program.expression_table.expression(argument) {
        ExpressionNode::String(value) => HostCallArgumentKind::Text(value.clone()),
        // Oversize (u64-magnitude) literals fall through to the Expression
        // path, whose consumers reject non-simple host arguments LOUDLY.
        ExpressionNode::Integer(value) => match value.value_i64() {
            Some(value) => HostCallArgumentKind::Integer(value),
            None => HostCallArgumentKind::Expression(
                expressions.copy_from(&program.expression_table, argument),
            ),
        },
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
