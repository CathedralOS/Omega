use crate::{FunctionLiveness, LivenessIdentity, LivenessPlan};

/// `liveness_identity` from the digests of `plan.functions`, in order.
pub fn liveness_identity_from_digests(
    plan: &LivenessPlan,
    digests: &[[u8; 32]],
) -> LivenessIdentity {
    assert_eq!(
        digests.len(),
        plan.functions.len(),
        "one digest per liveness function"
    );
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-register-liveness.v11\0");
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&plan.target.identity_bytes());
    crate::codec_bytes::encode_len(&mut bytes, digests.len());
    for digest in digests {
        bytes.extend_from_slice(digest);
    }
    LivenessIdentity(crate::digest_encoder::content_identity(&bytes).into())
}

/// One function's share of `liveness_identity`.
pub fn liveness_function_digest(function: &FunctionLiveness) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(
        256 + function.operand_positions.len() * 24 + function.blocks.len() * 1024,
    );
    bytes.extend_from_slice(b"omega.terminal-register-liveness-function.v2\0");
    encode_function(&mut bytes, function);
    crate::digest_encoder::content_identity(&bytes).into()
}

fn encode_function(bytes: &mut Vec<u8>, function: &FunctionLiveness) {
    crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
    crate::codec_bytes::encode_len(bytes, function.entry_definitions.len());
    for definition in &function.entry_definitions {
        crate::codec_bytes::push_encoded(bytes, definition.virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, definition.class.0.to_le_bytes());
        encode_option_u16(bytes, definition.fixed_view.map(|view| view.0));
    }
    crate::codec_bytes::encode_len(bytes, function.operand_positions.len());
    for operand in function.operand_positions.storage_slice() {
        encode_operand(bytes, operand);
    }
    crate::codec_bytes::encode_len(bytes, function.blocks.len());
    for block in &function.blocks {
        crate::codec_bytes::push_encoded(bytes, block.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, block.source_block.get().to_le_bytes());
        encode_vregs(bytes, &block.virtual_live_in);
        encode_vregs(bytes, &block.virtual_live_out);
        encode_units(bytes, &block.unit_live_in);
        encode_units(bytes, &block.unit_live_out);
        crate::codec_bytes::encode_len(bytes, block.instructions.len());
        for instruction in &block.instructions {
            crate::codec_bytes::push_encoded(bytes, instruction.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, instruction.instruction.0.to_le_bytes());
            // Encode membership by the actual operand rows, not arena addresses.
            let operands = function
                .operand_positions
                .span_or_empty(instruction.operands);
            crate::codec_bytes::encode_len(bytes, operands.len());
            for operand in operands {
                encode_operand(bytes, operand);
            }
            encode_vregs(bytes, &instruction.virtual_uses);
            encode_vregs(bytes, &instruction.virtual_defs);
            encode_vregs(bytes, &instruction.virtual_live_in);
            encode_vregs(bytes, &instruction.virtual_live_out);
            encode_units(bytes, &instruction.unit_uses);
            encode_units(bytes, &instruction.unit_defs);
            encode_units(bytes, &instruction.unit_clobbers);
            encode_units(bytes, &instruction.unit_live_in);
            encode_units(bytes, &instruction.unit_live_out);
        }
        crate::codec_bytes::encode_len(bytes, block.successors.len());
        for successor in &block.successors {
            crate::codec_bytes::push_encoded(bytes, successor.terminator.0.to_le_bytes());
            bytes.push(successor.polarity_ordinal);
            crate::codec_bytes::push_encoded(
                bytes,
                successor
                    .psi_edge
                    .map_or(0, |edge| edge.get())
                    .to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(bytes, successor.target.0.to_le_bytes());
            encode_vregs(bytes, &successor.virtual_live);
            encode_units(bytes, &successor.unit_live);
        }
    }
}

fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
    }
}

fn encode_vregs(bytes: &mut Vec<u8>, values: &[crate::VirtualRegisterId]) {
    crate::codec_bytes::encode_le_list(bytes, values, |value| value.0.to_le_bytes());
}

fn encode_units(bytes: &mut Vec<u8>, values: &[register_model::RegisterUnitId]) {
    crate::codec_bytes::encode_le_list(bytes, values, |value| value.0.to_le_bytes());
}

fn encode_operand(bytes: &mut Vec<u8>, operand: &crate::OperandPosition) {
    crate::codec_bytes::push_encoded(bytes, operand.position.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, operand.instruction.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, operand.operand.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, operand.virtual_register.0.to_le_bytes());
    bytes.push(operand.access.access_tag());
    crate::codec_bytes::push_encoded(bytes, operand.class.0.to_le_bytes());
    encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
    encode_option_u16(bytes, operand.tied_to);
    bytes.push(u8::from(operand.early_clobber));
}
