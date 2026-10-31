//! The System-V and Microsoft call and return keys, the register call key
//! families and the preservation convention of each target.

use register_model::{
    PreservationConvention, RegisterConstraintKey as Key, ValidatedPhysicalRegisterModel,
};
use target::{Architecture, NativeTarget, ObjectFormat};

pub const X86_64_MICROSOFT_CALL_UNIT: Key = Key::call(700);

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

pub const X86_64_SYSTEM_V_CALL: Key = Key::call(0);

pub const X86_64_MICROSOFT_CALL: Key = Key::call(1);

/// Arity-ordered keys for the microsoft register-only U64 call ABI.
pub fn x86_64_microsoft_register_call_keys() -> Vec<Key> {
    (10..=14).map(Key::call).collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn x86_64_system_v_register_unit_call_keys() -> Vec<Key> {
    (0..=6).map(|arity| Key::call(740 + arity)).collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn x86_64_microsoft_register_unit_call_keys() -> Vec<Key> {
    (0..=4)
        .map(|arity| Key::call(if arity == 2 { 700 } else { 720 + arity }))
        .collect()
}

/// Arity-ordered keys for the complete register-only U64 call ABI.
pub fn x86_64_system_v_register_call_keys() -> Vec<Key> {
    [4, 5, 3, 6, 7, 8, 9].into_iter().map(Key::call).collect()
}

/// Exact Linux System-V scalar call with two U64 arguments and one U64 result.
pub fn x86_64_system_v_aggregate_call_keys() -> Vec<Key> {
    (1000..1014).map(Key::call).collect()
}

pub fn x86_64_system_v_aggregate_return_keys() -> Vec<Key> {
    (10..12).map(Key::ret).collect()
}

/// Microsoft direct aggregates return one integer fragment in rax; keys follow
/// the zero-through-four positional integer argument counts. Hidden result
/// pointers are a different ABI transport and have no row in this family.
pub fn x86_64_microsoft_aggregate_call_keys() -> Vec<Key> {
    (1020..1025).map(Key::call).collect()
}

pub fn x86_64_microsoft_aggregate_return_keys() -> Vec<Key> {
    vec![Key::ret(12)]
}

/// One System V import template. Evaluated call placement supplies operands.
pub fn x86_64_system_v_normalized_foreign_call_keys() -> Vec<Key> {
    vec![Key::call(3000)]
}

/// One Microsoft import template, distinct from System V target custody.
pub fn x86_64_microsoft_normalized_foreign_call_keys() -> Vec<Key> {
    vec![Key::call(3300)]
}

/// Exact Linux System-V scalar call with two U64 arguments and one U64 result.
pub(crate) const X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64: Key = Key::call(3);

pub const X86_64_SYSTEM_V_RETURN: Key = Key::ret(0);

pub const X86_64_MICROSOFT_RETURN: Key = Key::ret(1);

pub const X86_64_SYSTEM_V_RETURN_UNIT: Key = Key::ret(2);

pub const X86_64_MICROSOFT_RETURN_UNIT: Key = Key::ret(3);
