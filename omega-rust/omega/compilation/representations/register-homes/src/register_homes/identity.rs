//! Canonical home and fixed-site identity bytes; callers own their wire domains.

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

use register_model::RegisterOperandAccess;
use selected_instructions::VirtualFixedConstraintSite;
use sha2::{Digest, Sha256};

use crate::RegisterHomePlan;

pub fn register_home_identity(plan: &RegisterHomePlan) -> RegisterHomeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-register-homes.v7\0");
    bytes.extend_from_slice(&encode_terminal_register_home_content(plan));
    RegisterHomeIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_register_home_content(plan: &RegisterHomePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    crate::codec_bytes::length(&mut bytes, plan.functions.len(), "register-home identity");
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        crate::codec_bytes::length(
            &mut bytes,
            function.assignments.len(),
            "register-home identity",
        );
        for assignment in &function.assignments {
            bytes.extend_from_slice(&assignment.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.class.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
        }
    }
    bytes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbstractSpillAccessConstraintPlanIdentity([u8; 32]);

impl AbstractSpillAccessConstraintPlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

pub(crate) fn encode_fixed_constraint_site_identity(
    bytes: &mut Vec<u8>,
    site: VirtualFixedConstraintSite,
) {
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
