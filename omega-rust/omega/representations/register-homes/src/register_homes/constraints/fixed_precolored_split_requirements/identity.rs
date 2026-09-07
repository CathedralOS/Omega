use register_model::RegisterOperandAccess;
use sha2::{Digest, Sha256};
use target::{Architecture, ObjectFormat};

use crate::{
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
};
use selected_instructions::VirtualFixedConstraintSite;

pub fn fixed_precolored_split_requirement_plan_identity(
    plan: &FixedPrecoloredSplitRequirementPlan,
) -> FixedPrecoloredSplitRequirementPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-split-requirements.v1\0");
    bytes.extend_from_slice(&plan.fixed_intervals.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.target.architecture {
        Architecture::X86_64 => 0,
        Architecture::Aarch64 => 1,
    });
    bytes.push(match plan.target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(plan.target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(plan.target.pointer_alignment as u64).to_le_bytes());
    bytes.push(match plan.policy {
        FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_family(&mut bytes, &plan.functions);
    encode_family(&mut bytes, &plan.structural_unit_functions);
    FixedPrecoloredSplitRequirementPlanIdentity(Sha256::digest(bytes).into())
}

fn encode_family(
    bytes: &mut Vec<u8>,
    functions: &[crate::FunctionFixedPrecoloredSplitRequirements],
) {
    length(bytes, functions.len());
    for function in functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        length(bytes, function.registers.len());
        for register in &function.registers {
            bytes.extend_from_slice(&register.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&register.class.0.to_le_bytes());
            length(bytes, register.fragments.len());
            for fragment in &register.fragments {
                bytes.extend_from_slice(&fragment.block.0.to_le_bytes());
                bytes.extend_from_slice(&fragment.source_start.0.to_le_bytes());
                bytes.extend_from_slice(&fragment.source_end.0.to_le_bytes());
                length(bytes, fragment.segments.len());
                for segment in &fragment.segments {
                    bytes.extend_from_slice(&segment.id.0.to_le_bytes());
                    bytes.extend_from_slice(&segment.start.0.to_le_bytes());
                    bytes.extend_from_slice(&segment.end.0.to_le_bytes());
                    length(bytes, segment.candidates.len());
                    for view in &segment.candidates {
                        bytes.extend_from_slice(&view.0.to_le_bytes());
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
                            encode_site(bytes, site);
                            bytes.extend_from_slice(&destination_view.0.to_le_bytes());
                        }
                    }
                }
            }
        }
    }
}

fn encode_connector(bytes: &mut Vec<u8>, connector: selected_instructions::LiveRangeEdgeConnector) {
    bytes.extend_from_slice(&connector.source.0.to_le_bytes());
    bytes.extend_from_slice(&connector.terminator.0.to_le_bytes());
    bytes.push(connector.polarity_ordinal);
    bytes.extend_from_slice(&connector.psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&connector.target.0.to_le_bytes());
}

fn encode_site(bytes: &mut Vec<u8>, site: VirtualFixedConstraintSite) {
    match site {
        VirtualFixedConstraintSite::Entry => bytes.push(0),
        VirtualFixedConstraintSite::Operand {
            position,
            point,
            instruction,
            operand,
            access,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&position.0.to_le_bytes());
            bytes.extend_from_slice(&point.0.to_le_bytes());
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&operand.to_le_bytes());
            bytes.push(match access {
                RegisterOperandAccess::Use => 0,
                RegisterOperandAccess::Def => 1,
                RegisterOperandAccess::UseDef => 2,
            });
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("fixed/precolored split requirement length fits u64")
            .to_le_bytes(),
    );
}
