use target::{Architecture, ObjectFormat};

use crate::{
    ArchitecturalUnitActionKind, LiveRangeIdentity, LiveRangePlan, VirtualFixedConstraintSite,
};

/// `live_range_identity` from the digests of `plan.functions`, in order.
pub fn live_range_identity_from_digests(
    plan: &LiveRangePlan,
    digests: &[[u8; 32]],
) -> LiveRangeIdentity {
    assert_eq!(
        digests.len(),
        plan.functions.len(),
        "one digest per live-range function"
    );
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-live-range-fragments.v12\0");
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.liveness.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.target.architecture {
        Architecture::X86_64 => 0,
        Architecture::Aarch64 => 1,
    });
    bytes.push(match plan.target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    });
    crate::codec_bytes::push_encoded(&mut bytes, (plan.target.pointer_size as u64).to_le_bytes());
    crate::codec_bytes::push_encoded(
        &mut bytes,
        (plan.target.pointer_alignment as u64).to_le_bytes(),
    );
    crate::codec_bytes::encode_len(&mut bytes, digests.len());
    for digest in digests {
        bytes.extend_from_slice(digest);
    }
    LiveRangeIdentity::from_bytes(crate::digest_encoder::content_identity(&bytes).into())
}

/// One function's share of `live_range_identity`.
pub fn live_range_function_digest(function: &crate::FunctionLiveRanges) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(
        256 + function.virtual_registers.len() * 48 + function.interference.len() * 8,
    );
    bytes.extend_from_slice(b"omega.terminal-live-range-function.v1\0");
    crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
    crate::codec_bytes::encode_len(&mut bytes, function.block_domains.len());
    for block in &function.block_domains {
        crate::codec_bytes::push_encoded(&mut bytes, block.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, block.source_block.get().to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, block.start.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, block.end.0.to_le_bytes());
    }
    crate::codec_bytes::encode_len(&mut bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        crate::codec_bytes::push_encoded(&mut bytes, register.virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, register.class.0.to_le_bytes());
        crate::codec_bytes::encode_len(&mut bytes, register.occurrences.len());
        for occurrence in &register.occurrences {
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.point.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.operand.to_le_bytes());
            bytes.push(occurrence.access.access_tag());
        }
        crate::codec_bytes::encode_len(&mut bytes, register.fixed_constraints.len());
        for constraint in &register.fixed_constraints {
            match constraint.site {
                VirtualFixedConstraintSite::Entry => bytes.push(0),
                VirtualFixedConstraintSite::Operand {
                    position,
                    point,
                    instruction,
                    operand,
                    access,
                } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(&mut bytes, position.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, point.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, instruction.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, operand.to_le_bytes());
                    bytes.push(access.access_tag());
                }
            }
            crate::codec_bytes::push_encoded(&mut bytes, constraint.view.0.to_le_bytes());
        }
        encode_fragments(&mut bytes, &register.fragments);
        encode_connectors(&mut bytes, &register.edge_connectors);
    }
    crate::codec_bytes::encode_len(&mut bytes, function.tied_pairs.len());
    crate::codec_bytes::encode_len(&mut bytes, function.edge_transfers.len());
    for edge in &function.edge_transfers {
        crate::codec_bytes::push_encoded(&mut bytes, edge.source.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.target.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.psi_edge.get().to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.argument.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.parameter.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.class.0.to_le_bytes());
    }
    crate::codec_bytes::encode_len(&mut bytes, function.copy_affinities.len());
    for affinity in &function.copy_affinities {
        crate::codec_bytes::push_encoded(&mut bytes, affinity.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, affinity.instruction.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, affinity.source.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, affinity.destination.0.to_le_bytes());
    }
    for tie in &function.tied_pairs {
        crate::codec_bytes::push_encoded(&mut bytes, tie.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.position.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.instruction.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.use_operand.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.use_virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.use_point.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.def_operand.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.def_virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.def_point.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.class.0.to_le_bytes());
    }
    crate::codec_bytes::encode_len(&mut bytes, function.early_clobbers.len());
    for early in &function.early_clobbers {
        crate::codec_bytes::push_encoded(&mut bytes, early.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.position.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.instruction.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.early_point.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_operand.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_class.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_point.0.to_le_bytes());
        crate::codec_bytes::encode_len(&mut bytes, early.uses.len());
        for used in &early.uses {
            crate::codec_bytes::push_encoded(&mut bytes, used.operand.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, used.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, used.class.0.to_le_bytes());
        }
    }
    crate::codec_bytes::encode_len(&mut bytes, function.architectural_units.len());
    for unit in &function.architectural_units {
        crate::codec_bytes::push_encoded(&mut bytes, unit.unit.0.to_le_bytes());
        crate::codec_bytes::encode_len(&mut bytes, unit.actions.len());
        for action in &unit.actions {
            crate::codec_bytes::push_encoded(&mut bytes, action.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, action.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, action.point.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, action.instruction.0.to_le_bytes());
            bytes.push(match action.kind {
                ArchitecturalUnitActionKind::Use => 0,
                ArchitecturalUnitActionKind::Def => 1,
                ArchitecturalUnitActionKind::Clobber => 2,
            });
        }
        encode_fragments(&mut bytes, &unit.fragments);
        encode_connectors(&mut bytes, &unit.edge_connectors);
    }
    crate::codec_bytes::encode_len(&mut bytes, function.interference.len());
    for pair in &function.interference {
        crate::codec_bytes::push_encoded(&mut bytes, pair.lower.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, pair.higher.0.to_le_bytes());
    }
    crate::digest_encoder::content_identity(&bytes).into()
}

fn encode_fragments(bytes: &mut Vec<u8>, fragments: &[crate::LiveRangeFragment]) {
    crate::codec_bytes::encode_len(bytes, fragments.len());
    for fragment in fragments {
        crate::codec_bytes::push_encoded(bytes, fragment.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, fragment.start.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, fragment.end.0.to_le_bytes());
    }
}

fn encode_connectors(bytes: &mut Vec<u8>, connectors: &[crate::LiveRangeEdgeConnector]) {
    crate::codec_bytes::encode_len(bytes, connectors.len());
    for connector in connectors {
        crate::codec_bytes::push_encoded(bytes, connector.source.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, connector.terminator.0.to_le_bytes());
        bytes.push(connector.polarity_ordinal);
        crate::codec_bytes::push_encoded(
            bytes,
            connector
                .psi_edge
                .map_or(0, |edge| edge.get())
                .to_le_bytes(),
        );
        crate::codec_bytes::push_encoded(bytes, connector.target.0.to_le_bytes());
    }
}
