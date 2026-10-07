//! Leaf identity bytes shared by fragment, exit-contract and section writers.
//! Call owners and boundary invocation sites retain distinct completion tags.
//! Fragment target tags differ and remain in their own writer.

use selected_instructions::MachineIdentityBytes;
use semantic_vocabulary::BoundaryInvocationSite;
use target::{Architecture, NativeTarget, ObjectFormat};
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

pub(super) fn encode_exit_target(bytes: &mut impl MachineIdentityBytes, target: NativeTarget) {
    bytes.identity_bytes(&[match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    bytes.identity_bytes(&[match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    bytes.identity_bytes(&(target.pointer_size as u64).to_le_bytes());
    bytes.identity_bytes(&(target.pointer_alignment as u64).to_le_bytes());
}
