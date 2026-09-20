//! Indirect descriptor-parameter call identity: the retained dispatch join,
//! the signature-bound `{instance, table}` ABI pair, the invoked requirement
//! row, the erased adapter plan, the slot offset, and the result home encode
//! as one indivisible custody row.
use super::calling::{encode_call_plan, encode_placement, encode_shape};
use super::scalar::encode_scalar_type;
use super::shared::*;
use super::structural_types::{encode_access, encode_string};
use target_operations::TargetUnitScalarHomeRequirement;
use terminal_psi::{TerminalDynamicDescriptorParameter, TerminalDynamicRequirement};

pub(super) fn encode_dynamic_parameter_call(
    bytes: &mut Vec<u8>,
    call: &LegalizedDynamicParameterCall,
) {
    encode_descriptor_parameter(bytes, &call.dynamic_dispatch.parameter);
    bytes.extend_from_slice(&call.dynamic_dispatch.dispatch.owner.get().to_le_bytes());
    bytes.extend_from_slice(&call.dynamic_dispatch.dispatch.operation.get().to_le_bytes());
    bytes.extend_from_slice(
        &call
            .dynamic_dispatch
            .dispatch
            .parameter_ordinal
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &call
            .dynamic_dispatch
            .dispatch
            .requirement_slot
            .to_le_bytes(),
    );
    encode_descriptor_parameter(bytes, &call.parameter_abi.parameter);
    encode_placement(bytes, &call.parameter_abi.instance);
    encode_placement(bytes, &call.parameter_abi.table);
    encode_requirement(bytes, &call.requirement);
    encode_call_plan(bytes, &call.dispatch_call_plan);
    bytes.extend_from_slice(&call.table_slot_byte_offset.to_le_bytes());
    match &call.result_home {
        Some(home) => {
            bytes.push(1);
            encode_scalar_home(bytes, home);
        }
        None => bytes.push(0),
    }
    encode_ids(
        bytes,
        call.requirement_obligations.iter().map(|value| value.get()),
    );
    let crashes = terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
        .expect("canonical call crash continuations");
    encode_len(bytes, crashes.len());
    bytes.extend_from_slice(&crashes);
}

fn encode_descriptor_parameter(
    bytes: &mut Vec<u8>,
    parameter: &TerminalDynamicDescriptorParameter,
) {
    bytes.extend_from_slice(&parameter.owner.get().to_le_bytes());
    bytes.extend_from_slice(&parameter.ordinal.to_le_bytes());
    bytes.extend_from_slice(&parameter.source_position.to_le_bytes());
    encode_string(bytes, &parameter.trait_identity);
    encode_access(bytes, parameter.access);
    encode_len(bytes, parameter.requirements.len());
    for requirement in &parameter.requirements {
        encode_requirement(bytes, requirement);
    }
}

fn encode_requirement(bytes: &mut Vec<u8>, requirement: &TerminalDynamicRequirement) {
    bytes.extend_from_slice(&requirement.slot.to_le_bytes());
    encode_string(bytes, &requirement.declaring_trait_identity);
    encode_string(bytes, &requirement.public_requirement_identity);
    encode_len(bytes, requirement.family_tuple.len());
    for identity in &requirement.family_tuple {
        encode_string(bytes, identity);
    }
    bytes.push(match requirement.result {
        terminal_psi::ClosedConformanceCallableResult::Unit => 0,
        terminal_psi::ClosedConformanceCallableResult::I32 => 1,
        terminal_psi::ClosedConformanceCallableResult::Bool => 2,
    });
}

fn encode_scalar_home(bytes: &mut Vec<u8>, home: &TargetUnitScalarHomeRequirement) {
    bytes.extend_from_slice(&home.defining_operation.get().to_le_bytes());
    bytes.extend_from_slice(&home.source_value.get().to_le_bytes());
    encode_scalar_type(bytes, home.scalar_type);
    encode_shape(bytes, home.shape);
}
