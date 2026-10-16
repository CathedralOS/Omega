//! Canonical identity producer for the selected-instruction plan.
//!
//! Owns the byte-stable encoding over which `SelectedInstructionPlanIdentity`
//! is derived; the pipeline only wraps it in the selection receipt.
//!
//! The identity is two-level: `Sha256` over the plan's roots and the ordered
//! digests of its functions, each digest `Sha256` over one function's full
//! encoding. Every field of every function still reaches the identity. The
//! split exists for cost: a rewrite validator mints the identity of each plan
//! it admits, and register-pressure recovery admits one plan per attempted
//! step, each differing from its source in one function. The function
//! storage memoizes each digest beside its content, so minting the identity
//! of such a plan hashes the edited function and the digest roster only.

use optimization_unit::{FuelSettlement, PsiProvenance};
use register_model::RegisterOperandAccess;
use semantic_vocabulary::{IntegerSign, ScalarType};

use crate::{
    SaturatingCarrier, SaturatingOperation, SelectedFunction, SelectedInstruction,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedSuccessor,
};

mod contracts;
#[cfg(test)]
mod function_digest_tests;
mod ordinary;
mod primitives;
mod register;

use crate::effects::program::identity::encode_ids;
use primitives::encode_constraint_key;

/// The identity of a complete selected plan: its roots and the ordered
/// digests of its functions (`selected_function_digest`). Each function's
/// digest is memoized on its shared storage entry, so a rewrite that clones
/// the plan and edits one function hashes that function alone; every other
/// digest was computed for the identical content of the source plan.
pub fn selected_instruction_plan_identity(
    plan: &SelectedInstructionPlan,
) -> SelectedInstructionPlanIdentity {
    let domain = b"omega.terminal-selected-instructions.v52\0".as_slice();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.psi.vocabulary_marker.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&plan.target.identity_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.entry.get().to_le_bytes());
    crate::codec_bytes::encode_len(&mut bytes, plan.functions.len());
    for digest in plan.functions.identity_digests() {
        bytes.extend_from_slice(&digest);
    }
    SelectedInstructionPlanIdentity::from_canonical_bytes(&bytes)
}

/// One function's share of `selected_instruction_plan_identity`: `Sha256`
/// over the function's complete canonical encoding.
pub(crate) fn selected_function_digest(function: &SelectedFunction) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-selected-instruction-function.v1\0");
    crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
    optimization_core::encode_option_u64(
        &mut bytes,
        function.attachment.map(|attachment| attachment.get()),
    );
    optimization_core::encode_option_u64(
        &mut bytes,
        function
            .nominal_cleanup_subject
            .map(|attachment| attachment.get()),
    );
    encode_ids(
        &mut bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        &mut bytes,
        function.provenance.edges.iter().map(|edge| edge.get()),
    );
    contracts::encode(&mut bytes, function);
    crate::codec_bytes::push_encoded(&mut bytes, function.entry_block.0.to_le_bytes());
    crate::codec_bytes::encode_len(&mut bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        register::encode(&mut bytes, register);
    }
    crate::codec_bytes::encode_len(&mut bytes, function.blocks.len());
    for block in &function.blocks {
        crate::codec_bytes::push_encoded(&mut bytes, block.id.0.to_le_bytes());
        match block.origin {
            crate::SelectedBlockOrigin::Source(source) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(&mut bytes, source.get().to_le_bytes());
            }
            crate::SelectedBlockOrigin::CaseDispatch {
                source,
                case_ordinal,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(&mut bytes, source.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, case_ordinal.to_le_bytes());
            }
            crate::SelectedBlockOrigin::EdgeTransfer { edge, target } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(&mut bytes, edge.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, target.get().to_le_bytes());
            }
        }
        crate::codec_bytes::encode_len(&mut bytes, block.instructions.len());
        for instruction in &block.instructions {
            encode_instruction(&mut bytes, instruction);
        }
        ordinary::encode_terminator(&mut bytes, &block.terminator);
    }
    crate::digest_encoder::content_identity(&bytes).into()
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    crate::codec_bytes::push_encoded(bytes, instruction.id.0.to_le_bytes());
    bytes.push(match instruction.kind {
        SelectedInstructionKind::CopyBytes => 59,
        SelectedInstructionKind::ScanByteSequence { .. } => 233,
        SelectedInstructionKind::LoadEncodedScalar => 236,
        SelectedInstructionKind::StoreEncodedScalar => 237,
        SelectedInstructionKind::CompareByteSequences => 234,
        SelectedInstructionKind::ZeroBytes => 235,
        SelectedInstructionKind::BitwiseAndI64 => 48,
        SelectedInstructionKind::BitwiseXorI64 => 49,
        SelectedInstructionKind::BitwiseOrI64 => 93,
        SelectedInstructionKind::BitwiseNotI64 => 94,
        SelectedInstructionKind::ExactDivideU64 { .. } => 54,
        SelectedInstructionKind::ExactRemainderU64 { .. } => 89,
        SelectedInstructionKind::WrappingRemainderI64 { .. } => 55,
        SelectedInstructionKind::WrappingDivideI64 { .. } => 92,
        SelectedInstructionKind::WrappingSubtractI64 => 90,
        SelectedInstructionKind::WrappingMultiplyI64 => 91,
        SelectedInstructionKind::WrappingAddI64 => 56,
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            saturating_tag(SaturatingOperation::Add, carrier)
        }
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            saturating_tag(SaturatingOperation::Subtract, carrier)
        }
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
            saturating_tag(SaturatingOperation::Divide, carrier)
        }
        SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
            saturating_tag(SaturatingOperation::Remainder, carrier)
        }
        SelectedInstructionKind::SaturatingMultiply { carrier } => {
            saturating_tag(SaturatingOperation::Multiply, carrier)
        }
        // Trapping forms take 122 plus their dense ordinal, above every
        // other tag of this plan identity.
        SelectedInstructionKind::TrappingInteger { form } => crate::trapping_family_tag(form),
        SelectedInstructionKind::PortWrite { .. } => 239,
        SelectedInstructionKind::RegisterAccess { .. } => 247,
        SelectedInstructionKind::AtomicFence => 238,
        SelectedInstructionKind::AtomicAccess { form } => form.machine_tag(),
        SelectedInstructionKind::PlacedScalarAccess { form, .. } => form.machine_tag(),
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => form.machine_tag(),
        SelectedInstructionKind::FloatResultPolicy { form } => form.machine_tag(),
        SelectedInstructionKind::LoadPacked { .. } => 46,
        SelectedInstructionKind::StorePacked { .. } => 47,
        SelectedInstructionKind::CallAggregate { .. } => 35,
        SelectedInstructionKind::ReturnAggregate { .. } => 36,
        SelectedInstructionKind::Crash => 111,
        SelectedInstructionKind::HostedExitProcessI32 => 31,
        SelectedInstructionKind::HostedReadByte { .. } => 32,
        SelectedInstructionKind::HostedWriteByteI32 { .. } => 23,
        SelectedInstructionKind::SaveFloatingControl { .. } => 103,
        SelectedInstructionKind::RestoreFloatingControl { .. } => 104,
        SelectedInstructionKind::Store { .. } => 24,
        SelectedInstructionKind::AddressOffset { .. } => 25,
        SelectedInstructionKind::Load64 { .. } => 16,
        SelectedInstructionKind::Load8 { .. } => 33,
        SelectedInstructionKind::Load16 { .. } => 34,
        SelectedInstructionKind::Load32 { .. } => 30,
        SelectedInstructionKind::Load8Indexed => 21,
        SelectedInstructionKind::ByteViewAddress => 22,
        SelectedInstructionKind::Store64 { .. } => 17,
        SelectedInstructionKind::FrameAddress { .. } => 18,
        SelectedInstructionKind::CallUnit { .. } => 19,

        SelectedInstructionKind::Jump => 14,
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::ConditionalBranchNonZero => 2,
        SelectedInstructionKind::ReturnScalar => 3,
        SelectedInstructionKind::CopyI64 => 4,
        SelectedInstructionKind::Float32ToBits => 26,
        SelectedInstructionKind::Float64ToBits => 27,
        SelectedInstructionKind::BitsToFloat32 => 28,
        SelectedInstructionKind::BitsToFloat64 => 29,
        SelectedInstructionKind::ZeroExtendU8 => 15,
        SelectedInstructionKind::ZeroExtendU32 => 20,
        SelectedInstructionKind::ZeroExtendU16 => 37,
        SelectedInstructionKind::SignExtendI8 => 38,
        SelectedInstructionKind::SignExtendI16 => 39,
        SelectedInstructionKind::SignExtendI32 => 40,
        SelectedInstructionKind::MaterializeBooleanEqual => 41,
        SelectedInstructionKind::MaterializeBooleanU64LessThan => 42,
        SelectedInstructionKind::MaterializeBooleanI64LessThan => 43,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 44,
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 45,
        SelectedInstructionKind::ExactAddI64 { .. } => 5,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 6,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 7,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
        SelectedInstructionKind::CompareI64 => 10,
        SelectedInstructionKind::CompareI64Immediate { .. } => 53,
        SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
        SelectedInstructionKind::CallScalar { .. } => 12,
        SelectedInstructionKind::NormalizedForeignCall { .. } => 57,
        SelectedInstructionKind::ExactMultiplyI64 { .. } => 88,
        SelectedInstructionKind::WrappingShiftLeftI64 => 105,
        SelectedInstructionKind::WrappingShiftRightI64 => 106,
        SelectedInstructionKind::WrappingShiftRightU64 => 107,
        SelectedInstructionKind::ExactShiftLeftI64 { .. } => 108,
        SelectedInstructionKind::ExactShiftRightI64 { .. } => 109,
        SelectedInstructionKind::ExactShiftRightU64 { .. } => 110,
        SelectedInstructionKind::ExactDivideI64 { .. } => 112,
        SelectedInstructionKind::ExactRemainderI64 { .. } => 113,
        SelectedInstructionKind::ConditionalBranchI64LessThan => 13,
        SelectedInstructionKind::ImmutableImageAddress { .. } => 232,
    });
    match instruction.kind {
        SelectedInstructionKind::RegisterAccess { operation } => {
            crate::codec_bytes::push_encoded(bytes, operation.catalog_tag().to_le_bytes());
        }
        SelectedInstructionKind::PortWrite { port, value } => {
            crate::codec_bytes::push_encoded(bytes, port.to_le_bytes());
            bytes.push(value);
        }

        SelectedInstructionKind::CopyBytes | SelectedInstructionKind::ZeroBytes => {}
        SelectedInstructionKind::ScanByteSequence { predicate } => {
            bytes.push(predicate.mask_bit());
        }
        SelectedInstructionKind::CompareByteSequences
        | SelectedInstructionKind::LoadEncodedScalar
        | SelectedInstructionKind::StoreEncodedScalar => {}
        SelectedInstructionKind::LoadPacked { byte_offset, width }
        | SelectedInstructionKind::StorePacked { byte_offset, width } => {
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            bytes.push(width.byte_size());
        }
        SelectedInstructionKind::ReturnAggregate { fragment_count } => bytes.push(fragment_count),
        SelectedInstructionKind::CallAggregate { target } => target.encode_identity(bytes),
        crate::hosted_slot_quad_pattern!(slot, ..) => {
            contracts::frame_slot(bytes, crate::FrameStorageSlotId::Local(slot));
        }
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            bytes.push(byte_size);
        }
        SelectedInstructionKind::AddressOffset { byte_offset } => {
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::Load64 { byte_offset }
        | crate::load_width_triple_pattern!(byte_offset, ..) => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes())
        }
        SelectedInstructionKind::Store64 { slot, byte_offset }
        | SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
            contracts::frame_slot(bytes, slot);
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::MaterializeI64 { value } => match value {
            semantic_vocabulary::IntegerValue::Signed(value) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
            }
            semantic_vocabulary::IntegerValue::Unsigned(value) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
            }
        },
        SelectedInstructionKind::ExactDivideU64 {
            obligation,
            accepted_fact,
        }
        | crate::exact_wrapping_divrem_set_pattern!(obligation, accepted_fact, ..)
        | SelectedInstructionKind::SaturatingDivide {
            obligation,
            accepted_fact,
            ..
        }
        | SelectedInstructionKind::SaturatingRemainder {
            obligation,
            accepted_fact,
            ..
        }
        | SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => {
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactMultiplyI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactShiftLeftI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactShiftRightI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactShiftRightU64 {
            obligation,
            accepted_fact,
        } => {
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        crate::exact_add_or_sub_pair_pattern!(immediate, obligation, accepted_fact, ..) => {
            match immediate {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
                }
            }
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => match immediate {
            semantic_vocabulary::IntegerValue::Signed(value) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
            }
            semantic_vocabulary::IntegerValue::Unsigned(value) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
            }
        },
        // Every float form shares one tag; its dense ordinal follows.
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => {
            bytes.push(form.machine_payload())
        }
        SelectedInstructionKind::PlacedScalarAccess {
            form,
            contract,
            byte_offset,
        } => {
            bytes.push(form.machine_payload());
            bytes.extend_from_slice(&contract);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::FloatResultPolicy { form } => bytes.push(form.machine_payload()),
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::BitwiseAndI64
        | SelectedInstructionKind::BitwiseOrI64
        | SelectedInstructionKind::BitwiseXorI64
        | SelectedInstructionKind::BitwiseNotI64
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::WrappingSubtractI64
        | SelectedInstructionKind::WrappingMultiplyI64
        | SelectedInstructionKind::WrappingShiftLeftI64
        | SelectedInstructionKind::WrappingShiftRightI64
        | SelectedInstructionKind::WrappingShiftRightU64
        | crate::saturating_add_or_subtract_pattern!()
        | SelectedInstructionKind::SaturatingMultiply { .. }
        | SelectedInstructionKind::TrappingInteger { .. }
        | SelectedInstructionKind::AtomicFence
        | SelectedInstructionKind::AtomicAccess { .. }
        | SelectedInstructionKind::CompareI64
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU32
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::Crash
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::ReturnUnit
        | SelectedInstructionKind::Jump => {}
        SelectedInstructionKind::CallScalar { target }
        | SelectedInstructionKind::CallUnit { target } => {
            target.encode_identity(bytes);
        }
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => {
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, ordinal.to_le_bytes());
        }
        SelectedInstructionKind::ImmutableImageAddress { reference } => {
            reference.encode_identity(bytes);
        }
    }
    encode_constraint_key(bytes, instruction.constraint);
    crate::codec_bytes::encode_len(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        crate::codec_bytes::push_encoded(bytes, operand.operand.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, operand.virtual_register.0.to_le_bytes());
        bytes.push(match operand.access {
            RegisterOperandAccess::Use => 0,
            RegisterOperandAccess::Def => 1,
            RegisterOperandAccess::UseDef => 2,
        });
        crate::codec_bytes::push_encoded(bytes, operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_ids(
        bytes,
        instruction
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .values
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        instruction.provenance.edges.iter().map(|edge| edge.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    encode_fuel(bytes, &instruction.provenance.fuel);
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.push(match successor.role {
        crate::SelectedSuccessorRole::Semantic => 0,
        crate::SelectedSuccessorRole::EdgeTransferContinuation => 1,
        crate::SelectedSuccessorRole::CaseDispatchContinuation => 2,
    });
    crate::codec_bytes::push_encoded(bytes, successor.psi_edge.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, successor.block.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, successor.source_target.get().to_le_bytes());
    crate::codec_bytes::encode_len(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        crate::codec_bytes::push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, binding.semantic.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.semantic.scalar_type);
        match binding.transport {
            crate::SelectedValueTransport::Unused => bytes.push(0),
            crate::SelectedValueTransport::Registers {
                argument,
                parameter,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, argument.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, parameter.0.to_le_bytes());
            }
        }
    }
    match &successor.structural_case {
        None => bytes.push(0),
        Some(case) => {
            bytes.push(1);
            case.encode_identity(bytes);
        }
    }
    crate::codec_bytes::encode_len(bytes, successor.structural_bindings.len());
    for binding in &successor.structural_bindings {
        crate::codec_bytes::encode_len(bytes, binding.source_indices.len());
        for index in &binding.source_indices {
            crate::codec_bytes::push_encoded(bytes, index.operand.value.get().to_le_bytes());
            encode_scalar_type(bytes, index.operand.scalar_type);
            crate::codec_bytes::push_encoded(bytes, index.stride.to_le_bytes());
            optimization_core::encode_option_u64(bytes, index.extent);
            crate::codec_bytes::push_encoded(bytes, index.obligation.get().to_le_bytes());
            bytes.extend_from_slice(&index.accepted_fact.bytes());
        }
        crate::codec_bytes::push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        crate::codec_bytes::push_encoded(
            bytes,
            binding.semantic.argument.place.get().to_le_bytes(),
        );
        crate::codec_bytes::encode_len(bytes, binding.semantic.argument.path.len());
        for segment in &binding.semantic.argument.path {
            match segment {
                terminal_psi::StructuralPathSegment::Case(case) => {
                    bytes.push(7);
                    crate::codec_bytes::push_encoded(bytes, case.get().to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::Referent => bytes.push(2),
                terminal_psi::StructuralPathSegment::FixedByteRange { start, end } => {
                    bytes.push(3);
                    crate::codec_bytes::push_encoded(bytes, start.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, end.to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::Field(field) => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, field.0.to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, index.to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation } => {
                    bytes.push(4);
                    crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::RepresentationView {
                    structural_type,
                    stored_scalar,
                } => {
                    bytes.push(8);
                    crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
                    bytes.push(u8::from(stored_scalar.is_some()));
                    if let Some(scalar) = stored_scalar {
                        encode_scalar_type(bytes, *scalar);
                    }
                }
            }
        }
        bytes.push(match binding.semantic.argument.access {
            terminal_psi::StructuralAccess::Owned => 0,
            terminal_psi::StructuralAccess::SharedBorrow => 1,
            terminal_psi::StructuralAccess::MutableBorrow => 2,
            terminal_psi::StructuralAccess::WriteOnlyBorrow => 3,
        });
        match binding.transport {
            crate::SelectedStructuralTransport::Unused => bytes.push(0),
            crate::SelectedStructuralTransport::WholeValue {
                argument,
                source_byte_offset,
                destination,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, argument.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, source_byte_offset.to_le_bytes());
                destination.encode_identity(bytes);
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
            }
            crate::SelectedStructuralTransport::ScalarReference {
                base,
                byte_offset,
                source,
                destination,
            } => {
                bytes.push(4);
                match base {
                    crate::SelectedAddressBase::Register(register) => {
                        bytes.push(0);
                        crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
                    }
                    crate::SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                match source {
                    crate::SelectedScalarReferenceSource::Canonical { codec } => {
                        bytes.push(0);
                        crate::codec_bytes::push_encoded(bytes, codec.to_le_bytes());
                    }
                    crate::SelectedScalarReferenceSource::Descriptor => bytes.push(1),
                }
                destination.encode_identity(bytes);
            }
            crate::SelectedStructuralTransport::Address {
                base,
                byte_offset,
                byte_count,
                destination,
            } => {
                bytes.push(3);
                match base {
                    crate::SelectedAddressBase::Register(register) => {
                        bytes.push(0);
                        crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
                    }
                    crate::SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_count.to_le_bytes());
                destination.encode_identity(bytes);
            }
        }
    }
    encode_fuel(bytes, &successor.fuel);
}

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    crate::codec_bytes::push_encoded(bytes, (fuel.len() as u64).to_le_bytes());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
        crate::codec_bytes::push_encoded(bytes, settlement.units.to_le_bytes());
    }
}

pub(crate) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => 0,
                semantic_vocabulary::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            crate::codec_bytes::push_encoded(bytes, integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        Some(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    crate::codec_bytes::encode_len(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    }
}

/// This plan identity's saturating tags: the u64 subtract and add keep 51
/// and 52, the i32 forms keep 60, 61, and 62, and every other carrier takes
/// its ordinal above a per-operation base (add 63, subtract 71, divide 79,
/// remainder 95, multiply 114).
const fn saturating_tag(operation: SaturatingOperation, carrier: SaturatingCarrier) -> u8 {
    match (operation, carrier) {
        (SaturatingOperation::Subtract, SaturatingCarrier::U64) => 51,
        (SaturatingOperation::Add, SaturatingCarrier::U64) => 52,
        (SaturatingOperation::Add, SaturatingCarrier::I32) => 60,
        (SaturatingOperation::Subtract, SaturatingCarrier::I32) => 61,
        (SaturatingOperation::Divide, SaturatingCarrier::I32) => 62,
        (SaturatingOperation::Add, carrier) => 63 + carrier.ordinal(),
        (SaturatingOperation::Subtract, carrier) => 71 + carrier.ordinal(),
        (SaturatingOperation::Divide, carrier) => 79 + carrier.ordinal(),
        (SaturatingOperation::Remainder, carrier) => 95 + carrier.ordinal(),
        (SaturatingOperation::Multiply, carrier) => 114 + carrier.ordinal(),
    }
}

#[cfg(test)]
mod tests {
    use super::encode_instruction;
    use crate::{
        SaturatingCarrier, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance,
    };
    use register_model::RegisterConstraintKey;

    #[test]
    fn float_result_policy_plan_identity_binds_format_rule_operands_and_operation() {
        use crate::{FloatResultPolicyForm, SelectedOperand, VirtualRegisterId};
        use register_model::{RegisterClassId, RegisterOperandAccess};
        use semantic_vocabulary::{OperationId, ValueId};
        let encode = |instruction: &SelectedInstruction| {
            let mut bytes = Vec::new();
            encode_instruction(&mut bytes, instruction);
            bytes
        };
        let mut form_identities = Vec::new();
        for form in FloatResultPolicyForm::ALL {
            let input_count = form.operand_count();
            let instruction = SelectedInstruction {
                id: SelectedInstructionId(7),
                kind: SelectedInstructionKind::FloatResultPolicy { form },
                constraint: RegisterConstraintKey {
                    family: register_model::RegisterConstraintFamily::Instruction,
                    variant: 7,
                },
                operands: (0..=input_count)
                    .map(|position| SelectedOperand {
                        operand: position as u16,
                        virtual_register: VirtualRegisterId(position as u32 + 1),
                        access: if position < input_count {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        },
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    })
                    .collect(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(2).unwrap()],
                    values: (0..=input_count)
                        .map(|position| ValueId::new(position as u64 + 1).unwrap())
                        .collect(),
                    ..Default::default()
                },
            };
            let expected = encode(&instruction);
            form_identities.push(expected.clone());
            for mutation in 0..3 {
                let mut changed = instruction.clone();
                match mutation {
                    0 => changed.operands.swap(0, input_count),
                    1 => changed.provenance.values.swap(0, input_count),
                    _ => changed.provenance.operations[0] = OperationId::new(3).unwrap(),
                }
                assert_ne!(encode(&changed), expected, "{form:?}, mutation {mutation}");
            }
        }
        form_identities.sort();
        form_identities.dedup();
        assert_eq!(form_identities.len(), FloatResultPolicyForm::COUNT);
    }

    #[test]
    fn arithmetic_and_compare_instruction_identity_discriminants_are_distinct() {
        let kinds = [
            SelectedInstructionKind::WrappingRemainderI64 {
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
            },
            SelectedInstructionKind::SaturatingSubtract {
                carrier: SaturatingCarrier::U64,
            },
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::U64,
            },
            SelectedInstructionKind::CompareI64Immediate {
                immediate: semantic_vocabulary::IntegerValue::Unsigned(4095),
            },
            SelectedInstructionKind::ExactDivideU64 {
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
            },
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::I32,
            },
            SelectedInstructionKind::SaturatingSubtract {
                carrier: SaturatingCarrier::I32,
            },
            SelectedInstructionKind::SaturatingDivide {
                carrier: SaturatingCarrier::I32,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
            },
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::I8,
            },
            SelectedInstructionKind::SaturatingDivide {
                carrier: SaturatingCarrier::U64,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
            },
        ];
        let discriminants = kinds.map(|kind| {
            let instruction = SelectedInstruction {
                id: SelectedInstructionId(7),
                kind,
                constraint: RegisterConstraintKey {
                    family: register_model::RegisterConstraintFamily::Instruction,
                    variant: 7,
                },
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance::default(),
            };
            let mut bytes = Vec::new();
            encode_instruction(&mut bytes, &instruction);
            let identity = instruction.id.0.to_le_bytes();
            assert_eq!(&bytes[..identity.len()], &identity);
            bytes[identity.len()]
        });
        assert_eq!(discriminants, [55, 51, 52, 53, 54, 60, 61, 62, 63, 86]);
    }
}
