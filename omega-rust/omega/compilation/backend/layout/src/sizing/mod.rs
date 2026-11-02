use target::NativeTarget;

/// Size and alignment of one target carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    pub size: usize,
    pub alignment: usize,
}

pub(crate) const fn type_layout(size: usize, alignment: usize) -> TypeLayout {
    TypeLayout { size, alignment }
}

impl Default for TypeLayout {
    fn default() -> Self {
        Self {
            size: 0,
            alignment: 1,
        }
    }
}

/// Canonical fat-descriptor layout (`{ptr, len}`) for slices and text windows.
///
/// Single source of truth inside layout: derives size/alignment from
/// `runtime-abi` rather than re-deriving `2 * pointer_size` locally.
pub fn fat_descriptor_layout(target: NativeTarget) -> TypeLayout {
    let descriptor = runtime_abi::fat_descriptor_layout(target);
    type_layout(descriptor.byte_size, descriptor.alignment)
}

/// The canonical `TypeLayout` of one target pointer word.
///
/// Addresses, selectors, thin references and each word of a descriptor all
/// read the target's pointer geometry here instead of re-spelling
/// `{ target.pointer_size, target.pointer_alignment }` at the carrier arms.
pub fn pointer_layout(target: NativeTarget) -> TypeLayout {
    type_layout(target.pointer_size, target.pointer_alignment)
}
