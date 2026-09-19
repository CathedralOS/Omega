//! Encoding and validating selected x86_64 forms.
//!
//! This file carries the footprint, the validated encoding, the error and
//! the two entry points. `branch_forms.rs` encodes and validates branch
//! forms, `request_validation.rs` validates requests and resolves
//! registers, `instruction_bytes.rs` assembles instruction bytes,
//! `decoding.rs` decodes and validates the emitted bytes, and
//! `saturating_forms.rs` names the realization shape of each saturating
//! operation and carrier that those three and the machine-effect catalog share.

#[cfg(test)]
mod boolean_materialization_tests;
mod branch_forms;
#[cfg(test)]
mod byte_view_address_tests;
mod copy_bytes;
mod decoding;
mod float_bits;
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
#[cfg(test)]
mod tests;

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
pub use jump::*;
pub use memory::*;
pub use normalized_foreign_call::*;
pub use scalar_call::*;

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};

use crate::selected_form_encoding::decoding::{decode_all, footprint, validate_decoded};
use crate::selected_form_encoding::instruction_bytes::encode_unchecked;
use crate::selected_form_encoding::request_validation::{
    resolve_scalar_registers, validate_return_home,
};
use crate::selected_form_encoding::request_validation::{
    validate_alias_partition, validate_request,
};
use crate::x86_64_physical_register_model;

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
    if float_bits::is_transfer(kind) {
        return float_bits::encode(physical, kind, alternative, operands);
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
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::validate(physical, alternative, operands, 0, bytes);
    }
    if float_bits::is_transfer(kind) {
        return float_bits::validate(physical, kind, alternative, operands, bytes);
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
