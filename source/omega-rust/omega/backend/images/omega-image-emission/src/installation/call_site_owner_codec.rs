//! Canonical format-36 codec for native call-site ownership.
//!
//! Call stack rows and ordering remain in the installation parent. This child
//! owns only the operation-versus-cleanup owner tag and its exact identities.

use omega_target_operations::CallSiteOwner;
use psi_core::{EdgeId, OperationId};

use super::{InstallationError, Reader, push_u32, push_u64};

pub(super) fn encode_call_site_owner(bytes: &mut Vec<u8>, owner: CallSiteOwner) {
    match owner {
        CallSiteOwner::Operation(operation) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, operation.get());
        }
        CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, edge.get());
            push_u32(bytes, action_ordinal);
            push_u32(bytes, 0);
        }
    }
}

pub(super) fn decode_call_site_owner(
    reader: &mut Reader<'_>,
) -> Result<CallSiteOwner, InstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    match tag {
        1 => Ok(CallSiteOwner::Operation(
            OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
        )),
        2 => {
            let edge = EdgeId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            let action_ordinal = reader.u32()?;
            if reader.u32()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            Ok(CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            })
        }
        tag => Err(InstallationError::InvalidCallSiteOwnerTag(tag)),
    }
}
