use sha2::{Digest, Sha256};

use crate::{SyntheticReloadValuePlan, SyntheticReloadValuePolicy};

pub fn synthetic_reload_value_plan_identity(
    plan: &SyntheticReloadValuePlan,
) -> crate::SyntheticReloadValuePlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.synthetic-reload-values.v1\0");
    bytes.extend_from_slice(&plan.abstract_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.reload_value_homes.bytes());
    bytes.push(match plan.policy {
        SyntheticReloadValuePolicy::ValidatedSingleSpillEpochZeroCanonicalOrderV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        let Some(binding) = function.binding else {
            bytes.push(0);
            continue;
        };
        bytes.push(1);
        bytes.extend_from_slice(&binding.logical.0.to_le_bytes());
        bytes.extend_from_slice(&binding.synthetic.epoch.to_le_bytes());
        bytes.extend_from_slice(&binding.synthetic.ordinal.to_le_bytes());
        bytes.extend_from_slice(&binding.block.0.to_le_bytes());
        bytes.extend_from_slice(&binding.start.0.to_le_bytes());
        bytes.extend_from_slice(&binding.exclusive_end.0.to_le_bytes());
        bytes.extend_from_slice(&binding.class.0.to_le_bytes());
        bytes.extend_from_slice(&binding.view.0.to_le_bytes());
    }
    crate::SyntheticReloadValuePlanIdentity(Sha256::digest(bytes).into())
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("synthetic reload-value length fits u64")
            .to_le_bytes(),
    );
}
