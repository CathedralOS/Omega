//! Canonical V1 identity for abstract spill-access constraints.

use sha2::{Digest, Sha256};

use crate::{
    AbstractSpillAccessConstraintPlan, AbstractSpillAccessConstraintPlanIdentity,
    AbstractSpillAccessConstraintPolicy, AbstractSpillAccessDependencyReason,
    AbstractSpillAccessKind,
};

pub fn abstract_spill_access_constraint_plan_identity(
    plan: &AbstractSpillAccessConstraintPlan,
) -> AbstractSpillAccessConstraintPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.abstract-spill-access-constraints.v1\0");
    bytes.extend_from_slice(&plan.abstract_spill_memory_effects.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        AbstractSpillAccessConstraintPolicy::BlockLocalDataBarrierAndOverlapV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.spill_area_bytes.to_le_bytes());
        length(&mut bytes, function.placements.len());
        for placement in &function.placements {
            bytes.extend_from_slice(&placement.pseudo.ordinal.to_le_bytes());
            bytes.extend_from_slice(&placement.block.0.to_le_bytes());
            bytes.extend_from_slice(&placement.block_ordinal.to_le_bytes());
            bytes.extend_from_slice(&placement.point.0.to_le_bytes());
            bytes.extend_from_slice(&placement.before_instruction.0.to_le_bytes());
            bytes.push(match placement.kind {
                AbstractSpillAccessKind::Write => 0,
                AbstractSpillAccessKind::Read => 1,
            });
            action(&mut bytes, placement.storage);
            bytes.extend_from_slice(&placement.spill_area_offset.to_le_bytes());
            bytes.extend_from_slice(&placement.size_bytes.to_le_bytes());
            bytes.extend_from_slice(&placement.alignment_bytes.to_le_bytes());
        }
        length(&mut bytes, function.dependencies.len());
        for dependency in &function.dependencies {
            bytes.extend_from_slice(&dependency.before.ordinal.to_le_bytes());
            bytes.extend_from_slice(&dependency.after.ordinal.to_le_bytes());
            match dependency.reason {
                AbstractSpillAccessDependencyReason::StoredValue { storage } => {
                    bytes.push(0);
                    action(&mut bytes, storage);
                }
                AbstractSpillAccessDependencyReason::DeclaredBeforeReload => bytes.push(1),
                AbstractSpillAccessDependencyReason::OverlappingAbstractSlice {
                    spill_area_offset,
                    size_bytes,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&spill_area_offset.to_le_bytes());
                    bytes.extend_from_slice(&size_bytes.to_le_bytes());
                }
            }
        }
    }
    AbstractSpillAccessConstraintPlanIdentity(Sha256::digest(bytes).into())
}

fn action(bytes: &mut Vec<u8>, id: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&id.epoch.to_le_bytes());
    bytes.extend_from_slice(&id.ordinal.to_le_bytes());
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("abstract spill-access constraint length fits u64")
            .to_le_bytes(),
    );
}
