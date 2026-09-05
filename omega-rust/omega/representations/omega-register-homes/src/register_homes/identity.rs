//! Exact home identity; field ordering remains the version-6 wire contract.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegisterHomeIdentity(pub(crate) [u8; 32]);

impl RegisterHomeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

use sha2::{Digest, Sha256};

use crate::RegisterHomePlan;

pub fn register_home_identity(plan: &RegisterHomePlan) -> RegisterHomeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-register-homes.v6\0");
    bytes.extend_from_slice(&encode_terminal_register_home_content(plan));
    RegisterHomeIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_register_home_content(plan: &RegisterHomePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.assignments.len());
        for assignment in &function.assignments {
            bytes.extend_from_slice(&assignment.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.class.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
        }
    }
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.assignments.len());
        for assignment in &function.assignments {
            bytes.extend_from_slice(&assignment.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.class.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
        }
    }
    bytes
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("register-home identity length fits u64")
            .to_le_bytes(),
    );
}
