/*
  Exact structural case custody and physical transport of edge-produced case
  payloads.

  `SelectedStructuralCaseEdge` records where a selected case edge reads the
  dispatched sum (`SelectedCaseDispatchSource`), which `StructuralCaseId` and
  discriminant tag it matches, how each case payload field is transported
  (`SelectedCasePayloadBinding`), and any affine cleanup actions settled on the
  branch.
*/

use super::{LocalStorageSlotId, VirtualRegisterId};
use crate::codec_bytes::push_encoded;
use legalized_operations::{LegalizedStructuralCasePayload, encode_scalar_type};
use optimization_unit::encode_value_definition_site_identity;
use semantic_vocabulary::{PlaceId, StructuralCaseId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralCaseEdge {
    pub source: SelectedCaseDispatchSource,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<SelectedCasePayloadBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCaseDispatchSource {
    Local {
        slot: LocalStorageSlotId,
    },
    Borrowed {
        place: PlaceId,
        pointer: VirtualRegisterId,
        byte_size: u32, // The addressed value's full byte size.
    },
}

impl SelectedCaseDispatchSource {
    pub fn place(&self) -> Option<PlaceId> {
        match self {
            Self::Local { slot } => slot.structural_place(),
            Self::Borrowed { place, .. } => Some(*place),
        }
    }

    pub fn byte_size(&self) -> Option<u32> {
        match self {
            Self::Borrowed { byte_size, .. } => Some(*byte_size),
            _ => None,
        }
    }

    pub fn local_slot(&self) -> Option<LocalStorageSlotId> {
        match self {
            Self::Local { slot } => Some(*slot),
            Self::Borrowed { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCasePayloadBinding {
    pub semantic: LegalizedStructuralCasePayload,
    pub transport: SelectedCasePayloadTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCasePayloadTransport {
    Unmaterialized {
        parameter: VirtualRegisterId,
    },
    Unused,
    Registers {
        argument: VirtualRegisterId,
        parameter: VirtualRegisterId,
    },
}

impl SelectedStructuralCaseEdge {
    pub fn encode_identity(&self, bytes: &mut Vec<u8>) {
        match self.source {
            SelectedCaseDispatchSource::Local { slot } => {
                bytes.push(0);
                slot.encode_identity(bytes);
            }
            SelectedCaseDispatchSource::Borrowed {
                place,
                pointer,
                byte_size,
            } => {
                bytes.push(1);
                push_encoded(bytes, place.get().to_le_bytes());
                push_encoded(bytes, pointer.0.to_le_bytes());
                push_encoded(bytes, byte_size.to_le_bytes());
            }
        }
        push_encoded(bytes, self.case.get().to_le_bytes());
        push_encoded(bytes, self.case_tag.to_le_bytes());
        push_encoded(bytes, (self.payloads.len() as u64).to_le_bytes());
        for payload in &self.payloads {
            push_encoded(bytes, payload.semantic.field.get().to_le_bytes());
            push_encoded(bytes, payload.semantic.field_byte_offset.to_le_bytes());
            push_encoded(bytes, payload.semantic.parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, payload.semantic.parameter.scalar_type);
            encode_value_definition_site_identity(
                bytes,
                payload.semantic.parameter.definition_site,
            );
            match payload.transport {
                SelectedCasePayloadTransport::Unused => bytes.push(0),
                SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                    bytes.push(1);
                    push_encoded(bytes, parameter.0.to_le_bytes());
                }
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => {
                    bytes.push(2);
                    push_encoded(bytes, argument.0.to_le_bytes());
                    push_encoded(bytes, parameter.0.to_le_bytes());
                }
            }
        }
        push_encoded(bytes, (self.cleanup_actions.len() as u64).to_le_bytes());
        for action in &self.cleanup_actions {
            super::effects::program::identity::encode_cleanup(bytes, action);
        }
    }
}
