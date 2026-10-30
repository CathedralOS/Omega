//! Canonical V1 identity for fixed/precolored point intervals.

use crate::identity::encode_fixed_constraint_site_identity;

use crate::{
    FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPlanIdentity, FixedPrecoloredIntervalPolicy,
};

pub fn fixed_precolored_interval_plan_identity(
    plan: &FixedPrecoloredIntervalPlan,
) -> FixedPrecoloredIntervalPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-point-intervals.v2\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_family(&mut bytes, &plan.functions);
    FixedPrecoloredIntervalPlanIdentity(crate::digest_encoder::content_identity(&bytes).into())
}

fn encode_family(bytes: &mut Vec<u8>, functions: &[crate::FunctionFixedPrecoloredIntervals]) {
    length(bytes, functions.len());
    for function in functions {
        crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
        length(bytes, function.intervals.len());
        for interval in &function.intervals {
            crate::codec_bytes::push_encoded(bytes, interval.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.class.0.to_le_bytes());
            encode_fixed_constraint_site_identity(bytes, interval.site);
            crate::codec_bytes::push_encoded(bytes, interval.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.start.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.end.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.view.0.to_le_bytes());
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    crate::codec_bytes::length(bytes, value, "fixed/precolored interval");
}
