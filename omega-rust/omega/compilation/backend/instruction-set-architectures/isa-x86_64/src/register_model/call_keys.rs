//! The System-V and Microsoft call and return keys, the register call key
//! families and the preservation convention of each target.

use register_model::{
    PreservationConvention, RegisterConstraintKey, ValidatedPhysicalRegisterModel,
};
use target::{Architecture, NativeTarget, ObjectFormat};

pub const X86_64_MICROSOFT_CALL_UNIT: RegisterConstraintKey = RegisterConstraintKey::call(700);

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
        // Intel macOS preserves the System V AMD64 callee-saved set.
        ObjectFormat::Elf | ObjectFormat::MachO => "system-v-amd64",
        ObjectFormat::Coff => "microsoft-x64",
    };
    model.model().convention_named(name)
}

pub const X86_64_SYSTEM_V_CALL: RegisterConstraintKey = RegisterConstraintKey::call(0);

pub const X86_64_MICROSOFT_CALL: RegisterConstraintKey = RegisterConstraintKey::call(1);

/// Arity-ordered keys for the microsoft register-only U64 call ABI.
pub fn x86_64_microsoft_register_call_keys() -> Vec<RegisterConstraintKey> {
    (10..=14).map(RegisterConstraintKey::call).collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn x86_64_system_v_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=6)
        .map(|arity| RegisterConstraintKey::call(740 + arity))
        .collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn x86_64_microsoft_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (0..=4)
        .map(|arity| RegisterConstraintKey::call(if arity == 2 { 700 } else { 720 + arity }))
        .collect()
}

/// Arity-ordered keys for the complete register-only U64 call ABI.
pub fn x86_64_system_v_register_call_keys() -> Vec<RegisterConstraintKey> {
    [4, 5, 3, 6, 7, 8, 9]
        .into_iter()
        .map(RegisterConstraintKey::call)
        .collect()
}

/// Exact Linux System-V scalar call with two U64 arguments and one U64 result.
pub fn x86_64_system_v_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
    (1000..1014).map(RegisterConstraintKey::call).collect()
}

pub fn x86_64_system_v_aggregate_return_keys() -> Vec<RegisterConstraintKey> {
    (10..12).map(RegisterConstraintKey::ret).collect()
}

/// Microsoft direct aggregates return one integer fragment in rax; keys follow
/// the zero-through-four positional integer argument counts. Hidden result
/// pointers are a different ABI transport and have no row in this family.
pub fn x86_64_microsoft_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
    (1020..1025).map(RegisterConstraintKey::call).collect()
}

pub fn x86_64_microsoft_aggregate_return_keys() -> Vec<RegisterConstraintKey> {
    vec![RegisterConstraintKey::ret(12)]
}

/// One System V import template. Evaluated call placement supplies operands.
pub fn x86_64_system_v_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    vec![RegisterConstraintKey::call(3000)]
}

/// One Microsoft import template, distinct from System V target custody.
pub fn x86_64_microsoft_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
    vec![RegisterConstraintKey::call(3300)]
}

/// Exact Linux System-V scalar call with two U64 arguments and one U64 result.
pub(crate) const X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64: RegisterConstraintKey =
    RegisterConstraintKey::call(3);

pub const X86_64_SYSTEM_V_RETURN: RegisterConstraintKey = RegisterConstraintKey::ret(0);

pub const X86_64_MICROSOFT_RETURN: RegisterConstraintKey = RegisterConstraintKey::ret(1);

pub const X86_64_SYSTEM_V_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey::ret(2);

pub const X86_64_MICROSOFT_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey::ret(3);
