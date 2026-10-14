//! Boundary execution and realization custody, independent of invocation site.
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution,
};

pub fn encode_boundary_execution_identity(
    bytes: &mut Vec<u8>,
    execution: BoundaryExecutionBinding,
) {
    match execution {
        BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::PlacedScalarAccess {
                contract,
                placement_commitment,
                access_slot,
                operation,
            },
        ) => {
            bytes.push(5);
            bytes.extend_from_slice(&contract);
            bytes.extend_from_slice(&placement_commitment);
            bytes.extend_from_slice(&access_slot.to_le_bytes());
            bytes.push(operation as u8);
        }

        BoundaryExecutionBinding::AdmittedProvider(provider) => {
            bytes.push(0);
            super::structural::encode_provider_execution(bytes, provider);
        }
        BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::HostedExitProcessI32,
        ) => bytes.push(1),
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedReadByte) => {
            bytes.push(2)
        }
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedWriteByteI32) => {
            bytes.push(3)
        }
        BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format },
        ) => {
            bytes.push(4);
            bytes.push(operation.ordinal());
            super::scalar::encode_scalar_type(
                bytes,
                semantic_vocabulary::ScalarType::IeeeFloat(format),
            );
        }
    }
}

pub(super) fn encode_binding(bytes: &mut Vec<u8>, binding: &BoundarySettlementBinding) {
    crate::codec_bytes::push_encoded(bytes, binding.boundary.get().to_le_bytes());
    encode_boundary_execution_identity(bytes, binding.execution);
    match &binding.realization {
        BoundarySettlementRealization::NormalizedForeignCall(call) => {
            bytes.push(8);
            super::normalized_foreign::encode_call_binding(bytes, call);
        }
        BoundarySettlementRealization::Builtin(realization) => match realization {
            BoundaryRealization::PlacedScalarAccess(access) => {
                bytes.push(9);
                bytes.extend_from_slice(&access.contract);
                encode_placed_scalar_demand(bytes, access.demand);
            }

            BoundaryRealization::MetadataOnlyPort(port) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, port.effect_operation.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, port.service.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, port.port.to_le_bytes());
                bytes.push(port.value);
            }
            BoundaryRealization::DirectPortReadU8(port) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, port.service.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, port.port.to_le_bytes());
            }
            BoundaryRealization::LinuxWriteLine(_) => bytes.push(2),
            BoundaryRealization::HostedExitProcessI32(_) => bytes.push(3),
            BoundaryRealization::HostedReadByte(_) => bytes.push(4),
            BoundaryRealization::HostedWriteByteI32(_) => bytes.push(5),
            BoundaryRealization::ClaimCompletionOnly(_) => bytes.push(6),
            BoundaryRealization::NearestIeeeFloatArithmetic(float) => {
                bytes.push(7);
                bytes.push(float.operation.ordinal());
                super::scalar::encode_scalar_type(
                    bytes,
                    semantic_vocabulary::ScalarType::IeeeFloat(float.format),
                );
            }
        },
    }
}

pub fn encode_placed_scalar_demand(
    bytes: &mut Vec<u8>,
    demand: terminal_psi::PlacedScalarBoundaryDemand,
) {
    for identity in [
        demand.boundary.get(),
        demand.carrier.get(),
        demand.schema.get(),
        demand.accessor.get(),
    ] {
        bytes.extend_from_slice(&identity.to_le_bytes());
    }
    bytes.extend_from_slice(&demand.view_field.0.to_le_bytes());
    bytes.extend_from_slice(&demand.access_slot.to_le_bytes());
    bytes.extend_from_slice(&demand.placement_commitment);
    bytes.push(demand.operation as u8);
    bytes.push(demand.access as u8);
    super::scalar::encode_scalar_type(bytes, demand.scalar);
    bytes.extend_from_slice(&demand.byte_offset.to_le_bytes());
    bytes.extend_from_slice(&demand.transfer_width_bits.to_le_bytes());
    bytes.extend_from_slice(&demand.placement_byte_extent.to_le_bytes());
    bytes.push(demand.observation as u8);
}
