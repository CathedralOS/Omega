use crate::identity::encode_fixed_constraint_site_identity;

use target::{Architecture, ObjectFormat};

use crate::{
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
};

pub fn fixed_precolored_split_requirement_plan_identity(
    plan: &FixedPrecoloredSplitRequirementPlan,
) -> FixedPrecoloredSplitRequirementPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-split-requirements.v2\0");
    bytes.extend_from_slice(&plan.fixed_intervals.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
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
    bytes.push(match plan.policy {
        FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_family(&mut bytes, &plan.functions);
    FixedPrecoloredSplitRequirementPlanIdentity(
        crate::digest_encoder::content_identity(&bytes).into(),
    )
}

fn encode_family(
    bytes: &mut Vec<u8>,
    functions: &[crate::FunctionFixedPrecoloredSplitRequirements],
) {
    length(bytes, functions.len());
    for function in functions {
        crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
        length(bytes, function.registers.len());
        for register in &function.registers {
            crate::codec_bytes::push_encoded(bytes, register.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, register.class.0.to_le_bytes());
            length(bytes, register.fragments.len());
            for fragment in &register.fragments {
                crate::codec_bytes::push_encoded(bytes, fragment.block.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.source_start.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.source_end.0.to_le_bytes());
                length(bytes, fragment.segments.len());
                for segment in &fragment.segments {
                    crate::codec_bytes::push_encoded(bytes, segment.id.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, segment.start.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, segment.end.0.to_le_bytes());
                    length(bytes, segment.candidates.len());
                    for view in &segment.candidates {
                        crate::codec_bytes::push_encoded(bytes, view.0.to_le_bytes());
                    }
                    match segment.opening {
                        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1 => bytes.push(0),
                        FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 {
                            connector,
                        } => {
                            bytes.push(1);
                            encode_connector(bytes, connector);
                        }
                        FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                            incoming,
                            site,
                            destination_view,
                        } => {
                            bytes.push(2);
                            bytes.push(u8::from(incoming.is_some()));
                            if let Some(connector) = incoming {
                                encode_connector(bytes, connector);
                            }
                            encode_fixed_constraint_site_identity(bytes, site);
                            crate::codec_bytes::push_encoded(bytes, destination_view.0.to_le_bytes());
                        }
                    }
                }
            }
        }
    }
}

fn encode_connector(bytes: &mut Vec<u8>, connector: selected_instructions::LiveRangeEdgeConnector) {
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

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("fixed/precolored split requirement length fits u64")
            .to_le_bytes(),
    );
}
