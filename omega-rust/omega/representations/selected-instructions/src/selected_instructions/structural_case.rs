//! Exact case custody and the physical transport of edge-produced payloads.
use super::{LocalStorageSlotId, VirtualRegisterId};
use legalized_operations::LegalizedStructuralCasePayload;
use semantic_vocabulary::{PlaceId, StructuralCaseId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralCaseEdge {
    pub slot: LocalStorageSlotId,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<SelectedCasePayloadBinding>,
    pub trivial_affine_discards: Vec<PlaceId>,
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
        self.slot.encode_identity(bytes);
        bytes.extend_from_slice(&self.case.get().to_le_bytes());
        bytes.extend_from_slice(&self.case_tag.to_le_bytes());
        bytes.extend_from_slice(&(self.payloads.len() as u64).to_le_bytes());
        for payload in &self.payloads {
            bytes.extend_from_slice(&payload.semantic.field.get().to_le_bytes());
            bytes.extend_from_slice(&payload.semantic.field_byte_offset.to_le_bytes());
            bytes.extend_from_slice(&payload.semantic.parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, payload.semantic.parameter.scalar_type);
            encode_definition_site(bytes, payload.semantic.parameter.definition_site);
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

fn encode_definition_site(bytes: &mut Vec<u8>, site: optimization_unit::ValueDefinitionSite) {
    match site {
        optimization_unit::ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        optimization_unit::ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        optimization_unit::ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}
