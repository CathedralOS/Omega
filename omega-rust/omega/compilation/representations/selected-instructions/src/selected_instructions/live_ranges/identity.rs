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
    bytes.extend_from_slice(&plan.target.identity_bytes());
    crate::codec_bytes::encode_len(&mut bytes, digests.len());
    for digest in digests {
        bytes.extend_from_slice(digest);
    }
    LiveRangeIdentity::from_bytes(crate::digest_encoder::content_identity(&bytes).into())
}

/// One function's share of `live_range_identity`.
pub fn live_range_function_digest(function: &crate::FunctionLiveRanges) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"");
    digest.raw(b"omega.terminal-live-range-function.v1\0");
    digest.raw(&function.machine.get().to_le_bytes());
    digest.len(function.block_domains.len());
    for block in &function.block_domains {
        digest.raw(&block.block.0.to_le_bytes());
        digest.raw(&block.source_block.get().to_le_bytes());
        digest.raw(&block.start.0.to_le_bytes());
        digest.raw(&block.end.0.to_le_bytes());
    }
    digest.len(function.virtual_registers.len());
    for register in &function.virtual_registers {
        digest.raw(&register.virtual_register.0.to_le_bytes());
        digest.raw(&register.class.0.to_le_bytes());
        digest.len(register.occurrences.len());
        for occurrence in &register.occurrences {
            digest.raw(&occurrence.position.0.to_le_bytes());
            digest.raw(&occurrence.point.0.to_le_bytes());
            digest.raw(&occurrence.instruction.0.to_le_bytes());
            digest.raw(&occurrence.operand.to_le_bytes());
            digest.byte(occurrence.access.access_tag());
        }
        digest.len(register.fixed_constraints.len());
        for constraint in &register.fixed_constraints {
            match constraint.site {
                VirtualFixedConstraintSite::Entry => digest.byte(0),
                VirtualFixedConstraintSite::Operand {
                    position,
                    point,
                    instruction,
                    operand,
                    access,
                } => {
                    digest.byte(1);
                    digest.raw(&position.0.to_le_bytes());
                    digest.raw(&point.0.to_le_bytes());
                    digest.raw(&instruction.0.to_le_bytes());
                    digest.raw(&operand.to_le_bytes());
                    digest.byte(access.access_tag());
                }
            }
            digest.raw(&constraint.view.0.to_le_bytes());
        }
        encode_fragments(&mut digest, &register.fragments);
        encode_connectors(&mut digest, &register.edge_connectors);
    }
    digest.len(function.tied_pairs.len());
    digest.len(function.edge_transfers.len());
    for edge in &function.edge_transfers {
        digest.raw(&edge.source.0.to_le_bytes());
        digest.raw(&edge.target.0.to_le_bytes());
        digest.raw(&edge.psi_edge.get().to_le_bytes());
        digest.raw(&edge.argument.0.to_le_bytes());
        digest.raw(&edge.parameter.0.to_le_bytes());
        digest.raw(&edge.class.0.to_le_bytes());
    }
    digest.len(function.copy_affinities.len());
    for affinity in &function.copy_affinities {
        digest.raw(&affinity.block.0.to_le_bytes());
        digest.raw(&affinity.instruction.0.to_le_bytes());
        digest.raw(&affinity.source.0.to_le_bytes());
        digest.raw(&affinity.destination.0.to_le_bytes());
    }
    for tie in &function.tied_pairs {
        digest.raw(&tie.block.0.to_le_bytes());
        digest.raw(&tie.position.0.to_le_bytes());
        digest.raw(&tie.instruction.0.to_le_bytes());
        digest.raw(&tie.use_operand.to_le_bytes());
        digest.raw(&tie.use_virtual_register.0.to_le_bytes());
        digest.raw(&tie.use_point.0.to_le_bytes());
        digest.raw(&tie.def_operand.to_le_bytes());
        digest.raw(&tie.def_virtual_register.0.to_le_bytes());
        digest.raw(&tie.def_point.0.to_le_bytes());
        digest.raw(&tie.class.0.to_le_bytes());
    }
    digest.len(function.early_clobbers.len());
    for early in &function.early_clobbers {
        digest.raw(&early.block.0.to_le_bytes());
        digest.raw(&early.position.0.to_le_bytes());
        digest.raw(&early.instruction.0.to_le_bytes());
        digest.raw(&early.early_point.0.to_le_bytes());
        digest.raw(&early.def_operand.to_le_bytes());
        digest.raw(&early.def_virtual_register.0.to_le_bytes());
        digest.raw(&early.def_class.0.to_le_bytes());
        digest.raw(&early.def_point.0.to_le_bytes());
        digest.len(early.uses.len());
        for used in &early.uses {
            digest.raw(&used.operand.to_le_bytes());
            digest.raw(&used.virtual_register.0.to_le_bytes());
            digest.raw(&used.class.0.to_le_bytes());
        }
    }
    digest.len(function.architectural_units.len());
    for unit in &function.architectural_units {
        digest.raw(&unit.unit.0.to_le_bytes());
        digest.len(unit.actions.len());
        for action in &unit.actions {
            digest.raw(&action.block.0.to_le_bytes());
            digest.raw(&action.position.0.to_le_bytes());
            digest.raw(&action.point.0.to_le_bytes());
            digest.raw(&action.instruction.0.to_le_bytes());
            digest.byte(match action.kind {
                ArchitecturalUnitActionKind::Use => 0,
                ArchitecturalUnitActionKind::Def => 1,
                ArchitecturalUnitActionKind::Clobber => 2,
            });
        }
        encode_fragments(&mut digest, &unit.fragments);
        encode_connectors(&mut digest, &unit.edge_connectors);
    }
    digest.len(function.interference.len());
    for pair in &function.interference {
        digest.raw(&pair.lower.0.to_le_bytes());
        digest.raw(&pair.higher.0.to_le_bytes());
    }
    digest.finish()
}

fn encode_fragments(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    fragments: &[crate::LiveRangeFragment],
) {
    digest.len(fragments.len());
    for fragment in fragments {
        digest.raw(&fragment.block.0.to_le_bytes());
        digest.raw(&fragment.start.0.to_le_bytes());
        digest.raw(&fragment.end.0.to_le_bytes());
    }
}

fn encode_connectors(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    connectors: &[crate::LiveRangeEdgeConnector],
) {
    digest.len(connectors.len());
    for connector in connectors {
        digest.raw(&connector.source.0.to_le_bytes());
        digest.raw(&connector.terminator.0.to_le_bytes());
        digest.byte(connector.polarity_ordinal);
        digest.raw(
            &connector
                .psi_edge
                .map_or(0, |edge| edge.get())
                .to_le_bytes(),
        );
        digest.raw(&connector.target.0.to_le_bytes());
    }
}

#[cfg(test)]
mod tests;
