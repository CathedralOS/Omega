// Calling-plan, ordinary call, and boundary execution/realization identity encoders for
// legalized operations.
//
// `encode_call_plan`, `encode_placement`, and `encode_shape` serialize target ABI calling plans
// and value shapes. `encode_call` binds a `LegalizedScalarCall`'s access bindings, structural
// result, descriptor origins, call source, target, ABI plan, arguments, claim transfers,
// requirement obligations, and crash continuations. `encode_boundary_execution_identity`,
// `encode_binding`, and `encode_accessor_scalar_demand` serialize boundary execution and
// realization custody independent of the invocation site.

use super::structural::encode_call_source;
use super::structural_types::{encode_structural_argument, encode_target_structural_argument};
use super::{
    encode_access_call_bindings, encode_ids, encode_len, encode_option, encode_register,
    encode_scalar_type,
};
use crate::{
    LegalizedBoundarySettlement, LegalizedScalarArgument, LegalizedScalarCall,
    LegalizedStructuralContract,
};
use calling_conventions::{
    CallPlan, CallbackMaterialization, CallingPolicy, EntryControl, IndirectPointerLocation,
    NativePlace, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement, ValueShape,
};
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution,
};

pub fn encode_call_plan(bytes: &mut Vec<u8>, plan: &CallPlan) {
    bytes.push(match plan.policy {
        CallingPolicy::MicrosoftX64 => 1,
        CallingPolicy::SystemVAMD64 => 2,
        CallingPolicy::Aapcs64 => 3,
        CallingPolicy::LinuxSyscallX86_64 => 4,
        CallingPolicy::LinuxSyscallAarch64 => 5,
    });
    encode_len(bytes, plan.parameters.len());
    for placement in &plan.parameters {
        encode_placement(bytes, placement);
    }
    encode_option(bytes, plan.result.as_ref(), encode_placement);
    encode_len(bytes, plan.callback_materializations.len());
    for materialization in &plan.callback_materializations {
        encode_callback_materialization(bytes, materialization);
    }
    encode_len(bytes, plan.ordinary_clobbers.as_slice().len());
    for register in plan.ordinary_clobbers.as_slice() {
        encode_register(bytes, *register);
    }
    crate::codec_bytes::push_encoded(bytes, plan.stack_alignment.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, plan.shadow_bytes.to_le_bytes());
    match plan.entry_control {
        EntryControl::CallReturn => bytes.push(1),
        EntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            bytes.push(2);
            encode_register(bytes, number_register);
            crate::codec_bytes::push_encoded(bytes, immediate.to_le_bytes());
        }
        EntryControl::InterruptReturn => bytes.push(3),
    }
}

pub(crate) fn encode_callback_materialization(bytes: &mut Vec<u8>, row: &CallbackMaterialization) {
    crate::codec_bytes::push_encoded(bytes, row.binder.get().to_le_bytes());
    encode_native_place(bytes, &row.destination);
}

pub(crate) fn encode_native_place(bytes: &mut Vec<u8>, destination: &NativePlace) {
    match destination {
        NativePlace::Parameter(parameter) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, parameter.get().to_le_bytes());
        }
        NativePlace::Field {
            parameter,
            layout,
            field_path,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, parameter.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, layout.get().to_le_bytes());
            encode_len(bytes, field_path.len());
            for slot in field_path {
                crate::codec_bytes::push_encoded(bytes, slot.get().to_le_bytes());
            }
        }
    }
}

pub fn encode_placement(bytes: &mut Vec<u8>, placement: &ValuePlacement) {
    encode_shape(bytes, placement.shape);
    encode_len(bytes, placement.locations.len());
    for location in &placement.locations {
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                bytes.push(1);
                encode_register(bytes, register);
                crate::codec_bytes::push_encoded(bytes, value_byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, stack_byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, value_byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(3);
                encode_indirect_pointer(bytes, pointer);
                encode_option(bytes, copy_stack_byte_offset, |bytes, offset| {
                    crate::codec_bytes::push_encoded(bytes, offset.to_le_bytes())
                });
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
            }
        }
    }
}

pub(crate) fn encode_indirect_pointer(bytes: &mut Vec<u8>, pointer: IndirectPointerLocation) {
    match pointer {
        IndirectPointerLocation::Register(register) => {
            bytes.push(1);
            encode_register(bytes, register);
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset,
            alignment,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, stack_byte_offset.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
        }
    }
}

pub fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    match shape.class {
        ValueClass::Integer => bytes.push(1),
        ValueClass::Float => bytes.push(2),
        ValueClass::BorrowedReference => bytes.push(5),
        ValueClass::BorrowedScalarReference => bytes.push(6),
        ValueClass::HomogeneousFloatAggregate { members } => {
            bytes.push(3);
            bytes.push(members);
        }
        ValueClass::SystemVAggregate { first, second } => {
            bytes.push(4);
            encode_eightbyte_class(bytes, first);
            encode_eightbyte_class(bytes, second);
        }
    }
    crate::codec_bytes::push_encoded(bytes, shape.byte_size.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, shape.alignment.to_le_bytes());
}

pub(crate) fn encode_eightbyte_class(bytes: &mut Vec<u8>, class: SystemVEightbyteClass) {
    bytes.push(match class {
        SystemVEightbyteClass::Integer => 1,
        SystemVEightbyteClass::Sse => 2,
    });
}

pub(super) fn encode_call(bytes: &mut Vec<u8>, call: &LegalizedScalarCall) {
    encode_access_call_bindings(bytes, &call.access_bindings);
    encode_option(bytes, call.structural_result.as_ref(), |bytes, result| {
        super::structural::encode_operation_result(bytes, result)
    });
    encode_len(bytes, call.descriptor_origins.len());
    for origin in &call.descriptor_origins {
        match origin {
            crate::LegalizedDescriptorOrigin::Opaque => bytes.push(3),
            crate::LegalizedDescriptorOrigin::Empty => bytes.push(2),
            crate::LegalizedDescriptorOrigin::Argument {
                source,
                exact,
                indirect,
            } => {
                bytes.push(0);
                encode_structural_argument(bytes, source);
                bytes.push(u8::from(*exact));
                bytes.push(u8::from(*indirect));
            }
            crate::LegalizedDescriptorOrigin::Literal { machine, place } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
        }
    }
    encode_call_source(bytes, &call.source);
    bytes.extend_from_slice(&call.target.canonical_bytes());
    encode_call_plan(bytes, &call.call_plan);
    encode_len(bytes, call.arguments.len());
    for argument in &call.arguments {
        match argument {
            LegalizedScalarArgument::Scalar { source, placement } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                encode_placement(bytes, placement);
            }
            LegalizedScalarArgument::Structural {
                semantic, target, ..
            } => {
                bytes.push(1);
                encode_structural_argument(bytes, semantic);
                encode_target_structural_argument(bytes, target);
            }
        }
    }
    encode_option(bytes, call.result_placement.as_ref(), encode_placement);
    encode_len(bytes, call.claim_transfers.len());
    for transfer in &call.claim_transfers {
        crate::codec_bytes::push_encoded(bytes, transfer.claim.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, transfer.argument_index.to_le_bytes());
    }
    encode_ids(
        bytes,
        call.requirement_obligations.iter().map(|value| value.get()),
    );
    let crashes = terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
        .expect("canonical call crash continuations");
    encode_len(bytes, crashes.len());
    bytes.extend_from_slice(&crashes);
}

impl LegalizedStructuralContract {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        super::encode_structural_contract(&mut bytes, self, &mut None);
        bytes
    }
}

impl LegalizedScalarCall {
    pub fn canonical_bytes_with_effects(
        &self,
        effect: optimization_unit::EffectLink,
        ownership: &[optimization_unit::OwnershipEvent],
    ) -> Vec<u8> {
        let mut bytes = self.canonical_bytes();
        super::structural::encode_effect(&mut bytes, effect);
        super::structural::encode_ownership_roster(&mut bytes, ownership);
        bytes
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_call(&mut bytes, self);
        bytes
    }
}

impl LegalizedBoundarySettlement {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        super::structural::encode_boundary_settlement(&mut bytes, self);
        bytes
    }
}

pub fn encode_boundary_execution_identity(
    bytes: &mut Vec<u8>,
    execution: BoundaryExecutionBinding,
) {
    match execution {
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::EmptyRunRelease) => {
            bytes.push(9)
        }
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::StaticInterrupt {
            boundary,
            operation,
        }) => {
            bytes.push(10);
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
            bytes.push(operation.code());
        }
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::AccessLifecycle {
            boundary,
            operation,
        }) => {
            bytes.push(8);
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
            bytes.push(match operation {
                terminal_psi::TerminalAccessLifecycleOperation::Initialize => 0,
                terminal_psi::TerminalAccessLifecycleOperation::MoveOut => 1,
                terminal_psi::TerminalAccessLifecycleOperation::View => 2,
                terminal_psi::TerminalAccessLifecycleOperation::Relocate => 3,
            });
        }
        BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::ProgramStorageEntry,
        ) => bytes.push(7),
        BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::ProgramImageRetainedSuffixStart,
        ) => bytes.push(6),
        BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::AccessorScalarAccess {
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
            encode_scalar_type(bytes, semantic_vocabulary::ScalarType::IeeeFloat(format));
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
            BoundaryRealization::EmptyRunRelease => bytes.push(12),
            BoundaryRealization::StaticInterrupt {
                boundary,
                operation,
            } => {
                bytes.push(13);
                bytes.extend_from_slice(&boundary.get().to_le_bytes());
                bytes.push(operation.code());
            }
            BoundaryRealization::AccessLifecycle { boundary } => {
                bytes.push(11);
                bytes.extend_from_slice(&boundary.get().to_le_bytes());
            }
            BoundaryRealization::ProgramImageRetainedSuffixStart => bytes.push(10),
            BoundaryRealization::AccessorScalarAccess(access) => {
                bytes.push(9);
                bytes.extend_from_slice(&access.contract);
                encode_accessor_scalar_demand(bytes, access.demand);
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
                encode_scalar_type(
                    bytes,
                    semantic_vocabulary::ScalarType::IeeeFloat(float.format),
                );
            }
        },
    }
}

pub fn encode_accessor_scalar_demand(
    bytes: &mut Vec<u8>,
    demand: terminal_psi::AccessorScalarBoundaryDemand,
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
    encode_scalar_type(bytes, demand.scalar);
    bytes.extend_from_slice(&demand.byte_offset.to_le_bytes());
    bytes.extend_from_slice(&demand.transfer_width_bits.to_le_bytes());
    bytes.extend_from_slice(&demand.placement_byte_extent.to_le_bytes());
    bytes.push(demand.observation as u8);
}

#[cfg(test)]
mod tests {
    use super::{
        BoundaryExecutionBinding, CompilerBuiltinExecution, encode_boundary_execution_identity,
    };
    use semantic_vocabulary::BoundaryMachineId;
    use terminal_psi::TerminalAccessLifecycleOperation;

    #[test]
    fn lifecycle_execution_identity_binds_boundary_and_operation() {
        let encode = |boundary, operation| {
            let mut bytes = Vec::new();
            encode_boundary_execution_identity(
                &mut bytes,
                BoundaryExecutionBinding::CompilerBuiltin(
                    CompilerBuiltinExecution::AccessLifecycle {
                        boundary: BoundaryMachineId::new(boundary).expect("boundary"),
                        operation,
                    },
                ),
            );
            bytes
        };
        let initialize = encode(1, TerminalAccessLifecycleOperation::Initialize);
        assert_eq!(
            initialize,
            encode(1, TerminalAccessLifecycleOperation::Initialize)
        );
        assert_ne!(
            initialize,
            encode(2, TerminalAccessLifecycleOperation::Initialize)
        );
        assert_ne!(
            initialize,
            encode(1, TerminalAccessLifecycleOperation::MoveOut)
        );
        let mut hosted = Vec::new();
        encode_boundary_execution_identity(
            &mut hosted,
            BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedReadByte),
        );
        assert_ne!(initialize, hosted);
    }
}
