//! Plain binding data; complete plan and live occurrence are checked separately.
use super::encoding::encode_len;
use target_operations::{TargetPlacedCallAssociation, TargetPlacedCallBinding};
use terminal_psi::{StructuralAccess, TerminalPlacedViewInput};

fn encode_input(bytes: &mut Vec<u8>, input: &TerminalPlacedViewInput) {
    bytes.extend_from_slice(&input.machine.get().to_le_bytes());
    bytes.extend_from_slice(&input.position.to_le_bytes());
    bytes.extend_from_slice(&input.runtime_position.to_le_bytes());
    for identity in [
        input.carrier_identity,
        input.source_machine_identity,
        input.source_state_identity,
        input.source_parameter_identity,
    ] {
        bytes.extend_from_slice(&identity.0.to_le_bytes());
    }
    bytes.push(match input.access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
    bytes.push(u8::from(input.binding_is_const));
    bytes.push(u8::from(input.binding_is_mutable));
    for identity in [
        input.view_identity,
        input.policy_identity,
        input.policy_plan_machine_identity,
        input.schema_identity,
    ] {
        bytes.extend_from_slice(&identity.0.to_le_bytes());
    }
    bytes.extend_from_slice(&input.placement_report_fingerprint.to_le_bytes());
    bytes.extend_from_slice(&input.placement_commitment);
}

pub fn encode_placed_call_bindings(bytes: &mut Vec<u8>, bindings: &[TargetPlacedCallBinding]) {
    encode_len(bytes, bindings.len());
    for binding in bindings {
        encode_input(bytes, &binding.input);
        bytes.extend_from_slice(&binding.argument_index.to_le_bytes());
        match &binding.association {
            TargetPlacedCallAssociation::Captured { schema, carrier } => {
                bytes.push(0);
                bytes.extend_from_slice(&schema.get().to_le_bytes());
                bytes.extend_from_slice(&carrier.get().to_le_bytes());
            }
            TargetPlacedCallAssociation::CallerFormal(input) => {
                bytes.push(1);
                encode_input(bytes, input);
            }
        }
    }
}
