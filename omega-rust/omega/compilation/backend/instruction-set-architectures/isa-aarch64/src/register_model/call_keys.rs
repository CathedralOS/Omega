//! The AAPCS64 and Darwin call and return keys, the register call key
//! families and the preservation convention of each target.

use register_model::{
    PreservationConvention, RegisterConstraintKey, ValidatedPhysicalRegisterModel,
};
use target::{Architecture, NativeTarget, ObjectFormat};

/// Resolve the exact preservation convention selected by the clean terminal
/// lane for one supported AArch64 target. The ISA owner, rather than generic
/// orchestration, owns this target/object-format to ABI-policy mapping.
pub fn aarch64_preservation_convention_for_target(
    model: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Option<&PreservationConvention> {
    if target.architecture != Architecture::Aarch64 {
        return None;
    }
    let name = match target.object_format {
        ObjectFormat::Elf => "aapcs64",
        ObjectFormat::MachO => "darwin-aapcs64",
        ObjectFormat::Coff => return None,
    };
    model.model().convention_named(name)
}

pub const AARCH64_AAPCS64_CALL: RegisterConstraintKey = RegisterConstraintKey::call(0);

pub const AARCH64_DARWIN_CALL: RegisterConstraintKey = RegisterConstraintKey::call(1);

/// Arity-ordered keys for the darwin register-only U64 call ABI.
pub fn aarch64_darwin_register_call_keys() -> Vec<RegisterConstraintKey> {
    (11..=19).map(RegisterConstraintKey::call).collect()
}

/// Complete integer-bank result fragments, ordered by fragment count then input arity.
pub fn aarch64_register_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 1020 } else { 1000 };
    (first..first + 18)
        .map(RegisterConstraintKey::call)
        .collect()
}

pub fn aarch64_register_aggregate_return_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 12 } else { 10 };
    (first..first + 2).map(RegisterConstraintKey::ret).collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn aarch64_aapcs64_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (700..=708).map(RegisterConstraintKey::call).collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn aarch64_darwin_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (720..=728).map(RegisterConstraintKey::call).collect()
}

/// Arity-ordered keys for the complete register-only U64 call ABI.
pub fn aarch64_aapcs64_register_call_keys() -> Vec<RegisterConstraintKey> {
    [3, 4, 2, 5, 6, 7, 8, 9, 10]
        .into_iter()
        .map(RegisterConstraintKey::call)
        .collect()
}

/// Register-only Unit call keys of the target's ABI, indexed by argument
/// count. Every consumer states its target once through this selector
/// instead of re-spelling the AAPCS64/Darwin pair beside each use.
pub fn aarch64_register_unit_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    if darwin {
        aarch64_darwin_register_unit_call_keys()
    } else {
        aarch64_aapcs64_register_unit_call_keys()
    }
}

/// One AAPCS64 import template. Evaluated call placement supplies operands.
pub fn aarch64_aapcs64_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    vec![RegisterConstraintKey::call(3000)]
}

/// One Darwin import template, distinct from AAPCS64 target custody.
pub fn aarch64_darwin_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    vec![RegisterConstraintKey::call(3300)]
}

/// Exact Linux AAPCS64 scalar call with two U64 arguments and one U64 result.
pub(crate) const AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64: RegisterConstraintKey =
    RegisterConstraintKey::call(2);

pub const AARCH64_AAPCS64_RETURN: RegisterConstraintKey = RegisterConstraintKey::ret(0);

pub const AARCH64_DARWIN_RETURN: RegisterConstraintKey = RegisterConstraintKey::ret(1);

pub const AARCH64_AAPCS64_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey::ret(2);

pub const AARCH64_DARWIN_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey::ret(3);
