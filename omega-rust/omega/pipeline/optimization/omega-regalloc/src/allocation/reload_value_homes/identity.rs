use sha2::{Digest, Sha256};

use crate::{ReloadValueHomeIdentity, ReloadValueHomePlan, ReloadValueHomePolicy};

pub fn reload_value_home_identity(plan: &ReloadValueHomePlan) -> ReloadValueHomeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.reload-value-home.v1\0");
    bytes.extend_from_slice(&plan.abstract_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.logical_spill_operations.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        let Some(assignment) = &function.assignment else {
            bytes.push(0);
            continue;
        };
        bytes.push(1);
        bytes.extend_from_slice(&assignment.result.0.to_le_bytes());
        bytes.extend_from_slice(&assignment.block.0.to_le_bytes());
        bytes.extend_from_slice(&assignment.start.0.to_le_bytes());
        bytes.extend_from_slice(&assignment.exclusive_end.0.to_le_bytes());
        bytes.extend_from_slice(&assignment.class.0.to_le_bytes());
        length(&mut bytes, assignment.candidates.len());
        for candidate in &assignment.candidates {
            bytes.extend_from_slice(&candidate.0.to_le_bytes());
        }
        bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
        length(&mut bytes, assignment.coexisting_homes.len());
        for home in &assignment.coexisting_homes {
            bytes.extend_from_slice(&home.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&home.class.0.to_le_bytes());
            bytes.extend_from_slice(&home.view.0.to_le_bytes());
        }
    }
    ReloadValueHomeIdentity(Sha256::digest(bytes).into())
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("reload-value home length fits u64")
            .to_le_bytes(),
    );
}
