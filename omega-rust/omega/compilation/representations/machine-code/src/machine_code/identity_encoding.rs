//! Identity bytes for call owners and boundary invocation sites.
//! Call owners and boundary invocation sites retain distinct completion tags.

use selected_instructions::MachineIdentityBytes;
use semantic_vocabulary::BoundaryInvocationSite;
use target_operations::CallSiteOwner;

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
