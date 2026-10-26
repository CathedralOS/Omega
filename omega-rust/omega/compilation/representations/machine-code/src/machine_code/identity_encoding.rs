//! Identity bytes for call owners, boundary invocation sites, and the
//! resolved control-flow evidence every branch-bearing identity writes.
//! Call owners and boundary invocation sites retain distinct completion tags.

use selected_instructions::MachineIdentityBytes;
use semantic_vocabulary::BoundaryInvocationSite;
use target_operations::CallSiteOwner;

use crate::{ResolvedConditionalBranchEvidence, ResolvedJumpEvidence};

/// One resolved conditional branch's destinations after its own discriminant
/// and predicate rank: source block, both branch edges with their blocks and
/// offsets, and the target-relative displacement. Every identity that carries
/// this evidence writes these bytes in this order.
pub(super) fn encode_conditional_branch_arm(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    branch: &ResolvedConditionalBranchEvidence,
) {
    hasher.byte(1);
    hasher.byte(branch.predicate.rank());
    hasher.raw(&branch.source_block.0.to_le_bytes());
    hasher.raw(
        &branch
            .when_taken_edge
            .map_or(0, |edge| edge.get())
            .to_le_bytes(),
    );
    hasher.raw(&branch.when_taken_block.0.to_le_bytes());
    hasher.raw(&branch.when_taken_offset.to_le_bytes());
    hasher.raw(
        &branch
            .when_fallthrough_edge
            .map_or(0, |edge| edge.get())
            .to_le_bytes(),
    );
    hasher.raw(&branch.when_fallthrough_block.0.to_le_bytes());
    hasher.raw(&branch.when_fallthrough_offset.to_le_bytes());
    hasher.raw(&branch.byte_displacement.to_le_bytes());
}

/// One resolved jump's destination after its own discriminant byte: source
/// block, target edge, target block and offset, and the displacement.
pub(super) fn encode_jump_arm(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    jump: &ResolvedJumpEvidence,
) {
    hasher.byte(2);
    hasher.raw(&jump.source_block.0.to_le_bytes());
    hasher.raw(&jump.target_edge.map_or(0, |edge| edge.get()).to_le_bytes());
    hasher.raw(&jump.target_block.0.to_le_bytes());
    hasher.raw(&jump.target_offset.to_le_bytes());
    hasher.raw(&jump.byte_displacement.to_le_bytes());
}

pub(super) fn encode_call_owner(bytes: &mut impl MachineIdentityBytes, owner: CallSiteOwner) {
    match owner {
        CallSiteOwner::Operation(operation) => {
            bytes.identity_bytes(&[0]);
            bytes.identity_bytes(&operation.get().to_le_bytes());
        }
        CallSiteOwner::ExternalCompletion(edge) => {
            bytes.identity_bytes(&[2]);
            bytes.identity_bytes(&edge.get().to_le_bytes());
        }
        CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            bytes.identity_bytes(&[1]);
            bytes.identity_bytes(&edge.get().to_le_bytes());
            bytes.identity_bytes(&action_ordinal.to_le_bytes());
        }
    }
}

pub(super) fn encode_invocation_site(
    bytes: &mut impl MachineIdentityBytes,
    site: BoundaryInvocationSite,
) {
    match site {
        BoundaryInvocationSite::Operation(operation) => {
            bytes.identity_bytes(&[0]);
            bytes.identity_bytes(&operation.get().to_le_bytes());
        }
        BoundaryInvocationSite::ExternalCompletion(edge) => {
            bytes.identity_bytes(&[1]);
            bytes.identity_bytes(&edge.get().to_le_bytes());
        }
    }
}
