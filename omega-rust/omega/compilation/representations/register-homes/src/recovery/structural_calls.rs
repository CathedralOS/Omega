/*
We encode and decode ordinary scalar/resultless calls (`SelectedCallContract`),
normalized foreign calls (`SelectedNormalizedForeignCall`), calling convention
plans (`CallPlan`, `ValuePlacement`, `ValueShape`, `StatePlan`), provider
candidate conformances (`ProviderCandidateConformance`), access call bindings,
structural argument sources, and structural operation results inside the
`FixedViewCopyPlan` selected payload.

Decoding reconstructs sealed values through their owning admission paths rather
than trusting wire bytes: foreign locators are re-normalized, system-call
bindings validate their boundary entry plans, and same-stack contributions are
re-admitted so their recomputed report identity and commitment must equal the
retained bytes.
*/

use ::selected_instructions::effects::program::identity::{encode_ownership, encode_path};
use ::selected_instructions::{
    SelectedCallContract, SelectedInstructionId, SelectedNormalizedForeignCall,
};
use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallbackRequirementId, CallingPolicy,
    ConventionalSumCaseLayout, ConventionalSumLayout, EntryControl, EntryStack,
    IndirectPointerLocation, LayoutPlanId, LayoutSlotId, MachineRegime, MachineRegister,
    MachineState, MachineStateSet, NativeCallbackDemand, NativeParameterApplication,
    NativeParameterId, NativePlace, PackedFieldLayout, Preemption, RegisterSet, StatePlan,
    StaticMachineBinderId, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, validate_boundary_entry_plan,
};
use function_identity::MachineFunctionIdentity;
use legalized_operations::{
    LegalizedNormalizedForeignCall, LegalizedScalarArgument, LegalizedScalarCall,
    encode_call_binding, encode_call_plan, encode_integer, encode_native_callback_argument,
    encode_placement, encode_provider_execution, encode_shape,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, MachineId, ObligationId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use target::{
    ForeignLocatorCandidate, NormalizedComponentLocator, TargetProfile, normalize_foreign_locator,
};
use target_operations::{
    NormalizedForeignCallBinding, TargetAccessCallAssociation, TargetAccessCallBinding,
    TargetNativeCallbackArgument, TargetScalarBlockValue, TargetStructuralArgumentSource,
    TargetStructuralHomeLayout, TargetStructuralHomeOrigin, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
};
use task_plans::{
    SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
    SameStackProviderPlanCommitment, admit_same_stack_contribution,
};
use terminal_psi::{
    ClaimTransfer, ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, SemanticFingerprint,
    StructuralDomainRequirement, StructuralOperationResult, StructuralResultClaimBinding,
    TerminalAccessViewInput, TerminalIdentityIndex,
};

use crate::FixedViewCopyDecodeError;

use super::fixed_view_copy_codec::{
    Cursor, decode_accepted_fact, decode_bool, decode_id, decode_ids, decode_integer, decode_list,
    decode_option_u64, decode_scalar, encode_ids, encode_option_u64, encode_scalar, length,
};
use super::structural_contracts::{
    decode_call_source, decode_effect, decode_invocation_site, decode_ownership,
    decode_provider_execution, encode_call_source, encode_effect,
};
use super::structural_declarations::{
    decode_access, decode_multiplicity, decode_path, decode_projected, decode_qualification_path,
    decode_semantic_argument, decode_semantic_parameter, decode_string, decode_target_argument,
    encode_access, encode_multiplicity, encode_projected, encode_semantic_argument,
    encode_semantic_parameter, encode_target_argument,
};

const MACHINE_STATE_SET_BITS: u16 = (1 << 9) - 1;

fn decode_access_view_input(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalAccessViewInput, FixedViewCopyDecodeError> {
    Ok(TerminalAccessViewInput {
        machine: decode_id(cursor, MachineId::new)?,
        position: cursor.u32()?,
        runtime_position: cursor.u32()?,
        carrier_identity: TerminalIdentityIndex(cursor.u32()?),
        source_machine_identity: TerminalIdentityIndex(cursor.u32()?),
        source_state_identity: TerminalIdentityIndex(cursor.u32()?),
        source_parameter_identity: TerminalIdentityIndex(cursor.u32()?),
        access: decode_access(cursor)?,
        binding_is_const: decode_bool(cursor)?,
        binding_is_mutable: decode_bool(cursor)?,
        view_identity: TerminalIdentityIndex(cursor.u32()?),
        policy_identity: TerminalIdentityIndex(cursor.u32()?),
        policy_plan_machine_identity: TerminalIdentityIndex(cursor.u32()?),
        schema_identity: TerminalIdentityIndex(cursor.u32()?),
        placement_report_fingerprint: cursor.u64()?,
        placement_commitment: cursor
            .take(32)?
            .try_into()
            .map_err(|_| FixedViewCopyDecodeError::Truncated)?,
    })
}

pub(super) fn decode_access_bindings(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<TargetAccessCallBinding>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut bindings = Vec::new();
    for _ in 0..count {
        let input = decode_access_view_input(cursor)?;
        let argument_index = cursor.u32()?;
        let association = match cursor.byte()? {
            0 => TargetAccessCallAssociation::Captured {
                schema: decode_id(cursor, StructuralTypeId::new)?,
                carrier: decode_id(cursor, StructuralTypeId::new)?,
            },
            1 => TargetAccessCallAssociation::CallerFormal(decode_access_view_input(cursor)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        };
        bindings.push(TargetAccessCallBinding {
            input,
            argument_index,
            association,
        });
    }
    Ok(bindings)
}

pub(super) fn encode_argument_source(bytes: &mut Vec<u8>, source: &TargetStructuralArgumentSource) {
    match source {
        TargetStructuralArgumentSource::Placement(placement) => {
            bytes.push(0);
            encode_placement(bytes, placement);
        }
        TargetStructuralArgumentSource::EstablishedByteView { psi_operation } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
        TargetStructuralArgumentSource::BlockParameter { block, place } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
        }
        TargetStructuralArgumentSource::EstablishedPrimitiveLocal { psi_operation } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
        TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
        TargetStructuralArgumentSource::EstablishedElementView { psi_operation } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
    }
}

pub(super) fn decode_argument_source(
    cursor: &mut Cursor<'_>,
) -> Result<TargetStructuralArgumentSource, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(TargetStructuralArgumentSource::Placement(decode_placement(
            cursor,
        )?)),
        1 => Ok(TargetStructuralArgumentSource::EstablishedByteView {
            psi_operation: decode_id(cursor, OperationId::new)?,
        }),
        2 => Ok(TargetStructuralArgumentSource::BlockParameter {
            block: decode_id(cursor, BlockId::new)?,
            place: decode_id(cursor, PlaceId::new)?,
        }),
        3 => Ok(TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
            psi_operation: decode_id(cursor, OperationId::new)?,
        }),
        4 => Ok(TargetStructuralArgumentSource::StructuralHome {
            psi_operation: decode_id(cursor, OperationId::new)?,
        }),
        5 => Ok(TargetStructuralArgumentSource::EstablishedElementView {
            psi_operation: decode_id(cursor, OperationId::new)?,
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

pub(super) fn encode_call(bytes: &mut Vec<u8>, row: &SelectedCallContract) {
    crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
    match row.owner {
        target_operations::CallSiteOwner::Operation(operation) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
        target_operations::CallSiteOwner::InvokeNoNormal(edge) => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
        }
        target_operations::CallSiteOwner::ExternalCompletion(edge) => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
        }
        target_operations::CallSiteOwner::Installation { operation, role } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            bytes.push(match role {
                target_operations::InstallationCallRole::Validate => 0,
                target_operations::InstallationCallRole::Work => 1,
            });
        }
        target_operations::CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, action_ordinal.to_le_bytes());
        }
    }
    let call = &row.call;
    legalized_operations::encode_access_call_bindings(bytes, &call.access_bindings);
    encode_call_source(bytes, &call.source, true);
    bytes.extend_from_slice(&call.target.canonical_bytes());
    encode_call_plan(bytes, &call.call_plan);
    length(bytes, call.arguments.len());
    for argument in &call.arguments {
        match argument {
            LegalizedScalarArgument::Scalar { source, placement } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                encode_placement(bytes, placement);
            }
            LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
            } => {
                bytes.push(1);
                encode_semantic_argument(bytes, semantic);
                encode_target_argument(bytes, target);
                encode_runtime_indices(bytes, indices);
            }
        }
    }
    match &call.result_placement {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            encode_placement(bytes, value);
        }
    }
    match &call.structural_result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            encode_result(bytes, result);
        }
    }
    length(bytes, call.descriptor_origins.len());
    for origin in &call.descriptor_origins {
        match origin {
            legalized_operations::LegalizedDescriptorOrigin::Opaque => bytes.push(3),
            legalized_operations::LegalizedDescriptorOrigin::Empty => bytes.push(2),
            legalized_operations::LegalizedDescriptorOrigin::Argument {
                source,
                exact,
                indirect,
            } => {
                bytes.push(0);
                encode_semantic_argument(bytes, source);
                bytes.push(u8::from(*exact));
                bytes.push(u8::from(*indirect));
            }
            legalized_operations::LegalizedDescriptorOrigin::Literal { machine, place } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
        }
    }
    length(bytes, call.claim_transfers.len());
    for transfer in &call.claim_transfers {
        crate::codec_bytes::push_encoded(bytes, transfer.claim.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, transfer.argument_index.to_le_bytes());
        encode_path(bytes, &transfer.path);
    }
    encode_ids(
        bytes,
        call.requirement_obligations.iter().map(|id| id.get()),
    );
    let crash = terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
        .expect("canonical call crash routes");
    length(bytes, crash.len());
    bytes.extend_from_slice(&crash);
    encode_effect(bytes, row.effect);
    encode_ownership(bytes, &row.ownership);
}

pub(super) fn decode_call(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedCallContract, FixedViewCopyDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let owner = match cursor.byte()? {
        0 => target_operations::CallSiteOwner::Operation(decode_id(cursor, OperationId::new)?),
        3 => target_operations::CallSiteOwner::InvokeNoNormal(decode_id(cursor, EdgeId::new)?),
        2 => target_operations::CallSiteOwner::ExternalCompletion(decode_id(cursor, EdgeId::new)?),
        4 => target_operations::CallSiteOwner::Installation {
            operation: decode_id(cursor, OperationId::new)?,
            role: match cursor.byte()? {
                0 => target_operations::InstallationCallRole::Validate,
                1 => target_operations::InstallationCallRole::Work,
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            },
        },
        1 => target_operations::CallSiteOwner::CleanupAction {
            edge: decode_id(cursor, EdgeId::new)?,
            action_ordinal: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let access_bindings = decode_access_bindings(cursor)?;
    let source = decode_call_source(cursor, true)?;
    let target = match cursor.byte()? {
        0 => target_operations::NativeCallTarget::Machine(decode_id(cursor, MachineId::new)?),
        1 => target_operations::NativeCallTarget::Dynamic {
            selector: decode_id(cursor, ValueId::new)?,
            interface: cursor.u32()?,
            slot: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let call_plan = decode_call_plan(cursor)?;
    let count = cursor.length()?;
    let mut arguments = Vec::new();
    for _ in 0..count {
        arguments.push(match cursor.byte()? {
            0 => LegalizedScalarArgument::Scalar {
                source: decode_id(cursor, ValueId::new)?,
                placement: decode_placement(cursor)?,
            },
            1 => {
                let semantic = decode_semantic_argument(cursor)?;
                let target = decode_target_argument(cursor)?;
                let indices = decode_runtime_indices(cursor)?;
                LegalizedScalarArgument::Structural {
                    semantic,
                    target,
                    indices,
                }
            }
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        });
    }
    let result_placement = crate::cursor::optional(
        cursor,
        decode_placement,
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let structural_result = crate::cursor::optional(
        cursor,
        decode_result,
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let count = cursor.length()?;
    let mut descriptor_origins = Vec::new();
    for _ in 0..count {
        descriptor_origins.push(match cursor.byte()? {
            0 => {
                let source = decode_semantic_argument(cursor)?;
                let exact = match cursor.byte()? {
                    0 => false,
                    1 => true,
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                };
                let indirect = match cursor.byte()? {
                    0 => false,
                    1 => true,
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                };
                legalized_operations::LegalizedDescriptorOrigin::Argument {
                    source,
                    exact,
                    indirect,
                }
            }
            1 => legalized_operations::LegalizedDescriptorOrigin::Literal {
                machine: decode_id(cursor, MachineId::new)?,
                place: decode_id(cursor, PlaceId::new)?,
            },
            2 => legalized_operations::LegalizedDescriptorOrigin::Empty,
            3 => legalized_operations::LegalizedDescriptorOrigin::Opaque,
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        });
    }
    let count = cursor.length()?;
    let mut claim_transfers = Vec::new();
    for _ in 0..count {
        claim_transfers.push(ClaimTransfer {
            claim: decode_id(cursor, ClaimId::new)?,
            argument_index: cursor.u32()?,
            path: decode_path(cursor)?,
        });
    }
    let requirement_obligations = decode_ids(cursor, ObligationId::new)?;
    let length = cursor.length()?;
    let crash_continuations = terminal_codec::decode_crash_route_buckets(cursor.take(length)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidCrashContinuations)?;
    Ok(SelectedCallContract {
        instruction,
        owner,
        call: LegalizedScalarCall {
            descriptor_origins,
            access_bindings,
            source,
            target,
            call_plan,
            arguments,
            result_placement,
            structural_result,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        },
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
}

pub(super) fn encode_runtime_indices(
    bytes: &mut Vec<u8>,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) {
    length(bytes, indices.len());
    for index in indices {
        crate::codec_bytes::push_encoded(bytes, index.operand.value.get().to_le_bytes());
        encode_scalar(bytes, index.operand.scalar_type);
        crate::codec_bytes::push_encoded(bytes, index.stride.to_le_bytes());
        encode_option_u64(bytes, index.extent);
        crate::codec_bytes::push_encoded(bytes, index.obligation.get().to_le_bytes());
        bytes.extend_from_slice(&index.accepted_fact.bytes());
    }
}

pub(super) fn decode_runtime_indices(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, FixedViewCopyDecodeError> {
    decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(legalized_operations::LegalizedRuntimeIndexOperand {
            operand: abstract_operations::AbstractResult {
                value: decode_id(cursor, ValueId::new)?,
                scalar_type: decode_scalar(cursor)?,
            },
            stride: cursor.u32()?,
            extent: decode_option_u64(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        })
    })
}

pub(super) fn decode_call_plan(
    cursor: &mut Cursor<'_>,
) -> Result<CallPlan, FixedViewCopyDecodeError> {
    let policy = match cursor.byte()? {
        1 => CallingPolicy::MicrosoftX64,
        2 => CallingPolicy::SystemVAMD64,
        3 => CallingPolicy::Aapcs64,
        4 => CallingPolicy::LinuxSyscallX86_64,
        5 => CallingPolicy::LinuxSyscallAarch64,
        tag => return Err(FixedViewCopyDecodeError::UnknownCallingPolicy(tag)),
    };
    let parameters = decode_list(cursor, decode_placement)?;
    let result = crate::cursor::optional(
        cursor,
        decode_placement,
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let callback_materializations = decode_list(cursor, decode_callback)?;
    let ordinary_clobbers = decode_list(cursor, decode_machine_register)?;
    let stack_alignment = cursor.u16()?;
    let shadow_bytes = cursor.u16()?;
    let entry_control = match cursor.byte()? {
        1 => EntryControl::CallReturn,
        2 => EntryControl::SupervisorCall {
            number_register: decode_machine_register(cursor)?,
            immediate: cursor.u16()?,
        },
        3 => EntryControl::InterruptReturn,
        tag => return Err(FixedViewCopyDecodeError::UnknownEntryControl(tag)),
    };
    Ok(CallPlan {
        policy,
        parameters,
        result,
        callback_materializations,
        ordinary_clobbers: RegisterSet::new(ordinary_clobbers),
        stack_alignment,
        shadow_bytes,
        entry_control,
    })
}

fn decode_callback(
    cursor: &mut Cursor<'_>,
) -> Result<CallbackMaterialization, FixedViewCopyDecodeError> {
    let binder = decode_nominal(cursor, StaticMachineBinderId::new)?;
    Ok(CallbackMaterialization {
        binder,
        destination: decode_native_place(cursor)?,
    })
}

pub(super) fn decode_native_place(
    cursor: &mut Cursor<'_>,
) -> Result<NativePlace, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(NativePlace::Parameter(decode_nominal(
            cursor,
            NativeParameterId::new,
        )?)),
        2 => {
            let parameter = decode_nominal(cursor, NativeParameterId::new)?;
            let layout = decode_nominal(cursor, LayoutPlanId::new)?;
            let field_path =
                decode_list(cursor, |cursor| decode_nominal(cursor, LayoutSlotId::new))?;
            Ok(NativePlace::Field {
                parameter,
                layout,
                field_path,
            })
        }
        tag => Err(FixedViewCopyDecodeError::UnknownNativePlace(tag)),
    }
}

pub(super) fn decode_placement(
    cursor: &mut Cursor<'_>,
) -> Result<ValuePlacement, FixedViewCopyDecodeError> {
    let shape = decode_shape(cursor)?;
    let locations = decode_list(cursor, |cursor| {
        Ok(match cursor.byte()? {
            1 => ValueLocation::Register {
                register: decode_machine_register(cursor)?,
                value_byte_offset: cursor.u32()?,
                byte_size: cursor.u32()?,
            },
            2 => ValueLocation::Stack {
                stack_byte_offset: cursor.u32()?,
                value_byte_offset: cursor.u32()?,
                byte_size: cursor.u32()?,
                alignment: cursor.u16()?,
            },
            3 => ValueLocation::Indirect {
                pointer: decode_indirect_pointer(cursor)?,
                copy_stack_byte_offset: match cursor.byte()? {
                    0 => None,
                    1 => Some(cursor.u32()?),
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                },
                byte_size: cursor.u32()?,
                alignment: cursor.u16()?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueLocation(tag)),
        })
    })?;
    Ok(ValuePlacement { shape, locations })
}

fn decode_indirect_pointer(
    cursor: &mut Cursor<'_>,
) -> Result<IndirectPointerLocation, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(IndirectPointerLocation::Register(decode_machine_register(
            cursor,
        )?)),
        2 => Ok(IndirectPointerLocation::Stack {
            stack_byte_offset: cursor.u32()?,
            alignment: cursor.u16()?,
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownIndirectPointer(tag)),
    }
}

pub(super) fn decode_shape(
    cursor: &mut Cursor<'_>,
) -> Result<ValueShape, FixedViewCopyDecodeError> {
    let class = match cursor.byte()? {
        1 => ValueClass::Integer,
        2 => ValueClass::Float,
        5 => ValueClass::BorrowedReference,
        6 => ValueClass::BorrowedScalarReference,
        3 => ValueClass::HomogeneousFloatAggregate {
            members: cursor.byte()?,
        },
        4 => ValueClass::SystemVAggregate {
            first: decode_eightbyte_class(cursor)?,
            second: decode_eightbyte_class(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownValueClass(tag)),
    };
    Ok(ValueShape {
        class,
        byte_size: cursor.u32()?,
        alignment: cursor.u16()?,
    })
}

fn decode_eightbyte_class(
    cursor: &mut Cursor<'_>,
) -> Result<SystemVEightbyteClass, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(SystemVEightbyteClass::Integer),
        2 => Ok(SystemVEightbyteClass::Sse),
        tag => Err(FixedViewCopyDecodeError::UnknownSystemVEightbyteClass(tag)),
    }
}

fn decode_machine_register(
    cursor: &mut Cursor<'_>,
) -> Result<MachineRegister, FixedViewCopyDecodeError> {
    let tag = cursor.byte()?;
    let payload = cursor.byte()?;
    if tag <= 15 && payload != 0 {
        return Err(FixedViewCopyDecodeError::InvalidMachineRegisterPayload(
            payload,
        ));
    }
    Ok(match tag {
        0 => MachineRegister::X86Rax,
        1 => MachineRegister::X86Rcx,
        2 => MachineRegister::X86Rdx,
        3 => MachineRegister::X86Rbx,
        4 => MachineRegister::X86Rsp,
        5 => MachineRegister::X86Rbp,
        6 => MachineRegister::X86Rsi,
        7 => MachineRegister::X86Rdi,
        8 => MachineRegister::X86R8,
        9 => MachineRegister::X86R9,
        10 => MachineRegister::X86R10,
        11 => MachineRegister::X86R11,
        12 => MachineRegister::X86R12,
        13 => MachineRegister::X86R13,
        14 => MachineRegister::X86R14,
        15 => MachineRegister::X86R15,
        16 => MachineRegister::X86Xmm(payload),
        17 => MachineRegister::Aarch64X(payload),
        18 => MachineRegister::Aarch64V(payload),
        tag => return Err(FixedViewCopyDecodeError::UnknownMachineRegister(tag)),
    })
}

fn decode_nominal<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<T, FixedViewCopyDecodeError> {
    let raw = cursor.u64()?;
    constructor(raw).ok_or(FixedViewCopyDecodeError::InvalidNominalId(raw))
}

pub(super) fn encode_normalized_foreign_call(
    bytes: &mut Vec<u8>,
    row: &SelectedNormalizedForeignCall,
) {
    crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
    row.invocation_site.encode_identity(bytes);
    let call = &row.call;
    crate::codec_bytes::push_encoded(bytes, call.boundary.get().to_le_bytes());
    encode_provider_execution(bytes, call.provider_execution);
    encode_call_binding(bytes, &call.binding);
    length(bytes, call.scalar_arguments.len());
    for argument in &call.scalar_arguments {
        crate::codec_bytes::push_encoded(bytes, argument.parameter_index.to_le_bytes());
        encode_scalar_source(bytes, argument.source);
        encode_placement(bytes, &argument.placement);
    }
    length(bytes, call.structural_arguments.len());
    for argument in &call.structural_arguments {
        encode_target_argument(bytes, argument);
    }
    length(bytes, call.structural_indices.len());
    for indices in &call.structural_indices {
        encode_runtime_indices(bytes, indices);
    }
    match &call.result {
        target_operations::TargetBoundaryResult::Unit => bytes.push(0),
        target_operations::TargetBoundaryResult::Scalar(home) => {
            bytes.push(1);
            encode_scalar_home(bytes, home);
        }
        target_operations::TargetBoundaryResult::Structural(home) => {
            bytes.push(2);
            match &home.origin {
                TargetStructuralHomeOrigin::OperationResult { operation, result } => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                    encode_result(bytes, result);
                }
                TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
                    encode_semantic_parameter(bytes, declaration, true);
                }
            }
            match &home.layout {
                TargetStructuralHomeLayout::Aggregate(shape) => {
                    bytes.push(0);
                    encode_shape(bytes, *shape);
                }
                TargetStructuralHomeLayout::Sum(layout) => {
                    bytes.push(1);
                    encode_layout(bytes, layout);
                }
            }
        }
    }
    length(bytes, call.callbacks.len());
    for callback in &call.callbacks {
        encode_native_callback_argument(bytes, callback);
    }
    match call.descriptor_input {
        None => bytes.push(0),
        Some(guard) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(
                bytes,
                (guard.count_scalar_argument as u64).to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(
                bytes,
                (guard.structural_argument as u64).to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(bytes, guard.max_bytes.to_le_bytes());
            encode_scalar(bytes, ScalarType::Integer(guard.count_type));
        }
    }
    encode_effect(bytes, row.effect);
    encode_ownership(bytes, &row.ownership);
}

pub(super) fn decode_normalized_foreign_call(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedNormalizedForeignCall, FixedViewCopyDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let invocation_site = decode_invocation_site(cursor)?;
    let boundary = decode_id(cursor, BoundaryMachineId::new)?;
    let provider_execution = decode_provider_execution(cursor)?;
    let binding = decode_call_binding(cursor)?;
    let scalar_arguments = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(TargetUnitScalarCallArgument {
            parameter_index: cursor.u32()?,
            source: decode_scalar_source(cursor)?,
            placement: decode_placement(cursor)?,
        })
    })?;
    let structural_arguments = decode_list(cursor, decode_target_argument)?;
    let structural_indices = decode_list(cursor, decode_runtime_indices)?;
    let result = match cursor.byte()? {
        0 => target_operations::TargetBoundaryResult::Unit,
        1 => target_operations::TargetBoundaryResult::Scalar(decode_scalar_home(cursor)?),
        2 => {
            let origin = match cursor.byte()? {
                0 => TargetStructuralHomeOrigin::OperationResult {
                    operation: decode_id(cursor, OperationId::new)?,
                    result: decode_result(cursor)?,
                },
                1 => TargetStructuralHomeOrigin::BlockParameter {
                    block: decode_id(cursor, BlockId::new)?,
                    declaration: decode_semantic_parameter(cursor, true)?,
                },
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            };
            let layout = match cursor.byte()? {
                0 => TargetStructuralHomeLayout::Aggregate(decode_shape(cursor)?),
                1 => TargetStructuralHomeLayout::Sum(decode_layout(cursor)?),
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            };
            target_operations::TargetBoundaryResult::Structural(
                target_operations::TargetStructuralHomeRequirement { origin, layout },
            )
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let callbacks = decode_list(cursor, decode_native_callback_argument)?;
    let descriptor_input = match cursor.byte()? {
        0 => None,
        1 => Some(target_operations::DescriptorInputGuard {
            count_scalar_argument: usize::try_from(cursor.u64()?)
                .map_err(|_| FixedViewCopyDecodeError::LengthOverflow)?,
            structural_argument: usize::try_from(cursor.u64()?)
                .map_err(|_| FixedViewCopyDecodeError::LengthOverflow)?,
            max_bytes: cursor.u64()?,
            count_type: match decode_scalar(cursor)? {
                ScalarType::Integer(integer) => integer,
                _ => return Err(FixedViewCopyDecodeError::InvalidIntegerType),
            },
        }),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    Ok(SelectedNormalizedForeignCall {
        instruction,
        invocation_site,
        call: LegalizedNormalizedForeignCall {
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            structural_arguments,
            structural_indices,
            result,
            callbacks,
            descriptor_input,
        },
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
}

fn decode_native_callback_argument(
    cursor: &mut Cursor<'_>,
) -> Result<TargetNativeCallbackArgument, FixedViewCopyDecodeError> {
    let terminal_operation = decode_id(cursor, OperationId::new)?;
    let placement_index =
        usize::try_from(cursor.u64()?).map_err(|_| FixedViewCopyDecodeError::LengthOverflow)?;
    let callback_function = decode_machine_function_identity(cursor)?;
    let application = NativeParameterApplication {
        parameter: decode_id(cursor, NativeParameterId::new)?,
        native_ordinal: cursor.u32()?,
        shape: decode_shape(cursor)?,
        placement: decode_placement(cursor)?,
    };
    let destination = decode_native_place(cursor)?;
    let field_placements = crate::cursor::optional(
        cursor,
        decode_callback_placements,
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let registrar_boundary_entry_plan = BoundaryEntryPlan {
        call: decode_call_plan(cursor)?,
        state: decode_state_plan(cursor)?,
    };
    let binders = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(CallbackBinderRequirement {
            binder: decode_id(cursor, StaticMachineBinderId::new)?,
            requirement: decode_id(cursor, CallbackRequirementId::new)?,
        })
    })?;
    let demands = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(NativeCallbackDemand {
            destination: decode_native_place(cursor)?,
            requirement: decode_id(cursor, CallbackRequirementId::new)?,
        })
    })?;
    let registrar_application_commitment = cursor.array()?;
    let registrar_invokes_callback = decode_bool(cursor)?;
    Ok(TargetNativeCallbackArgument {
        registrar_invokes_callback,
        terminal_operation,
        placement_index,
        callback_function,
        application,
        destination,
        field_placements,
        registrar_boundary_entry_plan,
        registrar_context: CallbackMaterializationContext { binders, demands },
        registrar_application_commitment,
    })
}

fn decode_machine_function_identity(
    cursor: &mut Cursor<'_>,
) -> Result<MachineFunctionIdentity, FixedViewCopyDecodeError> {
    MachineFunctionIdentity::from_bytes(cursor.array()?)
        .ok_or(FixedViewCopyDecodeError::InvalidMachineFunctionIdentity)
}

fn decode_call_binding(
    cursor: &mut Cursor<'_>,
) -> Result<NormalizedForeignCallBinding, FixedViewCopyDecodeError> {
    let target_name = decode_string(cursor)?;
    if target_name.is_empty() {
        let target_name = decode_string(cursor)?;
        let target = TargetProfile::from_canonical_target_name(&target_name)
            .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
        let number = cursor.i64()?;
        let provider_plan_report_identity = cursor.u64()?;
        let provider_plan_commitment = cursor.array::<32>()?;
        let requirement_identity = decode_string(cursor)?;
        let call = decode_call_plan(cursor)?;
        let state = decode_state_plan(cursor)?;
        let signature = calling_conventions::CallSignature {
            parameters: call
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: call.result.as_ref().map(|result| result.shape),
        };
        let binding = NormalizedForeignCallBinding {
            boundary_entry_plan: BoundaryEntryPlan { call, state },
            mechanism: target_operations::NormalizedForeignCallMechanism::SystemCall(
                target_operations::NormalizedSystemCallBinding {
                    target,
                    number,
                    provider_plan_report_identity,
                    provider_plan_commitment,
                    requirement_identity,
                },
            ),
        };
        if !binding.has_valid_entry_mechanism()
            || validate_boundary_entry_plan(binding.boundary_entry_plan.clone(), &signature)
                .is_err()
        {
            return Err(FixedViewCopyDecodeError::InvalidSystemCallBinding);
        }
        return Ok(binding);
    }
    let mut component = None;
    let (locator, table) = if target_name == "@table" {
        let target_name = decode_string(cursor)?;
        let target = TargetProfile::from_canonical_target_name(&target_name)
            .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
        (None, Some((target, cursor.u32()?, decode_bool(cursor)?)))
    } else if target_name == "@component" {
        let target_name = decode_string(cursor)?;
        let target = TargetProfile::from_canonical_target_name(&target_name)
            .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
        let vocabulary_marker = terminal_psi::VocabularyMarker::new(cursor.u16()?)
            .ok_or(FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
        let provider = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array::<32>()?),
        };
        let candidate = decode_id(cursor, MachineId::new)?;
        let application_commitment = cursor.array::<32>()?;
        let locator =
            NormalizedComponentLocator::new(target, cursor.array::<32>()?, cursor.array::<32>()?)
                .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
        component = Some((locator, provider, candidate, application_commitment));
        (None, None)
    } else {
        (Some(decode_locator_for_target(cursor, &target_name)?), None)
    };
    let call = decode_call_plan(cursor)?;
    let state = decode_state_plan(cursor)?;
    let report_identity = cursor.u64()?;
    let commitment = cursor.array::<32>()?;
    let provider_plan_report_identity = cursor.u64()?;
    let provider_plan_commitment =
        SameStackProviderPlanCommitment::from_digest(cursor.array::<32>()?);
    let requirement_identity = TerminalIdentityIndex(cursor.u32()?);
    let receipt = SameStackContributionAdmissionReceiptId::from_normalized_identity(cursor.u64()?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidSameStackContribution)?;
    let candidate = SameStackContributionAdmissionCandidate {
        provider_plan_report_identity,
        provider_plan_commitment,
        requirement_identity,
        receipt,
        bytes: cursor.u64()?,
        alignment: cursor.u64()?,
    };
    let contribution = admit_same_stack_contribution(
        candidate.clone(),
        provider_plan_report_identity,
        provider_plan_commitment,
        candidate.requirement_identity,
    )
    .map_err(|_| FixedViewCopyDecodeError::InvalidSameStackContribution)?;
    if contribution.report_identity().normalized_identity() != report_identity
        || contribution.commitment().as_bytes() != commitment
    {
        return Err(FixedViewCopyDecodeError::InvalidSameStackContribution);
    }
    let mechanism = if let Some((locator, provider, candidate, application_commitment)) = component
    {
        target_operations::NormalizedForeignCallMechanism::Component {
            locator,
            provider,
            candidate,
            application_commitment,
            same_stack_contribution: contribution,
        }
    } else {
        match (locator, table) {
            (Some(locator), None) => target_operations::NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution: contribution,
            },
            (None, Some((target, byte_offset, dispatch_only))) => {
                target_operations::NormalizedForeignCallMechanism::Table {
                    target,
                    byte_offset,
                    dispatch_only,
                    same_stack_contribution: contribution,
                }
            }
            _ => return Err(FixedViewCopyDecodeError::InvalidForeignTargetProfile),
        }
    };
    let binding = NormalizedForeignCallBinding {
        boundary_entry_plan: BoundaryEntryPlan { call, state },
        mechanism,
    };
    if matches!(
        binding.mechanism,
        target_operations::NormalizedForeignCallMechanism::Component { .. }
    ) && !binding.has_valid_entry_mechanism()
    {
        return Err(FixedViewCopyDecodeError::InvalidForeignTargetProfile);
    }
    Ok(binding)
}

fn decode_callback_placements(
    cursor: &mut Cursor<'_>,
) -> Result<calling_conventions::PrivateCallbackPlacements, FixedViewCopyDecodeError> {
    use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
    let placements = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(match cursor.byte()? {
            0 => LayoutPlacementReport::At {
                offset: cursor.u64()?,
            },
            1 => LayoutPlacementReport::IntegerAt {
                offset: cursor.u64()?,
                stored_width: cursor.u64()?,
                interpretation: match cursor.byte()? {
                    0 => IntegerInterpretation::Signed,
                    1 => IntegerInterpretation::Unsigned,
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                },
            },
            2 => LayoutPlacementReport::Bits {
                container: cursor.u64()?,
                container_width: cursor.u64()?,
                destination_lsb: cursor.u64()?,
                source_lsb: cursor.u64()?,
                width: cursor.u64()?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        })
    })?;
    Ok(calling_conventions::PrivateCallbackPlacements::new(
        &placements,
    ))
}

#[cfg(test)]
fn decode_locator(
    cursor: &mut Cursor<'_>,
) -> Result<target::NormalizedForeignLocator, FixedViewCopyDecodeError> {
    let target_name = decode_string(cursor)?;
    decode_locator_for_target(cursor, &target_name)
}

fn decode_locator_for_target(
    cursor: &mut Cursor<'_>,
    target_name: &str,
) -> Result<target::NormalizedForeignLocator, FixedViewCopyDecodeError> {
    let profile = TargetProfile::from_canonical_target_name(target_name)
        .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
    let candidate = match cursor.byte()? {
        1 => ForeignLocatorCandidate::PeByName {
            library: decode_coordinate(cursor)?,
            export: decode_coordinate(cursor)?,
        },
        2 => ForeignLocatorCandidate::PeByOrdinal {
            library: decode_coordinate(cursor)?,
            ordinal: cursor.u16()?,
        },
        3 => ForeignLocatorCandidate::ElfVersioned {
            object: decode_coordinate(cursor)?,
            symbol: decode_coordinate(cursor)?,
            version: decode_coordinate(cursor)?,
        },
        4 => ForeignLocatorCandidate::MachODylibSymbol {
            install_name: decode_coordinate(cursor)?,
            symbol: decode_coordinate(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownForeignLocatorCase(tag)),
    };
    normalize_foreign_locator(candidate, profile)
        .map_err(|_| FixedViewCopyDecodeError::InvalidForeignLocator)
}

fn decode_coordinate(cursor: &mut Cursor<'_>) -> Result<Vec<u8>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    Ok(cursor.take(count)?.to_vec())
}

fn decode_state_plan(cursor: &mut Cursor<'_>) -> Result<StatePlan, FixedViewCopyDecodeError> {
    let initial_regime = match cursor.byte()? {
        1 => MachineRegime::X86Long64,
        2 => MachineRegime::Aarch64A64 {
            exception_level: cursor.byte()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownMachineRegime(tag)),
    };
    let interrupted_state = decode_state_set(cursor)?;
    let saved_state = decode_state_set(cursor)?;
    let restored_state = decode_state_set(cursor)?;
    let permitted_transitive_use = decode_state_set(cursor)?;
    let stack = match cursor.byte()? {
        1 => EntryStack::Interrupted,
        2 => EntryStack::Dedicated {
            class: cursor.u16()?,
        },
        3 => EntryStack::ProviderSelected,
        tag => return Err(FixedViewCopyDecodeError::UnknownEntryStack(tag)),
    };
    let preemption = match cursor.byte()? {
        1 => Preemption::NotApplicable,
        2 => Preemption::Masked,
        3 => Preemption::Nestable {
            maximum_depth: cursor.u16()?,
        },
        4 => Preemption::ProviderDefined,
        tag => return Err(FixedViewCopyDecodeError::UnknownPreemption(tag)),
    };
    Ok(StatePlan {
        initial_regime,
        interrupted_state,
        saved_state,
        restored_state,
        permitted_transitive_use,
        stack,
        preemption,
    })
}

fn decode_state_set(cursor: &mut Cursor<'_>) -> Result<MachineStateSet, FixedViewCopyDecodeError> {
    let bits = cursor.u16()?;
    if bits & !MACHINE_STATE_SET_BITS != 0 {
        return Err(FixedViewCopyDecodeError::InvalidMachineStateSet(bits));
    }
    const STATES: [MachineState; 9] = [
        MachineState::GeneralRegisters,
        MachineState::VectorRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::SegmentState,
        MachineState::ControlState,
        MachineState::DebugState,
        MachineState::ExtendedState,
    ];
    Ok(MachineStateSet::new(
        STATES
            .into_iter()
            .enumerate()
            .filter(move |(index, _)| bits & (1 << index) != 0)
            .map(|(_, state)| state),
    ))
}

fn encode_scalar_home(bytes: &mut Vec<u8>, home: &TargetUnitScalarHomeRequirement) {
    crate::codec_bytes::push_encoded(bytes, home.defining_operation.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, home.source_value.get().to_le_bytes());
    encode_scalar(bytes, home.scalar_type);
    encode_shape(bytes, home.shape);
}

fn decode_scalar_home(
    cursor: &mut Cursor<'_>,
) -> Result<TargetUnitScalarHomeRequirement, FixedViewCopyDecodeError> {
    Ok(TargetUnitScalarHomeRequirement {
        defining_operation: decode_id(cursor, OperationId::new)?,
        source_value: decode_id(cursor, ValueId::new)?,
        scalar_type: decode_scalar(cursor)?,
        shape: decode_shape(cursor)?,
    })
}

pub(super) fn encode_scalar_source(bytes: &mut Vec<u8>, source: TargetUnitScalarArgumentSource) {
    match source {
        TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, defining_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, bits.to_le_bytes());
                }
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, bits.to_le_bytes());
                }
            }
        }
        TargetUnitScalarArgumentSource::BlockParameter(parameter) => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, parameter.block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, parameter.value.get().to_le_bytes());
            encode_scalar(bytes, parameter.scalar_type);
        }
        TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, parameter_index.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            encode_scalar(bytes, scalar_type);
        }
        TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, defining_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            encode_scalar(bytes, ScalarType::Integer(scalar_type));
            encode_integer(bytes, value);
        }
        TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, defining_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            bytes.push(u8::from(value));
        }
        TargetUnitScalarArgumentSource::Home(home) => {
            bytes.push(6);
            encode_scalar_home(bytes, &home);
        }
    }
}

pub(super) fn decode_scalar_source(
    cursor: &mut Cursor<'_>,
) -> Result<TargetUnitScalarArgumentSource, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        1 => TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation: decode_id(cursor, OperationId::new)?,
            source_value: decode_id(cursor, ValueId::new)?,
            value: match cursor.byte()? {
                1 => semantic_vocabulary::IeeeFloatValue::Binary32(cursor.u32()?),
                2 => semantic_vocabulary::IeeeFloatValue::Binary64(cursor.u64()?),
                tag => return Err(FixedViewCopyDecodeError::UnknownIeeeFloatFormat(tag)),
            },
        },
        2 => TargetUnitScalarArgumentSource::BlockParameter(TargetScalarBlockValue {
            block: decode_id(cursor, BlockId::new)?,
            value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        }),
        3 => TargetUnitScalarArgumentSource::Parameter {
            parameter_index: cursor.u32()?,
            source_value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        },
        4 => TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation: decode_id(cursor, OperationId::new)?,
            source_value: decode_id(cursor, ValueId::new)?,
            scalar_type: match decode_scalar(cursor)? {
                ScalarType::Integer(integer) => integer,
                _ => return Err(FixedViewCopyDecodeError::InvalidIntegerType),
            },
            value: decode_integer(cursor)?,
        },
        5 => TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation: decode_id(cursor, OperationId::new)?,
            source_value: decode_id(cursor, ValueId::new)?,
            value: match cursor.byte()? {
                0 => false,
                1 => true,
                tag => return Err(FixedViewCopyDecodeError::UnknownBoolean(tag)),
            },
        },
        6 => TargetUnitScalarArgumentSource::Home(decode_scalar_home(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownForeignScalarSource(tag)),
    })
}

pub(super) fn encode_provider(
    bytes: &mut Vec<u8>,
    provider: &ProviderCandidateConformance,
    retain_projected_qualifications: bool,
) {
    crate::codec_bytes::push_encoded(bytes, provider.boundary.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.requirement_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.source_requirement_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.declaration_application.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.receiving_application.0.to_le_bytes());
    length(bytes, provider.receiving_region_arguments.len());
    for region in &provider.receiving_region_arguments {
        crate::codec_bytes::push_encoded(bytes, region.0.to_le_bytes());
    }
    crate::codec_bytes::push_encoded(bytes, provider.provider_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(
        bytes,
        provider.provider_declaration.identity().0.to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(bytes, provider.candidate_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.source_candidate_identity.0.to_le_bytes());
    crate::codec_bytes::option(
        bytes,
        provider.source_candidate_package_identity,
        |bytes, package| {
            bytes.extend_from_slice(&package.digest());
        },
    );
    crate::codec_bytes::push_encoded(bytes, provider.candidate.get().to_le_bytes());
    length(bytes, provider.signature.parameters.len());
    for parameter in &provider.signature.parameters {
        crate::codec_bytes::push_encoded(bytes, parameter.position.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, parameter.structural_type.get().to_le_bytes());
        encode_multiplicity(bytes, parameter.multiplicity);
        encode_access(bytes, parameter.access);
        encode_ids(
            bytes,
            parameter.qualifications.iter().map(|value| value.get()),
        );
        encode_projected(
            bytes,
            &parameter.projected_qualifications,
            retain_projected_qualifications,
        );
    }
    length(bytes, provider.refinement.positional_parameters.len());
    for parameter in &provider.refinement.positional_parameters {
        crate::codec_bytes::push_encoded(bytes, parameter.boundary_index.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, parameter.candidate_index.to_le_bytes());
    }
    length(bytes, provider.refinement.required_domains.len());
    for requirement in &provider.refinement.required_domains {
        crate::codec_bytes::push_encoded(bytes, requirement.argument_index.to_le_bytes());
        encode_path(bytes, &requirement.path);
        crate::codec_bytes::push_encoded(bytes, requirement.domain.get().to_le_bytes());
    }
    encode_ids(
        bytes,
        provider
            .refinement
            .realized_service_ceiling
            .iter()
            .map(|value| value.get()),
    );
}

pub(super) fn decode_provider(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<ProviderCandidateConformance, FixedViewCopyDecodeError> {
    let boundary = decode_id(cursor, BoundaryMachineId::new)?;
    let requirement_identity = TerminalIdentityIndex(cursor.u32()?);
    let source_requirement_identity = TerminalIdentityIndex(cursor.u32()?);
    let declaration_application = TerminalIdentityIndex(cursor.u32()?);
    let receiving_application = TerminalIdentityIndex(cursor.u32()?);
    let receiving_region_arguments =
        decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
            Ok(TerminalIdentityIndex(cursor.u32()?))
        })?;
    let provider_identity = TerminalIdentityIndex(cursor.u32()?);
    let provider_declaration =
        terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(cursor.u32()?));
    let candidate_identity = TerminalIdentityIndex(cursor.u32()?);
    let source_candidate_identity = TerminalIdentityIndex(cursor.u32()?);
    let source_candidate_package_identity = crate::cursor::optional(
        cursor,
        |cursor| {
            semantic_vocabulary::PackageKeyIdentity::from_digest(cursor.array()?)
                .ok_or(FixedViewCopyDecodeError::InvalidProviderExecution)
        },
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let candidate = decode_id(cursor, MachineId::new)?;
    let parameters = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(ProviderSignatureParameter {
            position: cursor.u32()?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
            multiplicity: decode_multiplicity(cursor)?,
            access: decode_access(cursor)?,
            qualifications: decode_ids(cursor, StructuralDomainId::new)?,
            projected_qualifications: decode_projected(cursor, retain_projected_qualifications)?,
        })
    })?;
    let positional_parameters = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(ProviderParameterRefinement {
            boundary_index: cursor.u32()?,
            candidate_index: cursor.u32()?,
        })
    })?;
    let required_domains = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(StructuralDomainRequirement {
            argument_index: cursor.u32()?,
            path: decode_qualification_path(cursor)?,
            domain: decode_id(cursor, StructuralDomainId::new)?,
        })
    })?;
    Ok(ProviderCandidateConformance {
        boundary,
        requirement_identity,
        source_requirement_identity,
        declaration_application,
        receiving_application,
        receiving_region_arguments,
        provider_identity,
        provider_declaration,
        candidate_identity,
        source_candidate_identity,
        source_candidate_package_identity,
        candidate,
        signature: ProviderSignature { parameters },
        refinement: ProviderRefinement {
            positional_parameters,
            required_domains,
            realized_service_ceiling: decode_ids(cursor, ServiceId::new)?,
        },
    })
}

pub(super) fn encode_read_result_and_layout(
    bytes: &mut Vec<u8>,
    result: &StructuralOperationResult,
    layout: &ConventionalSumLayout,
) {
    encode_result(bytes, result);
    encode_layout(bytes, layout);
}

pub(super) fn encode_layout(bytes: &mut Vec<u8>, layout: &ConventionalSumLayout) {
    encode_shape(bytes, layout.shape);
    crate::codec_bytes::push_encoded(bytes, layout.tag_byte_offset.to_le_bytes());
    encode_shape(bytes, layout.tag_shape);
    encode_packed_fields(bytes, &layout.common_fields);
    crate::codec_bytes::push_encoded(bytes, layout.payload_byte_offset.to_le_bytes());
    length(bytes, layout.cases.len());
    for case in &layout.cases {
        encode_packed_fields(bytes, &case.fields);
    }
}

pub(super) fn encode_result(bytes: &mut Vec<u8>, result: &StructuralOperationResult) {
    crate::codec_bytes::push_encoded(bytes, result.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected(bytes, &result.projected_qualifications, true);
    encode_projected(bytes, &result.additional_input_qualifications, true);
    length(bytes, result.claims.len());
    for claim in &result.claims {
        crate::codec_bytes::push_encoded(bytes, claim.claim.get().to_le_bytes());
        encode_path(bytes, &claim.path);
    }
}

pub(super) fn decode_result(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralOperationResult, FixedViewCopyDecodeError> {
    let place = decode_id(cursor, PlaceId::new)?;
    let structural_type = decode_id(cursor, StructuralTypeId::new)?;
    let multiplicity = decode_multiplicity(cursor)?;
    let qualifications = decode_ids(cursor, StructuralDomainId::new)?;
    let projected_qualifications = decode_projected(cursor, true)?;
    let additional_input_qualifications = decode_projected(cursor, true)?;
    let claims = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(StructuralResultClaimBinding {
            claim: decode_id(cursor, ClaimId::new)?,
            path: decode_path(cursor)?,
        })
    })?;
    Ok(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type,
        multiplicity,
        qualifications,
        projected_qualifications,
        additional_input_qualifications,
        claims,
    })
}

pub(super) fn decode_read_result_and_layout(
    cursor: &mut Cursor<'_>,
) -> Result<(StructuralOperationResult, ConventionalSumLayout), FixedViewCopyDecodeError> {
    let result = decode_result(cursor)?;
    Ok((result, decode_layout(cursor)?))
}

pub(super) fn decode_layout(
    cursor: &mut Cursor<'_>,
) -> Result<ConventionalSumLayout, FixedViewCopyDecodeError> {
    let shape = decode_shape(cursor)?;
    let tag_byte_offset = cursor.u32()?;
    let tag_shape = decode_shape(cursor)?;
    let common_fields = decode_packed_fields(cursor)?;
    let payload_byte_offset = cursor.u32()?;
    let cases = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(ConventionalSumCaseLayout {
            fields: decode_packed_fields(cursor)?,
        })
    })?;
    Ok(ConventionalSumLayout {
        shape,
        tag_byte_offset,
        tag_shape,
        common_fields,
        payload_byte_offset,
        cases,
    })
}

fn encode_packed_fields(bytes: &mut Vec<u8>, fields: &[PackedFieldLayout]) {
    length(bytes, fields.len());
    for field in fields {
        encode_shape(bytes, field.shape);
        crate::codec_bytes::push_encoded(bytes, field.byte_offset.to_le_bytes());
    }
}

fn decode_packed_fields(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<PackedFieldLayout>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| {
        Ok(PackedFieldLayout {
            shape: decode_shape(cursor)?,
            byte_offset: cursor.u32()?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BlockId, BoundaryMachineId, CallingPolicy, ClaimId, Cursor, FixedViewCopyDecodeError,
        IndirectPointerLocation, LegalizedNormalizedForeignCall, MachineRegister,
        NormalizedForeignCallBinding, OperationId, PlaceId,
        SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
        SameStackProviderPlanCommitment, ScalarType, SelectedInstructionId,
        SelectedNormalizedForeignCall, StructuralDomainId, StructuralOperationResult,
        StructuralResultClaimBinding, StructuralTypeId, TargetProfile,
        TargetStructuralArgumentSource, TargetUnitScalarArgumentSource,
        TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement, ValueClass, ValueId,
        ValueLocation, ValuePlacement, ValueShape, admit_same_stack_contribution,
        decode_argument_source, decode_call_binding, decode_locator,
        decode_machine_function_identity, decode_normalized_foreign_call, decode_placement,
        decode_read_result_and_layout, decode_scalar_source, decode_state_plan,
        encode_argument_source, encode_call_binding, encode_call_plan,
        encode_normalized_foreign_call, encode_read_result_and_layout, length,
        normalize_foreign_locator,
    };
    use crate::recovery::structural_declarations::encode_string;
    use calling_conventions::{CallSignature, encode_state_plan_identity};
    use legalized_operations::encode_locator;
    use optimization_unit::EffectLink;
    use semantic_vocabulary::{IntegerSign, IntegerType};
    use target::ForeignLocatorCandidate;
    use target_operations::ProviderPlanReportIdentity;

    const REQUIREMENT_INDEX: u32 = 1;

    #[test]
    fn argument_source_round_trip_preserves_origin_and_establishment() {
        for source in [
            TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(313).unwrap(),
            },
            TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: OperationId::new(313).unwrap(),
            },
            TargetStructuralArgumentSource::BlockParameter {
                block: BlockId::new(211).unwrap(),
                place: PlaceId::new(213).unwrap(),
            },
            TargetStructuralArgumentSource::Placement(ValuePlacement {
                shape: ValueShape {
                    byte_size: 16,
                    alignment: 8,
                    class: ValueClass::BorrowedReference,
                },
                locations: Vec::new(),
            }),
            TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: OperationId::new(313).unwrap(),
            },
            TargetStructuralArgumentSource::EstablishedElementView {
                psi_operation: OperationId::new(313).unwrap(),
            },
        ] {
            let mut encoded = Vec::new();
            encode_argument_source(&mut encoded, &source);
            let mut cursor = Cursor::new(&encoded);
            assert_eq!(decode_argument_source(&mut cursor).unwrap(), source);
            assert_eq!(cursor.remaining(), 0);
            for end in 0..encoded.len() {
                assert!(decode_argument_source(&mut Cursor::new(&encoded[..end])).is_err());
            }
        }
    }

    #[test]
    fn argument_source_rejects_unknown_kind_and_absent_establishment() {
        assert!(decode_argument_source(&mut Cursor::new(&[6])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[4, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[3, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[2])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[1, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        let mut first = Vec::new();
        let mut second = Vec::new();
        for (bytes, operation) in [(&mut first, 313), (&mut second, 317)] {
            encode_argument_source(
                bytes,
                &TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: OperationId::new(operation).unwrap(),
                },
            );
        }
        assert_ne!(first, second);
        let mut local = Vec::new();
        encode_argument_source(
            &mut local,
            &TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: OperationId::new(313).unwrap(),
            },
        );
        assert_ne!(
            first, local,
            "identical producer ID cannot equate a view and primitive local"
        );
        let mut aggregate = Vec::new();
        encode_argument_source(
            &mut aggregate,
            &TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(313).unwrap(),
            },
        );
        assert_ne!(aggregate, local);
        assert_ne!(aggregate, first);
    }

    #[test]
    fn wide_shape_and_fragment_offsets_round_trip_without_truncation() {
        for placement in [
            ValuePlacement {
                shape: ValueShape::borrowed_reference(u32::MAX, 8),
                locations: vec![ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
                    copy_stack_byte_offset: None,
                    byte_size: u32::MAX,
                    alignment: 8,
                }],
            },
            ValuePlacement {
                shape: ValueShape::integer(131_072, 8),
                locations: vec![
                    ValueLocation::Stack {
                        stack_byte_offset: 0,
                        value_byte_offset: 0,
                        byte_size: 65_536,
                        alignment: 8,
                    },
                    ValueLocation::Stack {
                        stack_byte_offset: 65_536,
                        value_byte_offset: 65_536,
                        byte_size: 65_536,
                        alignment: 8,
                    },
                ],
            },
        ] {
            let mut encoded = Vec::new();
            legalized_operations::encode_placement(&mut encoded, &placement);
            let mut cursor = Cursor::new(&encoded);
            assert_eq!(decode_placement(&mut cursor).unwrap(), placement);
            assert_eq!(cursor.remaining(), 0);
        }
    }

    #[test]
    fn callback_function_identity_roundtrip_uses_terminal_coordinates() {
        let caller = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([9; 32]),
        };
        let identity = function_identity::MachineFunctionIdentity::callback_thunk(caller, 17)
            .expect("valid callback identity");
        let mut bytes = Vec::new();
        legalized_operations::encode_machine_function_identity(&mut bytes, &identity);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_machine_function_identity(&mut cursor), Ok(identity));
        assert_eq!(cursor.remaining(), 0);
        for length in 0..bytes.len() {
            assert!(decode_machine_function_identity(&mut Cursor::new(&bytes[..length])).is_err());
        }
        bytes[2..34].fill(0);
        assert_eq!(
            decode_machine_function_identity(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidMachineFunctionIdentity),
        );
    }

    fn binding() -> NormalizedForeignCallBinding {
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"leaf".to_vec(),
                version: b"GLIBC_2.0".to_vec(),
            },
            TargetProfile::LinuxX64,
        )
        .expect("applicable locator");
        let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: Some(ValueShape::integer(4, 4)),
            },
        )
        .expect("evaluated entry plan")
        .plan()
        .clone();
        let provider_plan_report_identity = 0xA1;
        let provider_plan_commitment = SameStackProviderPlanCommitment::from_digest([0x42; 32]);
        let same_stack_contribution = admit_same_stack_contribution(
            SameStackContributionAdmissionCandidate {
                provider_plan_report_identity,
                provider_plan_commitment,
                requirement_identity: terminal_psi::TerminalIdentityIndex(REQUIREMENT_INDEX),
                receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(0xA2)
                    .unwrap(),
                bytes: 64,
                alignment: 16,
            },
            provider_plan_report_identity,
            provider_plan_commitment,
            terminal_psi::TerminalIdentityIndex(REQUIREMENT_INDEX),
        )
        .expect("same-stack admission");
        NormalizedForeignCallBinding {
            boundary_entry_plan,
            mechanism: target_operations::NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            },
        }
    }

    fn row() -> SelectedNormalizedForeignCall {
        let binding = binding();
        SelectedNormalizedForeignCall {
            instruction: SelectedInstructionId(3),
            invocation_site: OperationId::new(7).unwrap().into(),
            call: LegalizedNormalizedForeignCall {
                boundary: BoundaryMachineId::new(1).unwrap(),
                provider_execution:
                    target_operations::ProviderExecutionBinding::from_execution_record(
                        ProviderPlanReportIdentity::new(0xA1).unwrap(),
                        0xB1,
                        0xB2,
                        0xB3,
                        0xB4,
                    )
                    .unwrap(),
                scalar_arguments: vec![TargetUnitScalarCallArgument {
                    parameter_index: 0,
                    source: TargetUnitScalarArgumentSource::IntegerImmediate {
                        defining_operation: OperationId::new(6).unwrap(),
                        source_value: ValueId::new(5).unwrap(),
                        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        value: semantic_vocabulary::IntegerValue::Signed(9),
                    },
                    placement: binding.boundary_entry_plan.call.parameters[0].clone(),
                }],
                structural_arguments: Vec::new(),
                structural_indices: Vec::new(),
                callbacks: Vec::new(),
                descriptor_input: Some(target_operations::DescriptorInputGuard {
                    count_scalar_argument: 1,
                    structural_argument: 0,
                    max_bytes: 12,
                    count_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                }),
                result: target_operations::TargetBoundaryResult::Scalar(
                    TargetUnitScalarHomeRequirement {
                        defining_operation: OperationId::new(7).unwrap(),
                        source_value: ValueId::new(8).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        ),
                        shape: ValueShape::integer(4, 4),
                    },
                ),
                binding,
            },
            effect: EffectLink {
                input: 11,
                output: 12,
            },
            ownership: vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())],
        }
    }

    #[test]
    fn system_call_binding_roundtrip_retains_custody_without_an_import_claim() {
        use target_operations::{NormalizedForeignCallMechanism, NormalizedSystemCallBinding};
        for profile in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let signature = calling_conventions::CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                    ValueShape::integer(2, 2),
                    ValueShape::integer(8, 8),
                ],
                result: Some(ValueShape::integer(8, 8)),
            };
            let binding = NormalizedForeignCallBinding {
                boundary_entry_plan: calling_conventions::evaluate_ordinary_boundary_entry_plan(
                    calling_conventions::CallingPolicy::native_syscall_for_target(
                        profile.native_target(),
                    )
                    .unwrap(),
                    &signature,
                )
                .unwrap()
                .plan()
                .clone(),
                mechanism: NormalizedForeignCallMechanism::SystemCall(
                    NormalizedSystemCallBinding {
                        target: profile,
                        number: 211,
                        requirement_identity: "Kernel::invoke".into(),
                        provider_plan_report_identity: 0xA1,
                        provider_plan_commitment: [0x42; 32],
                    },
                ),
            };
            assert!(binding.import().is_none());
            let mut bytes = Vec::new();
            encode_call_binding(&mut bytes, &binding);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_call_binding(&mut cursor).unwrap(), binding);
            assert_eq!(cursor.remaining(), 0);
            for length in 0..bytes.len() {
                assert!(decode_call_binding(&mut Cursor::new(&bytes[..length])).is_err());
            }
            for corruption in 0..7 {
                let mut changed = binding.clone();
                let NormalizedForeignCallMechanism::SystemCall(kernel) = &mut changed.mechanism
                else {
                    panic!("kernel");
                };
                match corruption {
                    0 => kernel.number = -1,
                    1 => kernel.target = TargetProfile::MacosArm64,
                    2 => kernel.provider_plan_report_identity = 0,
                    3 => kernel.requirement_identity.clear(),
                    4 => kernel.target = TargetProfile::AlphaBootstrap,
                    5 => kernel.target = TargetProfile::CrossPlatformCli,
                    _ => {
                        changed.boundary_entry_plan.call.ordinary_clobbers =
                            calling_conventions::RegisterSet::new([])
                    }
                }
                let mut changed_bytes = Vec::new();
                encode_call_binding(&mut changed_bytes, &changed);
                assert_ne!(bytes, changed_bytes);
                assert_eq!(
                    decode_call_binding(&mut Cursor::new(&changed_bytes)),
                    Err(FixedViewCopyDecodeError::InvalidSystemCallBinding)
                );
            }
            for coordinate in 0..2 {
                let mut changed = binding.clone();
                let NormalizedForeignCallMechanism::SystemCall(kernel) = &mut changed.mechanism
                else {
                    panic!("kernel");
                };
                if coordinate == 0 {
                    kernel.number += 1;
                } else {
                    kernel.provider_plan_commitment[31] ^= 1;
                }
                let mut changed_bytes = Vec::new();
                encode_call_binding(&mut changed_bytes, &changed);
                assert_ne!(bytes, changed_bytes);
                assert_eq!(
                    decode_call_binding(&mut Cursor::new(&changed_bytes)).unwrap(),
                    changed
                );
            }
        }
    }

    #[test]
    fn normalized_foreign_call_roundtrip_preserves_complete_evaluated_custody() {
        let mut source = row();
        let borrowed_shape = ValueShape::borrowed_reference(4, 4);
        source.call.binding.boundary_entry_plan =
            calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
                &CallSignature {
                    parameters: vec![ValueShape::integer(4, 4), borrowed_shape],
                    result: Some(ValueShape::integer(4, 4)),
                },
            )
            .unwrap()
            .plan()
            .clone();
        source.call.scalar_arguments[0].placement =
            source.call.binding.boundary_entry_plan.call.parameters[0].clone();
        let indices = [(21, 64, 20, 4, 31), (22, 32, 4, 5, 32)]
            .into_iter()
            .map(|(value, width, stride, extent, obligation)| {
                legalized_operations::LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult {
                        value: ValueId::new(value).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, width).unwrap(),
                        ),
                    },
                    stride,
                    extent: Some(extent),
                    obligation: semantic_vocabulary::ObligationId::new(obligation).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [u8::try_from(obligation).unwrap(); 32],
                    ),
                }
            })
            .collect::<Vec<_>>();
        source.call.structural_arguments = vec![target_operations::TargetStructuralArgument {
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
            access: terminal_psi::StructuralAccess::SharedBorrow,
            path: indices
                .iter()
                .map(|index| terminal_psi::StructuralPathSegment::RuntimeIndex {
                    index: index.operand.value,
                    obligation: index.obligation,
                })
                .collect(),
            root_structural_type: semantic_vocabulary::StructuralTypeId::new(12).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(13).unwrap(),
            shape: borrowed_shape,
            source_byte_offset: 0,
            fixed_array_length: Some(4),
            element_stride: Some(20),
            indices: indices
                .iter()
                .enumerate()
                .map(
                    |(position, index)| target_operations::TargetStructuralRuntimeIndex {
                        operand: TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: u32::try_from(position).unwrap(),
                            source_value: index.operand.value,
                            scalar_type: index.operand.scalar_type,
                        },
                        stride: index.stride,
                    },
                )
                .collect(),
            source: target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(20).unwrap(),
            },
            destination: source.call.binding.boundary_entry_plan.call.parameters[1].clone(),
        }];
        source.call.structural_indices = vec![indices];
        let mut bytes = Vec::new();
        encode_normalized_foreign_call(&mut bytes, &source);
        let mut cursor = Cursor::new(&bytes);
        let decoded = decode_normalized_foreign_call(&mut cursor).unwrap();
        assert_eq!(cursor.remaining(), 0);
        assert_eq!(decoded, source);
        for length in 0..bytes.len() {
            assert!(
                decode_normalized_foreign_call(&mut Cursor::new(&bytes[..length])).is_err(),
                "truncated prefix {length} decoded"
            );
        }
        let mut changed = source.clone();
        changed.call.structural_indices[0].swap(0, 1);
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        assert_eq!(
            decode_normalized_foreign_call(&mut Cursor::new(&changed_bytes)).unwrap(),
            changed,
        );
        let mut changed = source.clone();
        changed.call.boundary = BoundaryMachineId::new(2).unwrap();
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        let mut changed = source.clone();
        changed.call.binding.boundary_entry_plan.call.parameters[0].locations =
            vec![ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 4,
                alignment: 4,
            }];
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        let mut changed = source.clone();
        changed.call.scalar_arguments[0].parameter_index = 9;
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        let mut changed = source.clone();
        changed.call.result = target_operations::TargetBoundaryResult::Unit;
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
    }

    #[test]
    fn normalized_foreign_structural_result_roundtrip_preserves_origin_and_layout() {
        let mut source = row();
        let result = terminal_psi::StructuralOperationResult {
            place: semantic_vocabulary::PlaceId::new(9).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(10).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        source.call.result = target_operations::TargetBoundaryResult::Structural(
            target_operations::TargetStructuralHomeRequirement {
                origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation: source
                        .invocation_site
                        .operation()
                        .expect("returning test call"),
                    result,
                },
                layout: target_operations::TargetStructuralHomeLayout::Aggregate(
                    ValueShape::integer(12, 4),
                ),
            },
        );
        let mut bytes = Vec::new();
        encode_normalized_foreign_call(&mut bytes, &source);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_normalized_foreign_call(&mut cursor).unwrap(), source);
        assert_eq!(cursor.remaining(), 0);
        for length in 0..bytes.len() {
            assert!(decode_normalized_foreign_call(&mut Cursor::new(&bytes[..length])).is_err());
        }
        let mut changed = source.clone();
        let target_operations::TargetBoundaryResult::Structural(home) = &mut changed.call.result
        else {
            panic!("structural fixture")
        };
        let target_operations::TargetStructuralHomeOrigin::OperationResult { result, .. } =
            &mut home.origin
        else {
            panic!("operation fixture")
        };
        result.place = semantic_vocabulary::PlaceId::new(11).unwrap();
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
    }

    #[test]
    fn decode_rejects_unknown_tags_and_substituted_sealed_coordinates() {
        let mut bytes = Vec::new();
        encode_string(&mut bytes, "not a canonical target");
        assert_eq!(
            decode_locator(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidForeignTargetProfile)
        );
        let mut bytes = Vec::new();
        encode_string(&mut bytes, TargetProfile::LinuxX64.target_name());
        bytes.push(9);
        assert_eq!(
            decode_locator(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownForeignLocatorCase(9))
        );
        let mut bytes = Vec::new();
        encode_string(&mut bytes, TargetProfile::LinuxX64.target_name());
        bytes.push(1);
        length(&mut bytes, b"kernel32.dll".len());
        bytes.extend_from_slice(b"kernel32.dll");
        length(&mut bytes, b"Leaf".len());
        bytes.extend_from_slice(b"Leaf");
        assert_eq!(
            decode_locator(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidForeignLocator)
        );
        for (tag, expected) in [
            (0u8, FixedViewCopyDecodeError::UnknownMachineRegime(0)),
            (9, FixedViewCopyDecodeError::UnknownMachineRegime(9)),
        ] {
            assert_eq!(decode_state_plan(&mut Cursor::new(&[tag])), Err(expected));
        }
        let mut bytes = Vec::new();
        bytes.push(1);
        for _ in 0..4 {
            crate::codec_bytes::push_encoded(&mut bytes, 0u16.to_le_bytes());
        }
        bytes.push(9);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownEntryStack(9))
        );
        let mut bytes = Vec::new();
        bytes.push(1);
        for _ in 0..4 {
            crate::codec_bytes::push_encoded(&mut bytes, 0u16.to_le_bytes());
        }
        bytes.push(3);
        bytes.push(9);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownPreemption(9))
        );
        let mut bytes = Vec::new();
        bytes.push(1);
        crate::codec_bytes::push_encoded(&mut bytes, 0x0200u16.to_le_bytes());
        for _ in 0..3 {
            crate::codec_bytes::push_encoded(&mut bytes, 0u16.to_le_bytes());
        }
        bytes.push(3);
        bytes.push(1);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidMachineStateSet(0x0200))
        );
        assert_eq!(
            decode_scalar_source(&mut Cursor::new(&[9])),
            Err(FixedViewCopyDecodeError::UnknownForeignScalarSource(9))
        );
        let binding = binding();
        let mut prefix = Vec::new();
        let (locator, contribution) = binding.import().unwrap();
        encode_locator(&mut prefix, locator);
        encode_call_plan(&mut prefix, &binding.boundary_entry_plan.call);
        encode_state_plan_identity(&mut prefix, &binding.boundary_entry_plan.state);
        let contribution_offset = prefix.len();
        let mut bytes = Vec::new();
        encode_call_binding(&mut bytes, &binding);
        assert_eq!(
            &bytes[contribution_offset..contribution_offset + 8],
            &contribution
                .report_identity()
                .normalized_identity()
                .to_le_bytes()
        );
        bytes[contribution_offset] ^= 0xFF;
        assert_eq!(
            decode_call_binding(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidSameStackContribution)
        );
        let mut bytes = Vec::new();
        encode_call_binding(&mut bytes, &binding);
        let provider_identity_offset = contribution_offset + 8 + 32;
        assert_eq!(
            &bytes[provider_identity_offset..provider_identity_offset + 8],
            &contribution.provider_plan_report_identity().to_le_bytes()
        );
        for byte in &mut bytes[provider_identity_offset..provider_identity_offset + 8] {
            *byte = 0;
        }
        assert_eq!(
            decode_call_binding(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidSameStackContribution)
        );
    }

    #[test]
    fn result_codec_retains_qualifications_claims_and_submitted_layout() {
        let result = StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: PlaceId::new(1).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            qualifications: vec![StructuralDomainId::new(3).unwrap()],
            projected_qualifications: vec![terminal_psi::StructuralPathQualification {
                path: Vec::new(),
                domain: StructuralDomainId::new(4).unwrap(),
            }],
            additional_input_qualifications: vec![terminal_psi::StructuralPathQualification {
                path: Vec::new(),
                domain: StructuralDomainId::new(3).unwrap(),
            }],
            claims: vec![StructuralResultClaimBinding {
                claim: ClaimId::new(5).unwrap(),
                path: Vec::new(),
            }],
        };
        let mut layout = calling_conventions::evaluate_conventional_sum_layout(
            &[],
            &[vec![], vec![calling_conventions::ValueShape::integer(4, 4)]],
        )
        .unwrap();
        layout.payload_byte_offset = 8;
        let mut bytes = Vec::new();
        encode_read_result_and_layout(&mut bytes, &result, &layout);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_read_result_and_layout(&mut cursor).unwrap(),
            (result, layout)
        );
        assert_eq!(cursor.remaining(), 0);
        for end in 0..bytes.len() {
            assert!(decode_read_result_and_layout(&mut Cursor::new(&bytes[..end])).is_err());
        }
    }
}
