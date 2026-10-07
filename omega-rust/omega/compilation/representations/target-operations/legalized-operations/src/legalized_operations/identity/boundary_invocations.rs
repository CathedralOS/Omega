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
    bytes.extend_from_slice(&binding.boundary.get().to_le_bytes());
    encode_boundary_execution_identity(bytes, binding.execution);
    match &binding.realization {
        BoundarySettlementRealization::NormalizedForeignCall(call) => {
            bytes.push(8);
            super::normalized_foreign::encode_call_binding(bytes, call);
        }
        BoundarySettlementRealization::Builtin(realization) => match realization {
            BoundaryRealization::MetadataOnlyPort(port) => {
                bytes.push(0);
                bytes.extend_from_slice(&port.effect_operation.get().to_le_bytes());
                bytes.extend_from_slice(&port.service.get().to_le_bytes());
                bytes.extend_from_slice(&port.port.to_le_bytes());
                bytes.push(port.value);
            }
            BoundaryRealization::DirectPortReadU8(port) => {
                bytes.push(1);
                bytes.extend_from_slice(&port.service.get().to_le_bytes());
                bytes.extend_from_slice(&port.port.to_le_bytes());
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
