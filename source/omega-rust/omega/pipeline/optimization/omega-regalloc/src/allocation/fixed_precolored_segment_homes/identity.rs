use omega_target::{Architecture, ObjectFormat};
use sha2::{Digest, Sha256};

use crate::{
    FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSegmentHomePolicy,
};

pub fn fixed_precolored_segment_home_plan_identity(
    plan: &FixedPrecoloredSegmentHomePlan,
) -> FixedPrecoloredSegmentHomePlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-segment-homes.v1\0");
    bytes.extend_from_slice(&plan.split_requirements.bytes());
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
        FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_family(&mut bytes, &plan.functions);
    encode_family(&mut bytes, &plan.structural_unit_functions);
    FixedPrecoloredSegmentHomePlanIdentity(Sha256::digest(bytes).into())
}

fn encode_family(bytes: &mut Vec<u8>, functions: &[crate::FunctionFixedPrecoloredSegmentHomes]) {
    length(bytes, functions.len());
    for function in functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        length(bytes, function.assignments.len());
        for assignment in &function.assignments {
            bytes.extend_from_slice(&assignment.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.class.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.source_segment.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.allocation_domain.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
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
