//! Optimizer module role: stage group. Canonical pre-allocation effect identity assembly.
//!
//! Root identities, ordinary CFG rows, and structural-unit rows are appended
//! here in their identity-defining order. Named leaves own each row vocabulary.

mod alternative;
mod instruction;
mod ownership;
mod provenance;
mod values;

#[cfg(test)]
mod tests;
use crate::{PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan};

use instruction::encode_cfg_instruction;
pub use ownership::{encode_effect_link, encode_ownership};
pub use provenance::encode_provenance;
use values::{encode_len, encode_target};

pub fn pre_allocation_machine_effect_identity(
    plan: &PreAllocationMachineEffectPlan,
) -> PreAllocationMachineEffectIdentity {
    identity_with_domain(plan, b"omega.terminal-preallocation-machine-effects.v11\0")
}

fn identity_with_domain(
    plan: &PreAllocationMachineEffectPlan,
    domain: &[u8],
) -> PreAllocationMachineEffectIdentity {
    use sha2::{Digest, Sha256};

    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encode_terminal_pre_allocation_machine_effect_content(plan));
    PreAllocationMachineEffectIdentity::from_bytes(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_pre_allocation_machine_effect_content(
    plan: &PreAllocationMachineEffectPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.register_constraints.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());

    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.block.0.to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                encode_cfg_instruction(&mut bytes, instruction);
            }
        }
    }

    bytes
}
