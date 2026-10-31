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
    LivenessIdentity::from_canonical_bytes(&bytes)
}

/// One function's share of `liveness_identity`.
pub fn liveness_function_digest(function: &FunctionLiveness) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"");
    digest.raw(b"omega.terminal-register-liveness-function.v2\0");
    encode_function(&mut digest, function);
    digest.finish()
}

fn encode_function(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    function: &FunctionLiveness,
) {
    digest.raw(&function.machine.get().to_le_bytes());
    digest.len(function.entry_definitions.len());
    for definition in &function.entry_definitions {
        digest.raw(&definition.virtual_register.0.to_le_bytes());
        digest.raw(&definition.class.0.to_le_bytes());
        encode_option_u16(digest, definition.fixed_view.map(|view| view.0));
    }
    digest.len(function.operand_positions.len());
    for operand in function.operand_positions.storage_slice() {
        encode_operand(digest, operand);
    }
    digest.len(function.blocks.len());
    for block in &function.blocks {
        digest.raw(&block.block.0.to_le_bytes());
        digest.raw(&block.source_block.get().to_le_bytes());
        encode_vregs(digest, &block.virtual_live_in);
        encode_vregs(digest, &block.virtual_live_out);
        encode_units(digest, &block.unit_live_in);
        encode_units(digest, &block.unit_live_out);
        digest.len(block.instructions.len());
        for instruction in &block.instructions {
            digest.raw(&instruction.position.0.to_le_bytes());
            digest.raw(&instruction.instruction.0.to_le_bytes());
            // Encode membership by the actual operand rows, not arena addresses.
            let operands = function
                .operand_positions
                .span_or_empty(instruction.operands);
            digest.len(operands.len());
            for operand in operands {
                encode_operand(digest, operand);
            }
            encode_vregs(digest, &instruction.virtual_uses);
            encode_vregs(digest, &instruction.virtual_defs);
            encode_vregs(digest, &instruction.virtual_live_in);
            encode_vregs(digest, &instruction.virtual_live_out);
            encode_units(digest, &instruction.unit_uses);
            encode_units(digest, &instruction.unit_defs);
            encode_units(digest, &instruction.unit_clobbers);
            encode_units(digest, &instruction.unit_live_in);
            encode_units(digest, &instruction.unit_live_out);
        }
        digest.len(block.successors.len());
        for successor in &block.successors {
            digest.raw(&successor.terminator.0.to_le_bytes());
            digest.byte(successor.polarity_ordinal);
            digest.raw(
                &successor
                    .psi_edge
                    .map_or(0, |edge| edge.get())
                    .to_le_bytes(),
            );
            digest.raw(&successor.target.0.to_le_bytes());
            encode_vregs(digest, &successor.virtual_live);
            encode_units(digest, &successor.unit_live);
        }
    }
}

fn encode_option_u16(digest: &mut crate::digest_encoder::DomainDigestEncoder, value: Option<u16>) {
    match value {
        None => digest.byte(0),
        Some(value) => {
            digest.byte(1);
            digest.raw(&value.to_le_bytes());
        }
    }
}

fn encode_vregs(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    values: &[crate::VirtualRegisterId],
) {
    digest.len(values.len());
    for value in values {
        digest.raw(&value.0.to_le_bytes());
    }
}

fn encode_units(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    values: &[register_model::RegisterUnitId],
) {
    digest.len(values.len());
    for value in values {
        digest.raw(&value.0.to_le_bytes());
    }
}

fn encode_operand(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    operand: &crate::OperandPosition,
) {
    digest.raw(&operand.position.0.to_le_bytes());
    digest.raw(&operand.instruction.0.to_le_bytes());
    digest.raw(&operand.operand.to_le_bytes());
    digest.raw(&operand.virtual_register.0.to_le_bytes());
    digest.byte(operand.access.access_tag());
    digest.raw(&operand.class.0.to_le_bytes());
    encode_option_u16(digest, operand.fixed_view.map(|view| view.0));
    encode_option_u16(digest, operand.tied_to);
    digest.byte(u8::from(operand.early_clobber));
}

#[cfg(test)]
mod tests;
