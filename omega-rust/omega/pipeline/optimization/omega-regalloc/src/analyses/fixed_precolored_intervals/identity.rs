//! Canonical V1 identity for fixed/precolored point intervals.

use omega_register_model::RegisterOperandAccess;
use sha2::{Digest, Sha256};

use crate::{
    FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredIntervalPolicy, VirtualFixedConstraintSite,
};

pub fn fixed_precolored_interval_plan_identity(
    plan: &FixedPrecoloredIntervalPlan,
) -> FixedPrecoloredIntervalPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-point-intervals.v1\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_family(&mut bytes, &plan.functions);
    encode_family(&mut bytes, &plan.structural_unit_functions);
    FixedPrecoloredIntervalPlanIdentity(Sha256::digest(bytes).into())
}

fn encode_family(bytes: &mut Vec<u8>, functions: &[crate::FunctionFixedPrecoloredIntervals]) {
    length(bytes, functions.len());
    for function in functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        length(bytes, function.intervals.len());
        for interval in &function.intervals {
            bytes.extend_from_slice(&interval.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&interval.class.0.to_le_bytes());
            encode_site(bytes, interval.site);
            bytes.extend_from_slice(&interval.block.0.to_le_bytes());
            bytes.extend_from_slice(&interval.start.0.to_le_bytes());
            bytes.extend_from_slice(&interval.end.0.to_le_bytes());
            bytes.extend_from_slice(&interval.view.0.to_le_bytes());
        }
    }
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
            .expect("fixed/precolored interval length fits u64")
            .to_le_bytes(),
    );
}
