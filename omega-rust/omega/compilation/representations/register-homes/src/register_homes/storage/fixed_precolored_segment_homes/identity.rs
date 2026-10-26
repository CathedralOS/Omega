use crate::{
    FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSegmentHomePolicy,
};

pub fn fixed_precolored_segment_home_plan_identity(
    plan: &FixedPrecoloredSegmentHomePlan,
) -> FixedPrecoloredSegmentHomePlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-segment-homes.v2\0");
    bytes.extend_from_slice(&plan.split_requirements.bytes());
    bytes.extend_from_slice(&plan.fixed_intervals.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    crate::codec_bytes::encode_target(&mut bytes, plan.target);
    bytes.push(match plan.policy {
        FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_family(&mut bytes, &plan.functions);
    FixedPrecoloredSegmentHomePlanIdentity(crate::digest_encoder::content_identity(&bytes).into())
}

fn encode_family(bytes: &mut Vec<u8>, functions: &[crate::FunctionFixedPrecoloredSegmentHomes]) {
    length(bytes, functions.len());
    for function in functions {
        crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
        length(bytes, function.assignments.len());
        for assignment in &function.assignments {
            crate::codec_bytes::push_encoded(bytes, assignment.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.class.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.source_segment.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.allocation_domain.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.view.0.to_le_bytes());
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("fixed/precolored segment-home length fits u64")
            .to_le_bytes(),
    );
}
