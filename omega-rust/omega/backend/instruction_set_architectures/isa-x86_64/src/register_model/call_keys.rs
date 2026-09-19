//! The System-V and Microsoft call and return keys, the register call key
//! families and the preservation convention of each target.

use register_model::{
    PreservationConvention, RegisterConstraintFamily, RegisterConstraintKey,
    ValidatedPhysicalRegisterModel,
};
use target::{Architecture, NativeTarget, ObjectFormat};

pub const X86_64_MICROSOFT_CALL_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 700,
};

/// Resolve the exact preservation convention selected by the clean terminal
/// lane for one supported x86-64 target. Keeping this mapping in the ISA owner
/// prevents target-neutral orchestration from inferring ABI policy from vector
/// positions or authored names.
pub fn x86_64_preservation_convention_for_target(
    model: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Option<&PreservationConvention> {
    if target.architecture != Architecture::X86_64 {
        return None;
    }
    let name = match target.object_format {
        ObjectFormat::Elf => "system-v-amd64",
        ObjectFormat::Coff => "microsoft-x64",
        ObjectFormat::MachO => return None,
    };
    model
        .model()
        .conventions
        .iter()
        .find(|convention| convention.name == name)
}

pub const X86_64_SYSTEM_V_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 0,
};

pub const X86_64_MICROSOFT_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 1,
};

/// Arity-ordered keys for the microsoft register-only U64 call ABI.
pub fn x86_64_microsoft_register_call_keys() -> Vec<RegisterConstraintKey> {
    (10..=14)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn x86_64_system_v_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=6)
        .map(|arity| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 740 + arity,
        })
        .collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn x86_64_microsoft_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=4)
        .map(|arity| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: if arity == 2 { 700 } else { 720 + arity },
        })
        .collect()
}

/// Arity-ordered keys for the complete register-only U64 call ABI.
pub fn x86_64_system_v_register_call_keys() -> Vec<RegisterConstraintKey> {
    [4, 5, 3, 6, 7, 8, 9]
        .into_iter()
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Exact Linux System-V scalar call with two U64 arguments and one U64 result.
pub fn x86_64_system_v_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
    (1000..1014)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

pub fn x86_64_system_v_aggregate_return_keys() -> Vec<RegisterConstraintKey> {
    (10..12)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Return,
            variant,
        })
        .collect()
}

/// Microsoft direct aggregates return one integer fragment in rax; keys follow
/// the zero-through-four positional integer argument counts. Hidden result
/// pointers are a different ABI transport and have no row in this family.
pub fn x86_64_microsoft_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
    (1020..1025)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

pub fn x86_64_microsoft_aggregate_return_keys() -> Vec<RegisterConstraintKey> {
    vec![RegisterConstraintKey {
        family: RegisterConstraintFamily::Return,
        variant: 12,
    }]
}

/// Per-plan normalized foreign call rows over the System-V integer bank.
/// Variants encode `(register arity, has scalar result)` as
/// `3000 + arity * 2 + has_result`; stack-passed arguments are outgoing
/// custody, not row operands, so the family bounds at the six-register bank.
pub fn x86_64_system_v_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=6u32)
        .flat_map(|arity| {
            (0..=1u32).map(move |has_result| RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 3000 + arity * 2 + has_result,
            })
        })
        .collect()
}

/// Per-plan normalized foreign call rows over the Microsoft integer bank.
/// Variants encode `(register arity, has scalar result)` as
/// `3040 + arity * 2 + has_result`; stack-passed arguments are outgoing
/// custody, not row operands, so the family bounds at the four-register bank.
pub fn x86_64_microsoft_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=4u32)
        .flat_map(|arity| {
            (0..=1u32).map(move |has_result| RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 3040 + arity * 2 + has_result,
            })
        })
        .collect()
}

/// Exact Linux System-V scalar call with two U64 arguments and one U64 result.
pub const X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 3,
};

pub const X86_64_SYSTEM_V_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 0,
};

pub const X86_64_MICROSOFT_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 1,
};

pub const X86_64_SYSTEM_V_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 2,
};

pub const X86_64_MICROSOFT_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 3,
};
