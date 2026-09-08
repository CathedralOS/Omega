use calling_conventions::ConventionalSumLayout;
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use terminal_psi::StructuralOperationResult;

/// Bind an exact byte-input occurrence and its complete structural result payload.
/// Encoding identity does not establish source admission or physical realization.
pub fn encode_hosted_read_byte_identity(
    bytes: &mut Vec<u8>,
    operation: OperationId,
    boundary: BoundaryMachineId,
    result: &StructuralOperationResult,
    layout: &ConventionalSumLayout,
) {
    bytes.extend_from_slice(&operation.get().to_le_bytes());
    encode_payload(bytes, boundary, result, layout);
}

pub(super) fn encode_payload(
    bytes: &mut Vec<u8>,
    boundary: BoundaryMachineId,
    result: &StructuralOperationResult,
    layout: &ConventionalSumLayout,
) {
    bytes.extend_from_slice(&boundary.get().to_le_bytes());
    super::projected_structural_call_return::encode_operation_result(bytes, result);
    encode_layout(bytes, layout);
}

pub(super) fn encode_layout(bytes: &mut Vec<u8>, layout: &ConventionalSumLayout) {
    super::calling::encode_shape(bytes, layout.shape);
    bytes.extend_from_slice(&layout.tag_byte_offset.to_le_bytes());
    super::calling::encode_shape(bytes, layout.tag_shape);
    super::shared::encode_len(bytes, layout.common_fields.len());
    for field in &layout.common_fields {
        super::calling::encode_shape(bytes, field.shape);
        bytes.extend_from_slice(&field.byte_offset.to_le_bytes());
    }
    bytes.extend_from_slice(&layout.payload_byte_offset.to_le_bytes());
    super::shared::encode_len(bytes, layout.cases.len());
    for case in &layout.cases {
        super::shared::encode_len(bytes, case.fields.len());
        for field in &case.fields {
            super::calling::encode_shape(bytes, field.shape);
            bytes.extend_from_slice(&field.byte_offset.to_le_bytes());
        }
    }
}
