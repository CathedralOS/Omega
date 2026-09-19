//! The AAPCS64 and Darwin call and return keys, the register call key
//! families and the preservation convention of each target.

use register_model::{
    PreservationConvention, RegisterConstraintFamily, RegisterConstraintKey,
    ValidatedPhysicalRegisterModel,
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
    model
        .model()
        .conventions
        .iter()
        .find(|convention| convention.name == name)
}

pub const AARCH64_AAPCS64_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 0,
};

pub const AARCH64_DARWIN_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 1,
};

/// Arity-ordered keys for the darwin register-only U64 call ABI.
pub fn aarch64_darwin_register_call_keys() -> Vec<RegisterConstraintKey> {
    (11..=19)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Complete integer-bank result fragments, ordered by fragment count then input arity.
pub fn aarch64_register_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 1020 } else { 1000 };
    (first..first + 18)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

pub fn aarch64_register_aggregate_return_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 12 } else { 10 };
    (first..first + 2)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Return,
            variant,
        })
        .collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn aarch64_aapcs64_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (700..=708)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn aarch64_darwin_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (720..=728)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Arity-ordered keys for the complete register-only U64 call ABI.
pub fn aarch64_aapcs64_register_call_keys() -> Vec<RegisterConstraintKey> {
    [3, 4, 2, 5, 6, 7, 8, 9, 10]
        .into_iter()
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Per-plan normalized foreign call rows over the AAPCS64 integer bank.
/// Variants encode `(register arity, has scalar result)` as
/// `3000 + arity * 2 + has_result`; stack-passed arguments are outgoing
/// custody, not row operands, so the family bounds at the eight-register bank.
pub fn aarch64_aapcs64_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=8u32)
        .flat_map(|arity| {
            (0..=1u32).map(move |has_result| RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 3000 + arity * 2 + has_result,
            })
        })
        .collect()
}

/// Per-plan normalized foreign call rows over the Darwin AAPCS64 integer
/// bank. Variants encode `(register arity, has scalar result)` as
/// `3040 + arity * 2 + has_result`.
pub fn aarch64_darwin_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=8u32)
        .flat_map(|arity| {
            (0..=1u32).map(move |has_result| RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 3040 + arity * 2 + has_result,
            })
        })
        .collect()
}

/// Exact Linux AAPCS64 scalar call with two U64 arguments and one U64 result.
pub const AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 2,
};

pub const AARCH64_AAPCS64_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 0,
};

pub const AARCH64_DARWIN_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 1,
};

pub const AARCH64_AAPCS64_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 2,
};

pub const AARCH64_DARWIN_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 3,
};
