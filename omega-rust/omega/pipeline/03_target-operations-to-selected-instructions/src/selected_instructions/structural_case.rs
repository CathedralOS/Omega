//! Exact case custody and the physical transport of edge-produced payloads.
use super::{LocalStorageSlotId, VirtualRegisterId};
use crate::legalized_operations::LegalizedStructuralCasePayload;
use semantic_vocabulary::{PlaceId, StructuralCaseId};
use terminal_psi_to_abstract_operations::optimization_unit::encode_value_definition_site_identity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralCaseEdge {
    pub source: SelectedCaseDispatchSource,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<SelectedCasePayloadBinding>,
    pub trivial_affine_discards: Vec<PlaceId>,
}

/// Where a selected case edge reads the dispatched sum. An activation-local
/// home is a frame slot; a borrowed parameter is the entry-retained referent
/// pointer, so the edge reads caller-owned bytes through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCaseDispatchSource {
    Local {
        slot: LocalStorageSlotId,
    },
    Borrowed {
        place: PlaceId,
        pointer: VirtualRegisterId,
        /// The referent's full byte size — the payload-read bound a slot row
        /// supplies for local sources.
        byte_size: u32,
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
    /// Only a value-copy dispatch owns a local storage slot.
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
    /// Construction/projection only. Prepared plans must load on the selected
    /// edge and replace this state before allocation or publication.
    Unmaterialized { parameter: VirtualRegisterId },
    /// No register transfer on this physical leg. On a direct edge the payload
    /// is unused; on a bridge-bound edge the continuation supplies its transfer.
    Unused,
    Registers {
        argument: VirtualRegisterId,
        parameter: VirtualRegisterId,
    },
}

impl SelectedStructuralCaseEdge {
    /// Retain every semantic case coordinate and physical transport field.
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
                bytes.extend_from_slice(&place.get().to_le_bytes());
                bytes.extend_from_slice(&pointer.0.to_le_bytes());
                bytes.extend_from_slice(&byte_size.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&self.case.get().to_le_bytes());
        bytes.extend_from_slice(&self.case_tag.to_le_bytes());
        bytes.extend_from_slice(&(self.payloads.len() as u64).to_le_bytes());
        for payload in &self.payloads {
            bytes.extend_from_slice(&payload.semantic.field.get().to_le_bytes());
            bytes.extend_from_slice(&payload.semantic.field_byte_offset.to_le_bytes());
            bytes.extend_from_slice(&payload.semantic.parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, payload.semantic.parameter.scalar_type);
            encode_value_definition_site_identity(
                bytes,
                payload.semantic.parameter.definition_site,
            );
            match payload.transport {
                SelectedCasePayloadTransport::Unused => bytes.push(0),
                SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&parameter.0.to_le_bytes());
                }
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&argument.0.to_le_bytes());
                    bytes.extend_from_slice(&parameter.0.to_le_bytes());
                }
            }
        }
        bytes.extend_from_slice(&(self.trivial_affine_discards.len() as u64).to_le_bytes());
        for place in &self.trivial_affine_discards {
            bytes.extend_from_slice(&place.get().to_le_bytes());
        }
    }
}
fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: semantic_vocabulary::ScalarType) {
    match scalar_type {
        semantic_vocabulary::ScalarType::Boolean => bytes.push(0),
        semantic_vocabulary::ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => 0,
                semantic_vocabulary::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                semantic_vocabulary::IntegerSign::Signed => 0,
                semantic_vocabulary::IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
        semantic_vocabulary::ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}
