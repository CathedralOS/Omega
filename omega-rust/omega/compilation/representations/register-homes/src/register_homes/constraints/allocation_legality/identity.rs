use sha2::{Digest, Sha256};

use crate::{AllocationLegalityIdentity, AllocationLegalityPlan, FunctionAllocationLegality};
use selected_instructions::VirtualFixedConstraintSite;

/// `allocation_legality_identity` from the digests of `plan.functions`, in
/// order, as `allocation_legality_function_digest` computes them.
pub fn allocation_legality_identity_from_digests(
    plan: &AllocationLegalityPlan,
    digests: &[[u8; 32]],
) -> AllocationLegalityIdentity {
    assert_eq!(
        digests.len(),
        plan.functions.len(),
        "one digest per allocation-legality function"
    );
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocation-legality.v7\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    length(&mut bytes, digests.len());
    for digest in digests {
        bytes.extend_from_slice(digest);
    }
    AllocationLegalityIdentity::from_bytes(Sha256::digest(bytes).into())
}

/// One function's share of `allocation_legality_identity`.
pub fn allocation_legality_function_digest(function: &FunctionAllocationLegality) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocation-legality-function.v1\0");
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    length(&mut bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        bytes.extend_from_slice(&register.virtual_register.0.to_le_bytes());
        bytes.extend_from_slice(&register.class.0.to_le_bytes());
        length(&mut bytes, register.points.len());
        for point in &register.points {
            bytes.extend_from_slice(&point.block.0.to_le_bytes());
            bytes.extend_from_slice(&point.point.0.to_le_bytes());
            length(&mut bytes, point.candidates.len());
            for candidate in &point.candidates {
                bytes.extend_from_slice(&candidate.0.to_le_bytes());
            }
        }
        length(&mut bytes, register.early_clobber_points.len());
        for point in &register.early_clobber_points {
            bytes.extend_from_slice(&point.block.0.to_le_bytes());
            bytes.extend_from_slice(&point.position.0.to_le_bytes());
            bytes.extend_from_slice(&point.instruction.0.to_le_bytes());
            bytes.extend_from_slice(&point.operand.to_le_bytes());
            bytes.extend_from_slice(&point.point.0.to_le_bytes());
            length(&mut bytes, point.candidates.len());
            for candidate in &point.candidates {
                bytes.extend_from_slice(&candidate.0.to_le_bytes());
            }
        }
        length(&mut bytes, register.entry_transitions.len());
        for transition in &register.entry_transitions {
            bytes.extend_from_slice(&transition.from_view.0.to_le_bytes());
            encode_fixed_site(&mut bytes, transition.to_site);
            bytes.extend_from_slice(&transition.to_view.0.to_le_bytes());
        }
    }
    Sha256::digest(bytes).into()
}

fn encode_fixed_site(bytes: &mut Vec<u8>, site: VirtualFixedConstraintSite) {
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
                register_model::RegisterOperandAccess::Use => 0,
                register_model::RegisterOperandAccess::Def => 1,
                register_model::RegisterOperandAccess::UseDef => 2,
            });
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("allocation-legality identity length fits u64")
            .to_le_bytes(),
    );
}
