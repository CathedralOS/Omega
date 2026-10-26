//! Canonical home and fixed-site identity bytes; callers own their wire domains.

crate::identity_newtype! { pub struct RegisterHomeIdentity(pub(crate) [u8; 32]); }

use register_model::RegisterOperandAccess;
use selected_instructions::VirtualFixedConstraintSite;

use crate::RegisterHomePlan;

pub fn register_home_identity(plan: &RegisterHomePlan) -> RegisterHomeIdentity {
    RegisterHomeIdentity(crate::codec_bytes::domain_identity(
        b"omega.terminal-register-homes.v7\0",
        &encode_terminal_register_home_content(plan),
    ))
}

pub(crate) fn encode_terminal_register_home_content(plan: &RegisterHomePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    crate::codec_bytes::length(&mut bytes, plan.functions.len(), "register-home identity");
    for function in &plan.functions {
        crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        crate::codec_bytes::length(
            &mut bytes,
            function.assignments.len(),
            "register-home identity",
        );
        for assignment in &function.assignments {
            crate::codec_bytes::push_encoded(
                &mut bytes,
                assignment.virtual_register.0.to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(&mut bytes, assignment.class.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, assignment.view.0.to_le_bytes());
        }
    }
    bytes
}

crate::identity_newtype! { pub struct AbstractSpillAccessConstraintPlanIdentity([u8; 32]); }

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
            crate::codec_bytes::push_encoded(bytes, position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, point.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, operand.to_le_bytes());
            bytes.push(match access {
                RegisterOperandAccess::Use => 0,
                RegisterOperandAccess::Def => 1,
                RegisterOperandAccess::UseDef => 2,
            });
        }
    }
}
