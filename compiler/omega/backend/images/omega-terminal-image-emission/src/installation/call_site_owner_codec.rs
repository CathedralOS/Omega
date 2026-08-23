//! Canonical format-33 codec for native call-site ownership.
//!
//! Call stack rows and ordering remain in the installation parent. This child
//! owns only the operation-versus-cleanup owner tag and its exact identities.

use omega_terminal_target_operations::TerminalCallSiteOwner;
use psi_core::{EdgeId, OperationId};

use super::{Reader, TerminalInstallationError, push_u32, push_u64};

pub(super) fn encode_call_site_owner(bytes: &mut Vec<u8>, owner: TerminalCallSiteOwner) {
    match owner {
        TerminalCallSiteOwner::Operation(operation) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, operation.get());
        }
        TerminalCallSiteOwner::CleanupAction {
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
) -> Result<TerminalCallSiteOwner, TerminalInstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    match tag {
        1 => Ok(TerminalCallSiteOwner::Operation(
            OperationId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?,
        )),
        2 => {
            let edge = EdgeId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
            let action_ordinal = reader.u32()?;
            if reader.u32()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            Ok(TerminalCallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            })
        }
        tag => Err(TerminalInstallationError::InvalidCallSiteOwnerTag(tag)),
    }
}
