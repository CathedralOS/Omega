//! Encoding and validating selected x86_64 forms.
//!
//! This file carries the footprint, the validated encoding, the error and
//! the two entry points. `branch_forms.rs` encodes and validates branch
//! forms, `request_validation.rs` validates requests and resolves
//! registers, `instruction_bytes.rs` assembles instruction bytes,
//! `decoding.rs` decodes and validates the emitted bytes, and
//! `saturating_forms.rs` and `trapping_forms.rs` name the realization shape
//! of each saturating operation and carrier and of each Trapping form that
//! those three and the machine-effect catalog share.

pub(crate) mod atomic;
pub(crate) mod atomic_fence;
#[cfg(test)]
mod boolean_materialization_tests;
mod branch_forms;
#[cfg(test)]
mod byte_literal_address_tests;
#[cfg(test)]
mod byte_view_address_tests;
mod compare_byte_sequences;
mod copy_bytes;
pub(crate) mod crash;
pub(crate) mod decoding;
mod encoded_scalar;
mod float_bits;
pub(crate) mod float_result_policy;
pub(crate) mod floating_control;
pub(crate) mod ieee_float_arithmetic;
pub(crate) mod port_write;
mod scan_byte_sequence;
mod zero_bytes;

pub(crate) mod hosted_exit_process;
pub(crate) mod hosted_read_byte;
pub(crate) mod hosted_write_byte;
mod instruction_bytes;
#[cfg(test)]
mod integer_normalization_tests;
mod jump;
pub(crate) mod materialization;
mod memory;
mod normalized_foreign_call;
mod request_validation;
pub(crate) mod saturating_forms;
mod scalar_call;
pub(crate) mod system_call;
#[cfg(test)]
mod tests;
pub(crate) mod trapping_forms;

pub use branch_forms::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
pub use hosted_write_byte::{
    encode_x86_64_selected_hosted_write_byte_form, validate_x86_64_selected_hosted_write_byte_form,
};
pub use jump::{encode_x86_64_selected_jump_form, validate_x86_64_selected_jump_form};
pub use memory::frame::{
    encode as encode_x86_64_selected_frame_form, validate as validate_x86_64_selected_frame_form,
};
pub use memory::{encode_x86_64_selected_memory_form, validate_x86_64_selected_memory_form};
pub use normalized_foreign_call::{
    X86_64CallbackAddressMaterialization, X86_64CallbackMaterializationDestination,
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState, X86_64NormalizedForeignCallTemplateError,
    encode_x86_64_selected_normalized_foreign_call_template,
    validate_x86_64_selected_normalized_foreign_call_template,
};
pub use scalar_call::{
    X86_64ScalarCallFixup, X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    X86_64ScalarCallTemplateError, encode_x86_64_selected_dynamic_call_template,
    encode_x86_64_selected_scalar_call_template, validate_x86_64_selected_scalar_call_template,
};

use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, SelectedInstructionKind, is_float_bit_transfer,
};

use crate::selected_form_encoding::decoding::{decode_all, footprint, validate_decoded};
pub use crate::selected_form_encoding::instruction_bytes::X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET;
use crate::selected_form_encoding::instruction_bytes::encode_unchecked;
use crate::selected_form_encoding::request_validation::{
    resolve_scalar_registers, validate_return_home,
};
use crate::selected_form_encoding::request_validation::{
    validate_alias_partition, validate_request,
};

/// Shared single-alternative hosted declaration shell: the semantic/memory/
/// trap/family identity plus caller-supplied size knowledge and encoding.
pub(super) fn hosted_alternative_declaration(
    constraint: RegisterConstraintKey,
    semantic: MachineSemanticKind,
    memory: MachineMemoryEffect,
    trap: MachineTrapBehavior,
    family: MachineAlternativeFamily,
    size: MachineSizeKnowledge,
    encoded: MachineEncodedEffects,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic,
        constraint,
        memory,
        trap,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey { family, variant: 0 },
            applicability: MachineAlternativeApplicability::Always,
            size,
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }],
    }
}

/// The canonical rflags-clobbering footprint for memory-form encoders whose
/// operands split contiguously: the first `read_count` operands are the
/// register reads (external reads `0..read_count`), the rest the register
/// writes (external writes `read_count..len`), under the unchanged-stack,
/// architectural-fault, fall-through effects every byte-sequence encoder
/// declares.
pub(super) fn canonical_effects_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
    bytes: &[u8],
    read_count: usize,
    memory: MachineEncodedMemoryEffect,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let flags = physical
        .model()
        .view_named("rflags")
        .ok_or(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands[..read_count].to_vec(),
            register_writes: operands[read_count..].to_vec(),
            writes_rflags: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: (0..read_count as u16).collect(),
                external_operand_writes: (read_count as u16..operands.len() as u16).collect(),
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: flags.units.clone(),
                memory,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64SelectedFormFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub writes_rflags: bool,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedFormEncoding {
    bytes: Vec<u8>,
    footprint: X86_64SelectedFormFootprint,
}

impl ValidatedX86_64SelectedFormEncoding {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &X86_64SelectedFormFootprint {
        &self.footprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64SelectedFormEncodingError {
    UnsupportedAtomicForm,
    NonCanonicalPhysicalModel,
    LayoutDependentForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    BranchDisplacementOutsideI32,
    MalformedEncoding,
    EncodedFormMismatch,
    BranchDisplacementOutsideI8,
}

impl std::fmt::Display for X86_64SelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 selected-form encoding: {self:?}")
    }
}

impl std::error::Error for X86_64SelectedFormEncodingError {}

pub fn encode_x86_64_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::encode(physical, alternative, operands, 0);
    }
    if kind == SelectedInstructionKind::ZeroBytes {
        return zero_bytes::encode(physical, alternative, operands);
    }
    if matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) {
        return encoded_scalar::encode(physical, kind, alternative, operands);
    }
    if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
        return scan_byte_sequence::encode(physical, predicate, alternative, operands, 0);
    }
    if kind == SelectedInstructionKind::CompareByteSequences {
        return compare_byte_sequences::encode(physical, alternative, operands, 0);
    }
    if is_float_bit_transfer(kind) {
        return float_bits::encode(physical, kind, alternative, operands);
    }
    if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
        return ieee_float_arithmetic::encode(physical, form, alternative, operands);
    }
    if let SelectedInstructionKind::FloatResultPolicy { form } = kind {
        return float_result_policy::encode(physical, form, alternative, operands);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_scalar_registers(physical, kind, operands)?;
    validate_return_home(kind, &registers)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let bytes = encode_unchecked(kind, alternative, &registers)?;
    validate_x86_64_selected_form_encoding(physical, kind, alternative, operands, &bytes)
}

pub fn validate_x86_64_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if let SelectedInstructionKind::PortWrite { port, value } = kind {
        return port_write::validate(physical, port, value, alternative, operands, bytes);
    }
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::validate(physical, alternative, operands, 0, bytes);
    }
    if kind == SelectedInstructionKind::ZeroBytes {
        return zero_bytes::validate(physical, alternative, operands, bytes);
    }
    if matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) {
        return encoded_scalar::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
        return scan_byte_sequence::validate(physical, predicate, alternative, operands, 0, bytes);
    }
    if kind == SelectedInstructionKind::CompareByteSequences {
        return compare_byte_sequences::validate(physical, alternative, operands, 0, bytes);
    }
    if is_float_bit_transfer(kind) {
        return float_bits::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
        return ieee_float_arithmetic::validate(physical, form, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::FloatResultPolicy { form } = kind {
        return float_result_policy::validate(physical, form, alternative, operands, bytes);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_scalar_registers(physical, kind, operands)?;
    validate_return_home(kind, &registers)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let decoded = decode_all(bytes)?;
    validate_decoded(kind, alternative, &registers, &decoded)?;
    let canonical = encode_unchecked(kind, alternative, &registers)?;
    if bytes != canonical {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, alternative, operands),
    })
}

/// Selected forms must reject every proper prefix and every trailing byte:
/// truncation and extension are both encodings a validator may not admit, so
/// the per-form tests assert that byte-fragility uniformly.
#[cfg(test)]
pub(super) fn assert_byte_fragility(encoded: &[u8], rejects: impl Fn(&[u8]) -> bool) {
    for length in 0..encoded.len() {
        assert!(rejects(&encoded[..length]));
    }
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert!(rejects(&trailing));
}
